//! whittle: lint + auto-normalize Conventional Commit subjects.
//!
//! Library entrypoint used by both the standalone CLI binary
//! (`src/main.rs`) and the napi addon (`crates/whittle-napi`).

use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};

pub mod config;
pub mod lint;
pub mod transform;

use config::Config;
use lint::lint;
use transform::{CommitParts, transform};

#[derive(Parser, Debug)]
#[command(
    name = "whittle",
    version,
    about = "Lint + auto-normalize Conventional Commit messages"
)]
struct Cli {
    /// Path to a whittle.toml configuration file. Default: built-in defaults.
    #[arg(long, global = true)]
    config: Option<PathBuf>,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Validate a commit message file without modifying it.
    Check {
        /// Path to a commit message file (e.g. .`git/COMMIT_EDITMSG`).
        file: PathBuf,
    },
    /// Apply transforms in place, then validate. Default for `commit-msg` hook usage.
    Fix {
        /// Path to a commit message file (e.g. .`git/COMMIT_EDITMSG`).
        file: PathBuf,
    },
}

/// Run the whittle CLI with the given arguments (argv[0] = program name).
///
/// Returns a Unix-style exit code. Diagnostics are written to stderr.
pub fn run_cli<I, S>(args: I) -> i32
where
    I: IntoIterator<Item = S>,
    S: Into<std::ffi::OsString> + Clone,
{
    match run_inner(args) {
        Ok(code) => code,
        Err(e) => {
            eprintln!("whittle: {e:#}");
            1
        }
    }
}

fn run_inner<I, S>(args: I) -> Result<i32>
where
    I: IntoIterator<Item = S>,
    S: Into<std::ffi::OsString> + Clone,
{
    let cli = match Cli::try_parse_from(args) {
        Ok(c) => c,
        Err(e) => {
            // --help / --version exit 0 and print to stdout; real parse errors
            // exit 2 and print to stderr (clap convention).
            let use_stderr = e.use_stderr();
            e.print().ok();
            return Ok(if use_stderr { 2 } else { 0 });
        }
    };
    let config =
        Config::load_or_default(cli.config.as_deref()).context("failed to load whittle config")?;

    let (file, fix) = match &cli.command {
        Command::Check { file } => (file.clone(), false),
        Command::Fix { file } => (file.clone(), true),
    };

    let raw = std::fs::read_to_string(&file)
        .with_context(|| format!("could not read {}", file.display()))?;
    let stripped = strip_git_comments(&raw);

    if stripped.trim().is_empty() {
        return Ok(0);
    }

    let mut parts = match CommitParts::parse(&stripped) {
        Ok(p) => p,
        Err(e) => {
            if config.rules.require_conventional {
                eprintln!("whittle: {e}");
                return Ok(1);
            }
            return Ok(0);
        }
    };

    if fix {
        transform(&mut parts, &config);
        let new_content = format!("{}\n", parts.render());
        if new_content != raw {
            std::fs::write(&file, &new_content)
                .with_context(|| format!("could not write {}", file.display()))?;
        }
    }

    let diagnostics = lint(&parts, &config);
    if !diagnostics.is_empty() {
        for d in &diagnostics {
            eprintln!("whittle[{}]: {}", d.code, d.message);
        }
        return Ok(1);
    }

    Ok(0)
}

fn strip_git_comments(raw: &str) -> String {
    raw.lines()
        .filter(|l| !l.starts_with('#'))
        .collect::<Vec<_>>()
        .join("\n")
}
