//! Transform engine: applies normalization rules to a parsed Conventional Commit.

use crate::config::{Config, InternalDots, Replace, TrailingDot};
use regex::Regex;

#[derive(Debug, Clone)]
pub struct CommitParts {
    pub type_: String,
    pub scope: Option<String>,
    pub breaking: bool,
    pub description: String,
    pub body: Option<String>,
    pub footers: Vec<Footer>,
}

#[derive(Debug, Clone)]
pub struct Footer {
    pub token: String,
    pub separator: String,
    pub value: String,
}

impl CommitParts {
    /// # Errors
    /// Returns an error if `raw` is not a valid Conventional Commit message.
    pub fn parse(raw: &str) -> anyhow::Result<Self> {
        let commit = git_conventional::Commit::parse(raw.trim_end())
            .map_err(|e| anyhow::anyhow!("not a conventional commit: {e}"))?;
        Ok(Self {
            type_: commit.type_().as_str().to_string(),
            scope: commit.scope().map(|s| s.as_str().to_string()),
            breaking: commit.breaking(),
            description: commit.description().to_string(),
            body: commit.body().map(std::string::ToString::to_string),
            footers: commit
                .footers()
                .iter()
                .map(|f| Footer {
                    token: f.token().to_string(),
                    separator: f.separator().to_string(),
                    value: f.value().to_string(),
                })
                .collect(),
        })
    }

    #[must_use]
    pub fn render(&self) -> String {
        let mut out = self.type_.clone();
        if let Some(scope) = &self.scope {
            out.push('(');
            out.push_str(scope);
            out.push(')');
        }
        if self.breaking {
            out.push('!');
        }
        out.push_str(": ");
        out.push_str(&self.description);
        if let Some(body) = &self.body {
            out.push_str("\n\n");
            out.push_str(body);
        }
        if !self.footers.is_empty() {
            out.push_str("\n\n");
            for (i, f) in self.footers.iter().enumerate() {
                if i > 0 {
                    out.push('\n');
                }
                out.push_str(&f.token);
                out.push_str(&f.separator);
                out.push_str(&f.value);
            }
        }
        out
    }

    #[must_use]
    pub fn subject(&self) -> String {
        let mut s = self.type_.clone();
        if let Some(scope) = &self.scope {
            s.push('(');
            s.push_str(scope);
            s.push(')');
        }
        if self.breaking {
            s.push('!');
        }
        s.push_str(": ");
        s.push_str(&self.description);
        s
    }
}

pub fn transform(parts: &mut CommitParts, config: &Config) {
    transform_scope(parts, config);
    transform_description(parts, config);
    transform_body(parts, config);
    transform_footers(parts, config);
}

fn transform_scope(parts: &mut CommitParts, config: &Config) {
    let Some(scope) = parts.scope.as_mut() else {
        return;
    };
    if config.scope.lowercase {
        *scope = scope.to_lowercase();
    }
    for r in &config.scope.replace {
        *scope = apply_replace(scope, r);
    }
}

fn transform_description(parts: &mut CommitParts, config: &Config) {
    let cfg = &config.description;
    let mut d = parts.description.clone();

    if cfg.lowercase {
        d = d.to_lowercase();
    }
    for r in &cfg.replace {
        d = apply_replace(&d, r);
    }
    if !cfg.strip_chars.is_empty() {
        d = d.chars().filter(|c| !cfg.strip_chars.contains(c)).collect();
    }
    match cfg.internal_dots {
        InternalDots::All => {
            // dots already handled in trailing step below; here keep all
        }
        InternalDots::None => {
            d = d.replace('.', "");
        }
        InternalDots::KeepInNumbers => {
            d = strip_dots_outside_numbers(&d);
        }
    }
    if cfg.trailing_dot == TrailingDot::Strip {
        d = strip_trailing_dots(&d);
    }
    if cfg.collapse_whitespace {
        d = collapse_whitespace(&d);
    }
    parts.description = d.trim().to_string();

    if config.scope.lowercase {
        parts.type_ = parts.type_.to_lowercase();
    }
}

fn transform_body(parts: &mut CommitParts, config: &Config) {
    if !config.body.keep {
        parts.body = None;
    }
}

