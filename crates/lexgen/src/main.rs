//! `motet-lexgen generate|check --lexicons DIR --rust DIR --ts DIR`

use std::path::PathBuf;
use std::process::ExitCode;

use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(about = "Generate Rust and TypeScript types from lexicons")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Write generated code.
    Generate(Paths),
    /// Fail if generated code differs from what `generate` would write.
    Check(Paths),
}

#[derive(clap::Args)]
struct Paths {
    #[arg(long)]
    lexicons: PathBuf,
    #[arg(long)]
    rust: PathBuf,
    #[arg(long)]
    ts: Option<PathBuf>,
}

fn main() -> Result<ExitCode> {
    let cli = Cli::parse();
    let (paths, write) = match cli.command {
        Command::Generate(p) => (p, true),
        Command::Check(p) => (p, false),
    };
    let docs = motet_lexgen::load(&paths.lexicons)?;
    let mut outputs = motet_lexgen::rust::generate(&docs)?.into_iter().map(|(f, c)| (paths.rust.join(f), c)).collect::<Vec<_>>();
    if let Some(ts) = &paths.ts {
        outputs.extend(motet_lexgen::ts::generate(&docs)?.into_iter().map(|(f, c)| (ts.join(f), c)));
    }

    let mut stale = Vec::new();
    for (path, contents) in outputs {
        let current = std::fs::read_to_string(&path).ok();
        if current.as_deref() == Some(contents.as_str()) {
            continue;
        }
        if write {
            if let Some(dir) = path.parent() {
                std::fs::create_dir_all(dir)?;
            }
            std::fs::write(&path, contents)?;
        } else {
            stale.push(path);
        }
    }
    if stale.is_empty() {
        return Ok(ExitCode::SUCCESS);
    }
    for path in stale {
        eprintln!("stale: {} (run `lexgen` in the dev shell)", path.display());
    }
    Ok(ExitCode::FAILURE)
}
