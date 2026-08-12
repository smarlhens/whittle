//! Lint diagnostics for parsed conventional commits.

use crate::config::Config;
use crate::diagnostic::{Diagnostic, Field};
use crate::transform::CommitParts;

/// Check `parts` against the configured rules.
///
/// Every diagnostic returned is [`crate::diagnostic::Severity::Error`]: unlike a
/// transform, no rewrite whittle can perform will satisfy these.
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
        let allowed = rules.allowed_types.join(", ");
        out.push(
            Diagnostic::error("type.disallowed", Field::Type)
                .removing(&parts.type_)
                .detail(allowed.clone())
                .message_override(format!(
                    "type `{}` is not in allowed_types ({allowed})",
                    parts.type_
                )),
        );
    }

    let subject = parts.subject();
    let subject_len = subject.chars().count();
    if subject_len > rules.max_subject_length {
        out.push(
            Diagnostic::error("subject.too-long", Field::Subject)
                .removing(&subject)
                .detail(format!("{subject_len}/{}", rules.max_subject_length))
                .message_override(format!(
                    "subject is {subject_len} chars; max allowed is {}",
                    rules.max_subject_length
                )),
        );
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
        assert_eq!(diags[0].code, "type.disallowed");
    }

    #[test]
    fn empty_allowed_types_skips_check() {
        let mut cfg = Config::default();
        cfg.rules.allowed_types = vec![];
        let p = parse("wip: hack");
        let diags = lint(&p, &cfg);
        assert!(diags.iter().all(|d| d.code != "type.disallowed"));
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
        assert!(diags.iter().any(|d| d.code == "subject.too-long"));
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
        assert!(codes.contains(&"type.disallowed"));
        assert!(codes.contains(&"subject.too-long"));
    }
}