fn transform_footers(parts: &mut CommitParts, config: &Config) {
    if !config.footers.keep {
        parts.footers.clear();
        return;
    }
    parts.footers.retain(|f| {
        !config
            .footers
            .deny
            .iter()
            .any(|d| d.eq_ignore_ascii_case(&f.token))
    });
}

fn apply_replace(input: &str, r: &Replace) -> String {
    if r.regex {
        match Regex::new(&r.from) {
            Ok(re) => re.replace_all(input, r.to.as_str()).into_owned(),
            Err(_) => input.to_string(),
        }
    } else {
        input.replace(&r.from, &r.to)
    }
}

fn collapse_whitespace(s: &str) -> String {
    let re = Regex::new(r"\s+").expect("static regex");
    re.replace_all(s, " ").into_owned()
}

fn strip_trailing_dots(s: &str) -> String {
    s.trim_end_matches('.').to_string()
}

fn strip_dots_outside_numbers(s: &str) -> String {
    // Strip `.` unless both neighbours are ASCII digits.
    let chars: Vec<char> = s.chars().collect();
    let mut out = String::with_capacity(s.len());
    for (i, ch) in chars.iter().enumerate() {
        if *ch == '.' {
            let prev = i.checked_sub(1).and_then(|j| chars.get(j));
            let next = chars.get(i + 1);
            let between_digits = matches!(prev, Some(c) if c.is_ascii_digit())
                && matches!(next, Some(c) if c.is_ascii_digit());
            if between_digits {
                out.push('.');
            }
        } else {
            out.push(*ch);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;
    use crate::config::Config;

    fn defaults() -> Config {
        Config::default()
    }

    fn fix(raw: &str) -> String {
        let mut parts = CommitParts::parse(raw).expect("parse");
        transform(&mut parts, &defaults());
        parts.render()
    }

    #[test]
    fn collapse_whitespace_collapses_runs() {
        assert_eq!(collapse_whitespace("a   b\t\tc\n\nd"), "a b c d");
    }

    #[test]
    fn collapse_whitespace_no_change_for_single_spaces() {
        assert_eq!(collapse_whitespace("a b c"), "a b c");
    }

    #[test]
    fn strip_trailing_dots_removes_multiple() {
        assert_eq!(strip_trailing_dots("foo..."), "foo");
        assert_eq!(strip_trailing_dots("foo"), "foo");
        assert_eq!(strip_trailing_dots(""), "");
    }

    #[test]
    fn strip_dots_outside_numbers_keeps_version() {
        assert_eq!(
            strip_dots_outside_numbers("bump 1.2.3 yes"),
            "bump 1.2.3 yes"
        );
    }

    #[test]
    fn strip_dots_outside_numbers_strips_between_letters() {
        assert_eq!(strip_dots_outside_numbers("readme.md"), "readmemd");
    }

    #[test]
    fn strip_dots_outside_numbers_strips_at_edges() {
        assert_eq!(strip_dots_outside_numbers(".foo."), "foo");
    }

    #[test]
    fn strip_dots_outside_numbers_strips_digit_then_letter() {
        // `0.x` -> 0 then `.` then letter — neighbour not digit on right side
        assert_eq!(strip_dots_outside_numbers("v0.x"), "v0x");
    }

    #[test]
    fn apply_replace_regex_word_boundary() {
        let r = Replace {
            from: r"\band\b".into(),
            to: "&".into(),
            regex: true,
        };
        assert_eq!(apply_replace("foo and bar", &r), "foo & bar");
        assert_eq!(apply_replace("band aid", &r), "band aid");
        assert_eq!(apply_replace("Land", &r), "Land");
    }

    #[test]
    fn apply_replace_literal() {
        let r = Replace {
            from: "/".into(),
            to: "-".into(),
            regex: false,
        };
        assert_eq!(apply_replace("a/b/c", &r), "a-b-c");
    }

    #[test]
    fn apply_replace_invalid_regex_falls_through() {
        let r = Replace {
            from: "[invalid".into(),
            to: "x".into(),
            regex: true,
        };
        assert_eq!(apply_replace("hello", &r), "hello");
    }

    #[test]
    fn parse_simple_conventional() {
        let p = CommitParts::parse("feat: add thing").unwrap();
        assert_eq!(p.type_, "feat");
        assert_eq!(p.scope, None);
        assert!(!p.breaking);
        assert_eq!(p.description, "add thing");
    }

    #[test]
    fn parse_with_scope() {
        let p = CommitParts::parse("fix(api): handle null").unwrap();
        assert_eq!(p.scope.as_deref(), Some("api"));
    }

    #[test]
    fn parse_breaking_bang() {
        let p = CommitParts::parse("feat!: drop legacy api").unwrap();
        assert!(p.breaking);
    }

    #[test]
    fn parse_breaking_via_footer() {
        let raw = "feat: rework api\n\nBREAKING CHANGE: clients must migrate";
        let p = CommitParts::parse(raw).unwrap();
        assert!(p.breaking);
    }

    #[test]
    fn parse_with_body_and_footers() {
        let raw =
            "feat(api): add probe\n\nLonger explanation.\n\nCo-Authored-By: A <a@x>\nRefs: #42";
        let p = CommitParts::parse(raw).unwrap();
        assert_eq!(p.body.as_deref(), Some("Longer explanation."));
        assert_eq!(p.footers.len(), 2);
        assert_eq!(p.footers[0].token, "Co-Authored-By");
    }

    #[test]
    fn parse_non_conventional_errors() {
        assert!(CommitParts::parse("just words").is_err());
    }

    #[test]
    fn render_preserves_structure() {
        let raw = "feat(api): add probe\n\nbody";
        let p = CommitParts::parse(raw).unwrap();
        let rendered = p.render();
        assert!(rendered.starts_with("feat(api): add probe"));
        assert!(rendered.contains("body"));
    }

    #[test]
    fn render_breaking_bang_included() {
        let p = CommitParts {
            type_: "feat".into(),
            scope: None,
            breaking: true,
            description: "x".into(),
            body: None,
            footers: vec![],
        };
        assert_eq!(p.render(), "feat!: x");
    }

    #[test]
    fn subject_combines_components() {
        let p = CommitParts {
            type_: "fix".into(),
            scope: Some("api".into()),
            breaking: false,
            description: "foo".into(),
            body: None,
            footers: vec![],
        };
        assert_eq!(p.subject(), "fix(api): foo");
    }

    #[test]
    fn transform_scope_lowercases_and_replaces_slash() {
        let out = fix("fix(API/Users): NULL");
        assert_eq!(out, "fix(api-users): null");
    }

    #[test]
    fn transform_description_strips_brackets() {
        let out = fix("fix: a [b] c {d} e");
        assert_eq!(out, "fix: a b c d e");
    }

    #[test]
    fn transform_description_strips_backslash() {
        let out = fix(r"fix: path a\b\c");
        assert_eq!(out, "fix: path abc");
    }

    #[test]
    fn transform_description_collapses_whitespace() {
        let out = fix("fix:   foo    bar");
        assert_eq!(out, "fix: foo bar");
    }

    #[test]
    fn transform_description_strips_trailing_dot() {
        let out = fix("fix: foo.");
        assert_eq!(out, "fix: foo");
    }

    #[test]
    fn transform_description_keeps_version_dots() {
        let out = fix("chore: bump foo 1.2.3");
        assert_eq!(out, "chore: bump foo 1.2.3");
    }

    #[test]
    fn transform_description_strips_internal_non_version_dot() {
        let out = fix("docs: update readme.md");
        assert_eq!(out, "docs: update readmemd");
    }

    #[test]
    fn transform_drops_body_by_default() {
        let out = fix("feat: x\n\nbody text here");
        assert_eq!(out, "feat: x");
    }

    #[test]
    fn transform_drops_all_footers_by_default() {
        let out = fix("feat: x\n\nbody\n\nRefs: #1\nReviewed-by: alice");
        assert_eq!(out, "feat: x");
    }

    #[test]
    fn transform_keeps_body_when_configured() {
        let mut cfg = Config::default();
        cfg.body.keep = true;
        let mut p = CommitParts::parse("feat: x\n\nbody text").unwrap();
        transform(&mut p, &cfg);
        assert!(p.render().contains("body text"));
    }

    #[test]
    fn transform_keeps_non_denied_footers_when_configured() {
        let mut cfg = Config::default();
        cfg.footers.keep = true;
        cfg.footers.deny = vec!["Co-Authored-By".into()];
        let raw = "feat: x\n\nbody\n\nCo-Authored-By: a <a@x>\nRefs: #1";
        let mut p = CommitParts::parse(raw).unwrap();
        transform(&mut p, &cfg);
        let rendered = p.render();
        assert!(!rendered.contains("Co-Authored-By"));
        assert!(rendered.contains("Refs"));
    }

    #[test]
    fn transform_footer_deny_is_case_insensitive() {
        let mut cfg = Config::default();
        cfg.footers.keep = true;
        cfg.footers.deny = vec!["co-authored-by".into()];
        let raw = "feat: x\n\nbody\n\nCo-Authored-By: a <a@x>";
        let mut p = CommitParts::parse(raw).unwrap();
        transform(&mut p, &cfg);
        assert!(!p.render().contains("Co-Authored-By"));
    }

    #[test]
    fn transform_handles_breaking_bang() {
        let out = fix("Feat!: Drop Legacy API");
        assert_eq!(out, "feat!: drop legacy api");
    }

    #[test]
    fn transform_handles_breaking_with_scope() {
        let out = fix("Feat(API)!: Drop /v1");
        assert_eq!(out, "feat(api)!: drop v1");
    }

    #[test]
    fn transform_lowercases_uppercase_and() {
        let out = fix("Chore: A AND B");
        assert_eq!(out, "chore: a & b");
    }

    #[test]
    fn transform_does_not_replace_inner_and() {
        let out = fix("fix: handle band aid");
        assert_eq!(out, "fix: handle band aid");
    }

    #[test]
    fn transform_internal_dots_none_strips_all() {
        let mut cfg = Config::default();
        cfg.description.internal_dots = InternalDots::None;
        let mut p = CommitParts::parse("chore: bump 1.2.3").unwrap();
        transform(&mut p, &cfg);
        assert_eq!(p.description, "bump 123");
    }

    #[test]
    fn transform_trailing_dot_keep() {
        let mut cfg = Config::default();
        cfg.description.trailing_dot = TrailingDot::Keep;
        // disable internal dot stripping so trailing isn't removed via that path
        cfg.description.internal_dots = InternalDots::All;
        let mut p = CommitParts::parse("chore: foo.").unwrap();
        transform(&mut p, &cfg);
        assert_eq!(p.description, "foo.");
    }

    #[test]
    fn transform_lowercase_disabled_keeps_case() {
        let mut cfg = Config::default();
        cfg.description.lowercase = false;
        cfg.scope.lowercase = false;
        let mut p = CommitParts::parse("Fix(API): Handle Null").unwrap();
        transform(&mut p, &cfg);
        // type is only lowercased when scope.lowercase is true (current impl)
        assert_eq!(p.scope.as_deref(), Some("API"));
        assert_eq!(p.description, "Handle Null");
    }

    #[test]
    fn transform_multiple_replace_rules() {
        let mut cfg = Config::default();
        cfg.description.replace = vec![
            Replace {
                from: "foo".into(),
                to: "FOO".into(),
                regex: false,
            },
            Replace {
                from: "FOO".into(),
                to: "bar".into(),
                regex: false,
            },
        ];
        let mut p = CommitParts::parse("fix: foo here").unwrap();
        transform(&mut p, &cfg);
        // replacements run in order; second sees output of first
        assert_eq!(p.description, "bar here");
    }

    #[test]
    fn transform_empty_strip_chars_is_noop() {
        let mut cfg = Config::default();
        cfg.description.strip_chars = vec![];
        let mut p = CommitParts::parse("fix: [keep] /these\\").unwrap();
        transform(&mut p, &cfg);
        assert!(p.description.contains('['));
        assert!(p.description.contains('/'));
    }

    #[test]
    fn transform_scope_with_multiple_slashes() {
        let out = fix("feat(a/b/c/d): x");
        assert_eq!(out, "feat(a-b-c-d): x");
    }

    #[test]
    fn transform_idempotent() {
        let out1 = fix("Chore: Bump A and B.");
        let out2 = {
            let mut p = CommitParts::parse(&out1).unwrap();
            transform(&mut p, &defaults());
            p.render()
        };
        assert_eq!(out1, out2);
    }

    #[test]
    fn transform_preserves_unicode() {
        let out = fix("feat: café résumé");
        assert_eq!(out, "feat: café résumé");
    }
}
