//! Library entrypoint for the `whittle` CLI and the napi addon.
//!
//! `check` simulates normalization and exits non-zero without writing, for an
//! agent or CI. `fix` writes it. Both judge the message by its semantic
//! content (git's comment block and scissors section stripped, trimmed);
//! `fix` additionally canonicalizes that formatting on disk.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use clap::{Parser, Subcommand, ValueEnum};
use serde::Serialize;

pub mod config;
pub mod diagnostic;
pub mod guidance;
pub mod lint;
pub mod transform;

use config::Config;
use diagnostic::{Diagnostic, DiagnosticJson, Field, Severity};
use lint::lint;
use transform::{CommitParts, transform};

/// Version of the `--format json` payload. Bumped only on breaking changes.
const REPORT_VERSION: u32 = 1;

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

    /// Output format. `human` writes to stderr, `json` writes a report to stdout.
    #[arg(long, global = true, value_enum, default_value_t = Format::Human)]
    format: Format,

    #[command(subcommand)]
    command: Command,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
enum Format {
    Human,
    Json,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Report what `fix` would change, without modifying the file. Exits
    /// non-zero if any rewrite is pending or any rule is violated.
    Check {
        /// Path to a commit message file (e.g. .`git/COMMIT_EDITMSG`).
        file: PathBuf,
    },
    /// Apply transforms in place, then validate. Default for `commit-msg` hook usage.
    Fix {
        /// Path to a commit message file (e.g. .`git/COMMIT_EDITMSG`).
        file: PathBuf,
    },
    /// Print the active ruleset as instructions, for `CLAUDE.md` or an agent
    /// prompt. Knowing the rules up front beats learning them one rejected
    /// commit at a time.
    Rules,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    Check,
    Fix,
}

impl Mode {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Check => "check",
            Self::Fix => "fix",
        }
    }
}

/// Machine-readable result of a run, emitted by `--format json`.
#[derive(Serialize)]
struct Report<'a> {
    version: u32,
    mode: &'static str,
    file: &'a str,
    /// True exactly when the process exits 0.
    ok: bool,
    original: &'a str,
    /// The message to adopt, or null if nothing is adoptable.
    normalized: Option<&'a str>,
    diagnostics: Vec<DiagnosticJson<'a>>,
    guidance: &'a [String],
}

/// Exit code: 0 success, 1 diagnostics/I/O failure, 2 argument parse failure.
pub fn run_cli<I, S>(args: I) -> i32
where
    I: IntoIterator<Item = S>,
    S: Into<std::ffi::OsString> + Clone,
{
    let cli = match Cli::try_parse_from(args) {
        Ok(c) => c,
        Err(e) => {
            let use_stderr = e.use_stderr();
            e.print().ok();
            return if use_stderr { 2 } else { 0 };
        }
    };
    let (file, mode) = match &cli.command {
        Command::Check { file } => (file.clone(), Mode::Check),
        Command::Fix { file } => (file.clone(), Mode::Fix),
        Command::Rules => return run_rules(&cli),
    };

    let rules_command = rules_command(&cli);
    match run_inner(&cli, &file, mode) {
        Ok(outcome) => emit(&outcome, mode, cli.format, &rules_command),
        Err(e) => {
            let outcome = Outcome::failed(&file, &format!("{e:#}"));
            emit(&outcome, mode, cli.format, &rules_command)
        }
    }
}

/// `println!` panics on a closed pipe (Rust ignores SIGPIPE); exit quietly instead.
fn print_stdout(s: &str) {
    use std::io::Write;
    if writeln!(std::io::stdout(), "{s}").is_err() {
        std::process::exit(0);
    }
}

fn rules_command(cli: &Cli) -> String {
    match &cli.config {
        Some(p) => format!("whittle --config {} rules", p.display()),
        None => "whittle rules".to_string(),
    }
}

