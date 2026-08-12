//! Regenerates `README.md` from `README.md.tera`, running the real `whittle`
//! binary for each documented command. `cargo run -p xtask -- [--check]`.

use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

use anyhow::{Context, Result, bail};
use tera::{Kwargs, State, Tera, TeraResult};

const TEMPLATE_NAME: &str = "README.md.tera";
const DEFAULT_DISPLAY_PATH: &str = ".git/COMMIT_EDITMSG";

/// Resolved relative to the crate (not the caller's CWD), so this works
/// whether invoked as `cargo run -p xtask` from anywhere in the repo.
fn manifest_dir() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn main() -> Result<()> {
    let check_only = std::env::args().any(|a| a == "--check");
    let manifest_dir = manifest_dir();
    let template_path = manifest_dir.join("templates").join(TEMPLATE_NAME);
    let repo_root = manifest_dir
        .parent()
        .and_then(std::path::Path::parent)
        .context("crates/xtask is not nested two directories under the repo root")?;
    let output_path = repo_root.join("README.md");

    let template = std::fs::read_to_string(&template_path)
        .with_context(|| format!("could not read {}", template_path.display()))?;

    let mut tera = Tera::default();
    tera.autoescape_on(Vec::<&str>::new());
    // Must register before add_raw_template: tera 2.x resolves calls at parse time.
    tera.register_function("whittle", whittle_fn);
    tera.add_raw_template(TEMPLATE_NAME, &template)?;

    let rendered = tera
        .render(TEMPLATE_NAME, &tera::Context::new())
        .context("failed to render README.md.tera")?;

    if check_only {
        let current = std::fs::read_to_string(&output_path).unwrap_or_default();
        if current != rendered {
            bail!(
                "README.md is out of date with {TEMPLATE_NAME} — run \
                 `cargo run -p xtask` to regenerate"
            );
        }
        println!("README.md is up to date");
        return Ok(());
    }

    std::fs::write(&output_path, rendered)
        .with_context(|| format!("could not write {}", output_path.display()))?;
    println!("wrote {}", output_path.display());
    Ok(())
}

/// Backs `{{ whittle(cmd=, format=, input=, config=, display_path=) }}`.
#[allow(clippy::needless_pass_by_value)] // Tera's Function trait requires Kwargs by value
fn whittle_fn(kwargs: Kwargs, _state: &State) -> TeraResult<String> {
    let cmd = kwargs.must_get::<&str>("cmd")?;
    let format = kwargs.get::<&str>("format")?.unwrap_or("human");
    let display_path = kwargs
        .get::<&str>("display_path")?
        .unwrap_or(DEFAULT_DISPLAY_PATH);

    let work_dir = unique_temp_dir();
    std::fs::create_dir_all(&work_dir).map_err(tera_err)?;

    let mut cargo_args: Vec<String> = vec![
        "run".into(),
        "--quiet".into(),
        "-p".into(),
        "whittle".into(),
        "--bin".into(),
        "whittle".into(),
        "--".into(),
    ];

    if let Some(config) = kwargs.get::<&str>("config")? {
        let config_path = work_dir.join("whittle.toml");
        std::fs::write(&config_path, unescape(config)).map_err(tera_err)?;
        cargo_args.push("--config".into());
        cargo_args.push(config_path.display().to_string());
    }
    if format == "json" {
        cargo_args.push("--format".into());
        cargo_args.push("json".into());
    }
    cargo_args.push(cmd.to_string());

    let real_input_path = if let Some(input) = kwargs.get::<&str>("input")? {
        let path = work_dir.join("COMMIT_EDITMSG");
        std::fs::write(&path, unescape(input)).map_err(tera_err)?;
        cargo_args.push(path.display().to_string());
        Some(path.display().to_string())
    } else {
        None
    };

    let output = Command::new("cargo")
        .args(&cargo_args)
        .output()
        .map_err(tera_err)?;
    let _ = std::fs::remove_dir_all(&work_dir);

    let mut text = String::from_utf8_lossy(&output.stdout).into_owned();
    text.push_str(&String::from_utf8_lossy(&output.stderr));
    if let Some(real_path) = real_input_path {
        text = text.replace(&real_path, display_path);
    }

    Ok(text.trim_end().to_string())
}

/// Tera string literals don't interpret `\n`; it stays as literal backslash+n.
fn unescape(s: &str) -> String {
    s.replace("\\n", "\n")
}

fn tera_err(e: impl std::fmt::Display) -> tera::Error {
    tera::Error::message(e.to_string())
}

fn unique_temp_dir() -> std::path::PathBuf {
    static COUNTER: AtomicUsize = AtomicUsize::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!("xtask-readme-{}-{n}", std::process::id()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unescape_decodes_literal_backslash_n() {
        assert_eq!(unescape("a\\nb"), "a\nb");
    }

    #[test]
    fn unescape_leaves_real_newlines_alone() {
        assert_eq!(unescape("a\nb"), "a\nb");
    }
}
