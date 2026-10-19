//! The parent's side: one worker process per job, killed if it overruns.

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::process::Command;

use crate::job::{Failure, Job, Outcome};

#[derive(Debug, thiserror::Error)]
pub enum SupervisorError {
    #[error("can't start the render worker: {0}")]
    Spawn(#[from] std::io::Error),
}

/// Runs jobs in worker processes.
#[derive(Clone, Debug)]
pub struct Supervisor {
    worker: PathBuf,
    packages: Option<PathBuf>,
}

impl Supervisor {
    /// `worker` is the `motet-render-worker` binary. It runs with an empty
    /// environment, no arguments but the package mirror, and `/` as its
    /// working directory.
    pub fn new(worker: impl Into<PathBuf>, packages: Option<PathBuf>) -> Self {
        Self {
            worker: worker.into(),
            packages,
        }
    }

    pub async fn render(&self, job: &Job) -> Result<Outcome, SupervisorError> {
        if job.source.len() > job.limits.max_source_bytes {
            return Ok(Err(Failure::TooLong(job.limits.max_source_bytes)));
        }
        let mut command = Command::new(&self.worker);
        command
            .env_clear()
            .current_dir(Path::new("/"))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true);
        if let Some(packages) = &self.packages {
            command.arg(packages);
        }
        let mut child = command.spawn()?;
        let input = serde_json::to_vec(job).expect("jobs always serialize");
        let mut stdin = child.stdin.take().expect("stdin was piped");
        let mut stdout = child.stdout.take().expect("stdout was piped");

        // JSON escaping can at most double the SVG, plus framing.
        let cap = u64::try_from(job.limits.max_output_bytes)
            .unwrap_or(u64::MAX)
            .saturating_mul(2)
            + 4096;
        let exchange = async {
            stdin.write_all(&input).await?;
            drop(stdin);
            let mut output = Vec::new();
            (&mut stdout).take(cap + 1).read_to_end(&mut output).await?;
            let status = child.wait().await?;
            Ok::<_, std::io::Error>((status, output))
        };
        let wall = Duration::from_millis(job.limits.max_wall_millis);
        let Ok(result) = tokio::time::timeout(wall, exchange).await else {
            // Dropping the child kills it (kill_on_drop).
            return Ok(Err(Failure::Timeout));
        };
        let Ok((status, output)) = result else {
            return Ok(Err(Failure::Crashed));
        };
        if output.len() as u64 > cap {
            return Ok(Err(Failure::TooLarge(job.limits.max_output_bytes)));
        }
        if !status.success() {
            return Ok(Err(cpu_or_crash(status)));
        }
        Ok(serde_json::from_slice(&output).unwrap_or(Err(Failure::Crashed)))
    }
}

#[cfg(unix)]
fn cpu_or_crash(status: std::process::ExitStatus) -> Failure {
    use std::os::unix::process::ExitStatusExt;
    // SIGXCPU (24 on Linux and macOS) or SIGKILL at the hard CPU limit.
    match status.signal() {
        Some(24 | 9) => Failure::Timeout,
        _ => Failure::Crashed,
    }
}

#[cfg(not(unix))]
fn cpu_or_crash(_status: std::process::ExitStatus) -> Failure {
    Failure::Crashed
}