fn run_rules(cli: &Cli) -> i32 {
    let config = match Config::load_or_default(cli.config.as_deref()) {
        Ok(c) => c,
        Err(e) => {
            let message = format!("failed to load whittle config: {e:#}");
            if cli.format == Format::Json {
                let payload = serde_json::json!({
                    "version": REPORT_VERSION,
                    "ok": false,
                    "rules": [],
                    "diagnostics": [{
                        "code": "whittle.failed",
                        "severity": "error",
                        "message": message,
                    }],
                });
                print_stdout(&payload.to_string());
            } else {
                eprintln!("whittle: {message}");
            }
            return 1;
        }
    };
    let rules = guidance::rules(&config);
    match cli.format {
        Format::Human => {
            print_stdout("whittle commit message rules:");
            for r in &rules {
                print_stdout(&format!("  - {r}"));
            }
        }
        Format::Json => {
            let payload =
                serde_json::json!({ "version": REPORT_VERSION, "ok": true, "rules": rules });
            match serde_json::to_string_pretty(&payload) {
                Ok(s) => print_stdout(&s),
                Err(e) => {
                    eprintln!("whittle: could not serialize rules: {e}");
                    return 1;
                }
            }
        }
    }
    0
}

fn run_inner(cli: &Cli, file: &Path, mode: Mode) -> Result<Outcome> {
    let config =
        Config::load_or_default(cli.config.as_deref()).context("failed to load whittle config")?;

    let raw = std::fs::read_to_string(file)
        .with_context(|| format!("could not read {}", file.display()))?;
    let original = strip_git_comments(&raw).trim().to_string();

    if original.is_empty() {
        return Ok(Outcome::clean(file, original));
    }

    let mut parts = match CommitParts::parse(&original) {
        Ok(p) => p,
        Err(e) => {
            if !config.rules.require_conventional {
                return Ok(Outcome::clean(file, original));
            }
            let diag = Diagnostic::error("commit.not-conventional", Field::Message)
                .removing(&original)
                .message_override(e.to_string());
            let guidance = guidance::guidance(std::slice::from_ref(&diag), &config);
            return Ok(Outcome {
                file: file.to_path_buf(),
                original,
                normalized: None,
                diagnostics: vec![diag],
                guidance,
                needs_rewrite: false,
            });
        }
    };

    // Captured before `transform` mutates it: needed for the rewording check.
    let authored_description = parts.description.clone();
    let mut diagnostics = transform(&mut parts, &config);
    let normalized = parts.render();

    // e.g. `fix: [...]` normalizes to an empty, unparseable description.
    if let Err(e) = CommitParts::parse(&normalized) {
        let mut guidance = guidance::guidance(&diagnostics, &config);
        diagnostics.retain(|d| d.severity == Severity::Error);
        diagnostics.push(
            Diagnostic::error("normalize.invalid-result", Field::Message)
                .rewrite(&original, &normalized)
                .message_override(format!(
                    "normalizing would produce a message that does not parse ({e}); \
                     file left untouched"
                )),
        );
        diagnostics.extend(lint(&parts, &config));
        for line in guidance::guidance(&diagnostics, &config) {
            if !guidance.contains(&line) {
                guidance.push(line);
            }
        }
        return Ok(Outcome {
            file: file.to_path_buf(),
            original,
            normalized: None,
            diagnostics,
            guidance,
            needs_rewrite: false,
        });
    }

    let needs_rewrite = normalized != original;
    if needs_rewrite && diagnostics.is_empty() {
        diagnostics.push(Diagnostic::transform("message.reformatted", Field::Message).message_override(
            "the message needs reformatting (blank lines, footer spacing, or a `BREAKING CHANGE:` \
             footer turning into `!`) — see the suggested message below",
        ));
    } else if !needs_rewrite {
        diagnostics.retain(|d| d.severity == Severity::Error);
    }

    let mut ruined = Vec::new();
    if mode == Mode::Check {
        ruined = guidance::mangled_description(&authored_description, &config.description);
        for (before, after) in &ruined {
            diagnostics.push(
                Diagnostic::error("subject.needs-rewording", Field::Description)
                    .rewrite(before, after)
                    .message_override(format!(
                        "`{before}` would become `{after}` — reword it instead"
                    )),
            );
        }
    }

    if mode == Mode::Fix {
        let new_content = format!("{normalized}\n");
        if new_content != raw {
            std::fs::write(file, &new_content)
                .with_context(|| format!("could not write {}", file.display()))?;
        }
        Diagnostic::mark_applied(&mut diagnostics);
    }

    diagnostics.extend(lint(&parts, &config));
    let guidance = guidance::guidance(&diagnostics, &config);
    let adoptable = ruined.is_empty() && !diagnostics.iter().any(|d| d.severity == Severity::Error);

    Ok(Outcome {
        file: file.to_path_buf(),
        original,
        normalized: adoptable.then_some(normalized),
        diagnostics,
        guidance,
        needs_rewrite,
    })
}

