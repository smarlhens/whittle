//! Lint diagnostics for parsed conventional commits.

use crate::config::Config;
use crate::transform::CommitParts;

#[derive(Debug, Clone)]
pub struct Diagnostic {
    pub code: &'static str,
    pub message: String,
}

#[must_use]
pub fn lint(parts: &CommitParts, config: &Config) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    let rules = &config.rules;

    if !rules.allowed_types.is_empty()
        && !rules
            .allowed_types
            .iter()
            .any(|t| t.eq_ignore_ascii_case(&parts.type_))
    {
        out.push(Diagnostic {
            code: "disallowed-type",
            message: format!(
                "type `{}` is not in allowed_types ({})",
                parts.type_,
                rules.allowed_types.join(", ")
            ),
        });
    }

    let subject_len = parts.subject().chars().count();
    if subject_len > rules.max_subject_length {
        out.push(Diagnostic {
            code: "subject-too-long",
            message: format!(
                "subject is {subject_len} chars; max allowed is {}",
                rules.max_subject_length
            ),
        });
    }

    out
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;
    use crate::config::Config;
    use crate::transform::CommitParts;

    fn parse(raw: &str) -> CommitParts {
        CommitParts::parse(raw).expect("parse")
    }

    #[test]
    fn allowed_type_passes() {
        let p = parse("feat: x");
        let diags = lint(&p, &Config::default());
        assert!(diags.is_empty());
    }

    #[test]
    fn disallowed_type_fails() {
        let p = parse("wip: hack");
        let diags = lint(&p, &Config::default());
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].code, "disallowed-type");
    }

    #[test]
    fn empty_allowed_types_skips_check() {
        let mut cfg = Config::default();
        cfg.rules.allowed_types = vec![];
        let p = parse("wip: hack");
        let diags = lint(&p, &cfg);
        assert!(diags.iter().all(|d| d.code != "disallowed-type"));
    }

    #[test]
    fn allowed_types_case_insensitive() {
        let mut cfg = Config::default();
        cfg.rules.allowed_types = vec!["FEAT".into()];
        let p = parse("feat: x");
        assert!(lint(&p, &cfg).is_empty());
    }

    #[test]
    fn exact_max_subject_length_passes() {
        let desc: String = "x".repeat(72 - "feat: ".len());
        let raw = format!("feat: {desc}");
        let p = parse(&raw);
        assert_eq!(p.subject().chars().count(), 72);
        assert!(lint(&p, &Config::default()).is_empty());
    }

    #[test]
    fn one_over_max_subject_length_fails() {
        let desc: String = "x".repeat(73 - "feat: ".len());
        let raw = format!("feat: {desc}");
        let p = parse(&raw);
        let diags = lint(&p, &Config::default());
        assert!(diags.iter().any(|d| d.code == "subject-too-long"));
    }

    #[test]
    fn subject_length_counts_unicode_scalars_not_bytes() {
        // each `é` is 2 bytes, 1 scalar
        let desc: String = "é".repeat(60);
        let raw = format!("feat: {desc}");
        let p = parse(&raw);
        let subject_chars = p.subject().chars().count();
        assert_eq!(subject_chars, 66);
        assert!(lint(&p, &Config::default()).is_empty());
    }

    #[test]
    fn multiple_diagnostics_reported() {
        let mut cfg = Config::default();
        cfg.rules.max_subject_length = 5;
        cfg.rules.allowed_types = vec!["fix".into()];
        let p = parse("feat: too long for sure");
        let diags = lint(&p, &cfg);
        let codes: Vec<&str> = diags.iter().map(|d| d.code).collect();
        assert!(codes.contains(&"disallowed-type"));
        assert!(codes.contains(&"subject-too-long"));
    }
}
