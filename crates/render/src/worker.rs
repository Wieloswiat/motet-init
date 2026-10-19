//! The worker process: limits first, then one job, then exit.

use std::io::{Read, Write};
use std::path::PathBuf;

use typst_layout::PagedDocument;

use crate::job::{Failure, Job, Outcome, Rendered};
use crate::world::Sandbox;

/// Runs one job from stdin and writes its outcome to stdout as JSON.
///
/// OS limits are applied before the job is even read, so nothing the job
/// contains runs without them. Exceeding the CPU limit kills the process;
/// exceeding the memory limit makes allocation fail, which aborts it. The
/// supervisor reports either as a failure.
pub fn run(packages: Option<PathBuf>) -> std::io::Result<()> {
    let mut input = Vec::new();
    std::io::stdin().read_to_end(&mut input)?;
    let job: Job = serde_json::from_slice(&input).map_err(std::io::Error::other)?;
    apply_limits(&job)?;
    let outcome = render(&job, packages);
    let mut stdout = std::io::stdout().lock();
    serde_json::to_writer(&mut stdout, &outcome).map_err(std::io::Error::other)?;
    stdout.flush()
}

#[cfg(unix)]
fn apply_limits(job: &Job) -> std::io::Result<()> {
    let memory = job.limits.max_memory_bytes;
    rlimit::setrlimit(rlimit::Resource::AS, memory, memory)?;
    let cpu = job.limits.max_cpu_seconds;
    rlimit::setrlimit(rlimit::Resource::CPU, cpu, cpu)
}

#[cfg(not(unix))]
fn apply_limits(_job: &Job) -> std::io::Result<()> {
    Err(std::io::Error::other("the render worker needs OS resource limits"))
}

/// Compiles in this process. The worker calls it after applying limits;
/// tests call it directly to check the sandbox itself.
pub fn render(job: &Job, packages: Option<PathBuf>) -> Outcome {
    if job.source.len() > job.limits.max_source_bytes {
        return Err(Failure::TooLong(job.limits.max_source_bytes));
    }
    let world = Sandbox::new(job.source.clone(), job.date, packages);
    let warned = typst::compile::<PagedDocument>(&world);
    let warnings = warned.warnings.iter().map(|w| w.message.to_string()).collect();
    let document = warned
        .output
        .map_err(|errors| Failure::Source(errors.iter().map(|e| e.message.to_string()).collect()))?;
    let options = typst_svg::SvgOptions::default();
    let mut size = 0;
    let mut pages = Vec::with_capacity(document.pages().len());
    for page in document.pages() {
        let svg = typst_svg::svg(page, &options);
        size += svg.len();
        if size > job.limits.max_output_bytes {
            return Err(Failure::TooLarge(job.limits.max_output_bytes));
        }
        pages.push(svg);
    }
    Ok(Rendered { pages, warnings })
}