struct Outcome {
    file: PathBuf,
    original: String,
    normalized: Option<String>,
    diagnostics: Vec<Diagnostic>,
    guidance: Vec<String>,
    needs_rewrite: bool,
}

impl Outcome {
    fn clean(file: &Path, original: String) -> Self {
        Self {
            file: file.to_path_buf(),
            normalized: Some(original.clone()),
            original,
            diagnostics: Vec::new(),
            guidance: Vec::new(),
            needs_rewrite: false,
        }
    }

    fn failed(file: &Path, message: &str) -> Self {
        Self {
            file: file.to_path_buf(),
            original: String::new(),
            normalized: None,
            diagnostics: vec![
                Diagnostic::error("whittle.failed", Field::Message).message_override(message),
            ],
            guidance: Vec::new(),
            needs_rewrite: false,
        }
    }

    fn has_errors(&self) -> bool {
        self.diagnostics
            .iter()
            .any(|d| d.severity == Severity::Error)
    }

    fn exit_code(&self, mode: Mode) -> i32 {
        let failing = match mode {
            Mode::Check => self.needs_rewrite || self.has_errors(),
            Mode::Fix => self.has_errors(),
        };
        i32::from(failing)
    }
}

fn emit(outcome: &Outcome, mode: Mode, format: Format, rules_command: &str) -> i32 {
    let code = outcome.exit_code(mode);
    match format {
        Format::Human => emit_human(outcome, mode, rules_command),
        Format::Json => emit_json(outcome, mode, code),
    }
    code
}

fn emit_human(outcome: &Outcome, mode: Mode, rules_command: &str) {
    for d in &outcome.diagnostics {
        eprintln!("whittle[{}]: {}", d.code, d.message());
    }
    if !outcome.guidance.is_empty() {
        eprintln!();
        eprintln!("how to write a message whittle accepts:");
        for line in &outcome.guidance {
            eprintln!("  - {line}");
        }
        eprintln!();
        eprintln!("full ruleset: {rules_command}");
    }
    if let Some(normalized) = &outcome.normalized
        && mode == Mode::Check
        && outcome.needs_rewrite
        && !outcome.has_errors()
    {
        eprintln!();
        eprintln!("suggested message:");
        for line in normalized.lines() {
            eprintln!("  {line}");
        }
    }
}

fn emit_json(outcome: &Outcome, mode: Mode, code: i32) {
    let report = Report {
        version: REPORT_VERSION,
        mode: mode.as_str(),
        file: &outcome.file.to_string_lossy(),
        ok: code == 0,
        original: &outcome.original,
        normalized: outcome.normalized.as_deref(),
        diagnostics: outcome.diagnostics.iter().map(Into::into).collect(),
        guidance: &outcome.guidance,
    };
    match serde_json::to_string_pretty(&report) {
        Ok(s) => print_stdout(&s),
        Err(e) => print_stdout(&format!(
            "{{\"version\":{REPORT_VERSION},\"ok\":false,\
             \"diagnostics\":[{{\"code\":\"whittle.failed\",\"severity\":\"error\",\
             \"message\":\"could not serialize report: {e}\"}}]}}"
        )),
    }
}

/// Drops `#` comment lines and everything from the scissors line onward —
/// `git commit -v` appends the diff there uncommented, before git itself strips it.
fn strip_git_comments(raw: &str) -> String {
    let mut kept = Vec::new();
    for line in raw.lines() {
        if is_scissors_line(line) {
            break;
        }
        if !line.starts_with('#') {
            kept.push(line);
        }
    }
    kept.join("\n")
}

fn is_scissors_line(line: &str) -> bool {
    line.starts_with('#') && line.contains(">8")
}
