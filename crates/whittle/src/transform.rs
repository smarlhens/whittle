//! Transform engine: applies normalization rules to a parsed Conventional Commit.

use crate::config::{Config, InternalDots, Replace, TrailingDot};
use crate::diagnostic::{Diagnostic, Field};
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

impl Footer {
    /// `git_conventional` gives a bare separator + trimmed value; whitespace
    /// must be reinstated or the line stops being a git trailer. Its ref
    /// separator is `" #"` (leading space) — compare trimmed, not `"#"`.
    #[must_use]
    pub fn render(&self) -> String {
        if self.separator.trim() == "#" {
            format!("{} #{}", self.token, self.value)
        } else {
            format!("{}{} {}", self.token, self.separator.trim(), self.value)
        }
    }
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
                out.push_str(&f.render());
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

/// Applies every configured normalization, reporting each rewrite (pending —
/// see [`Diagnostic::mark_applied`]).
#[must_use]
pub fn transform(parts: &mut CommitParts, config: &Config) -> Vec<Diagnostic> {
    let mut diags = Vec::new();
    transform_type(parts, config, &mut diags);
    transform_scope(parts, config, &mut diags);
    transform_description(parts, config, &mut diags);
    transform_body(parts, config, &mut diags);
    transform_footers(parts, config, &mut diags);
    diags
}

/// Record `code` if `step` changed `value`, then keep the new value.
fn step(
    value: &mut String,
    code: &'static str,
    field: Field,
    diags: &mut Vec<Diagnostic>,
    step: impl FnOnce(&str) -> String,
) {
    let next = step(value);
    if next != *value {
        diags.push(Diagnostic::transform(code, field).rewrite(value, &next));
        *value = next;
    }
}

fn transform_type(parts: &mut CommitParts, config: &Config, diags: &mut Vec<Diagnostic>) {
    if config.type_.lowercase {
        step(
            &mut parts.type_,
            "type.lowercased",
            Field::Type,
            diags,
            str::to_lowercase,
        );
    }
}

fn transform_scope(parts: &mut CommitParts, config: &Config, diags: &mut Vec<Diagnostic>) {
    let Some(scope) = parts.scope.as_mut() else {
        return;
    };
    if config.scope.lowercase {
        step(
            scope,
            "scope.lowercased",
            Field::Scope,
            diags,
            str::to_lowercase,
        );
    }
    for r in &config.scope.replace {
        step(scope, "scope.replaced", Field::Scope, diags, |s| {
            apply_replace(s, r)
        });
    }
}

fn transform_description(parts: &mut CommitParts, config: &Config, diags: &mut Vec<Diagnostic>) {
    let cfg = &config.description;
    let mut d = parts.description.clone();

    if cfg.lowercase {
        step(
            &mut d,
            "description.lowercased",
            Field::Description,
            diags,
            str::to_lowercase,
        );
    }
    for r in &cfg.replace {
        step(
            &mut d,
            "description.replaced",
            Field::Description,
            diags,
            |s| apply_replace(s, r),
        );
    }
    if !cfg.strip_chars.is_empty() {
        step(
            &mut d,
            "description.chars-stripped",
            Field::Description,
            diags,
            |s| s.chars().filter(|c| !cfg.strip_chars.contains(c)).collect(),
        );
    }
    match cfg.internal_dots {
        InternalDots::All => {}
        InternalDots::None => {
            step(
                &mut d,
                "description.dots-stripped",
                Field::Description,
                diags,
                |s| s.replace('.', ""),
            );
        }
        InternalDots::KeepInNumbers => {
            step(
                &mut d,
                "description.dots-stripped",
                Field::Description,
                diags,
                strip_dots_outside_numbers,
            );
        }
    }
    if cfg.trailing_dot == TrailingDot::Strip {
        step(
            &mut d,
            "description.trailing-dot",
            Field::Description,
            diags,
            strip_trailing_dots,
        );
    }
    if cfg.collapse_whitespace {
        step(
            &mut d,
            "description.whitespace-collapsed",
            Field::Description,
            diags,
            collapse_whitespace,
        );
    }
    step(
        &mut d,
        "description.trimmed",
        Field::Description,
        diags,
        |s| s.trim().to_string(),
    );
    parts.description = d;
}

fn transform_body(parts: &mut CommitParts, config: &Config, diags: &mut Vec<Diagnostic>) {
    if config.body.keep {
        return;
    }
    let Some(body) = parts.body.take() else {
        return;
    };
    if body.trim().is_empty() {
        return;
    }
    let lines = body.lines().count();
    let plural = if lines == 1 { "line" } else { "lines" };
    diags.push(
        Diagnostic::transform("body.dropped", Field::Body)
            .removing(&body)
            .detail(format!("{lines} {plural}")),
    );
}

fn transform_footers(parts: &mut CommitParts, config: &Config, diags: &mut Vec<Diagnostic>) {
    if !config.footers.keep {
        if !parts.footers.is_empty() {
            let rendered = render_footers(&parts.footers);
            let count = parts.footers.len();
            diags.push(
                Diagnostic::transform("footers.dropped", Field::Footers)
                    .removing(&rendered)
                    .detail(count.to_string()),
            );
            parts.footers.clear();
        }
        return;
    }
    parts.footers.retain(|f| {
        let denied = config
            .footers
            .deny
            .iter()
            .any(|d| d.eq_ignore_ascii_case(&f.token));
        if denied {
            diags.push(
                Diagnostic::transform("footer.denied", Field::Footers)
                    .removing(&f.render())
                    .detail(f.token.clone()),
            );
        }
        !denied
    });
}

fn render_footers(footers: &[Footer]) -> String {
    footers
        .iter()
        .map(Footer::render)
        .collect::<Vec<_>>()
        .join("\n")
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

pub(crate) fn strip_dots_outside_numbers(s: &str) -> String {
    let chars: Vec<char> = s.chars().collect();
    let mut out = String::with_capacity(s.len());
    for (i, ch) in chars.iter().enumerate() {
        if *ch == '.' {
            if is_dot_between_digits(&chars, i) {
                out.push('.');
            }
        } else {
            out.push(*ch);
        }
    }
    out
}

/// `pub(crate)`: `guidance::mangled_description` reuses this exact rule.
pub(crate) fn is_dot_between_digits(chars: &[char], i: usize) -> bool {
    let prev = i.checked_sub(1).and_then(|j| chars.get(j));
    let next = chars.get(i + 1);
    matches!(prev, Some(c) if c.is_ascii_digit()) && matches!(next, Some(c) if c.is_ascii_digit())
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
        let _diags = transform(&mut parts, &defaults());
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
    fn footer_render_reinstates_space_after_colon() {
        let f = Footer {
            token: "Co-Authored-By".into(),
            separator: ":".into(),
            value: "Claude <a@b>".into(),
        };
        assert_eq!(f.render(), "Co-Authored-By: Claude <a@b>");
    }

    #[test]
    fn footer_render_keeps_hash_separator_style() {
        let p = CommitParts::parse("feat: x\n\nRefs #12").unwrap();
        let f = p.footers.first().expect("one footer");
        assert_eq!(f.separator, " #", "parser separator shape changed");
        assert_eq!(f.render(), "Refs #12");
    }

    #[test]
    fn ref_style_footers_survive_a_keep_round_trip() {
        let raw = "feat: x\n\nbody\n\nCloses #128\nRefs #12";
        let mut cfg = Config::default();
        cfg.body.keep = true;
        cfg.footers.keep = true;
        cfg.footers.deny = vec![];
        let mut p = CommitParts::parse(raw).unwrap();
        let _diags = transform(&mut p, &cfg);
        let out = p.render();
        assert!(out.contains("Closes #128"), "got: {out}");
        assert!(out.contains("Refs #12"), "got: {out}");
        assert!(!out.contains("# 12"), "extra space injected: {out}");
    }

    #[test]
    fn denied_footer_diagnostic_quotes_a_valid_trailer() {
        let raw = "feat: x\n\nCo-Authored-By: Claude <a@b>";
        let mut cfg = Config::default();
        cfg.footers.keep = true;
        let mut p = CommitParts::parse(raw).unwrap();
        let diags = transform(&mut p, &cfg);
        let d = diags
            .iter()
            .find(|d| d.code == "footer.denied")
            .expect("footer.denied");
        assert_eq!(d.before.as_deref(), Some("Co-Authored-By: Claude <a@b>"));
        assert_eq!(d.detail.as_deref(), Some("Co-Authored-By"));
    }

    #[test]
    fn dropped_footers_diagnostic_quotes_valid_trailers() {
        let raw = "feat: x\n\nCo-Authored-By: Claude <a@b>\nRefs: #12";
        let mut p = CommitParts::parse(raw).unwrap();
        let diags = transform(&mut p, &defaults());
        let d = diags
            .iter()
            .find(|d| d.code == "footers.dropped")
            .expect("footers.dropped");
        assert_eq!(
            d.before.as_deref(),
            Some("Co-Authored-By: Claude <a@b>\nRefs: #12")
        );
    }

    #[test]
    fn kept_footers_round_trip_as_valid_trailers() {
        let raw = "feat: x\n\nbody\n\nCo-Authored-By: Claude <a@b>\nRefs: #12";
        let mut cfg = Config::default();
        cfg.body.keep = true;
        cfg.footers.keep = true;
        cfg.footers.deny = vec![];
        let mut p = CommitParts::parse(raw).unwrap();
        let _diags = transform(&mut p, &cfg);
        let out = p.render();
        assert!(out.contains("Co-Authored-By: Claude <a@b>"), "got: {out}");
        assert!(out.contains("Refs: #12"), "got: {out}");
    }

    #[test]
    fn empty_body_produces_no_dropped_diagnostic() {
        let raw = "feat: x\n\n \n\nRefs: #1";
        let mut p = CommitParts::parse(raw).unwrap();
        let diags = transform(&mut p, &defaults());
        assert!(
            !diags.iter().any(|d| d.code == "body.dropped"),
            "whitespace-only body has nothing to report as removed"
        );
    }

    #[test]
    fn type_lowercasing_is_independent_of_scope_lowercasing() {
        let mut cfg = Config::default();
        cfg.scope.lowercase = false;
        let mut p = CommitParts::parse("Chore(API): Bump Thing").unwrap();
        let diags = transform(&mut p, &cfg);
        assert_eq!(p.type_, "chore");
        assert_eq!(p.scope.as_deref(), Some("API"));
        assert!(diags.iter().any(|d| d.code == "type.lowercased"));
    }

    #[test]
    fn type_lowercasing_can_be_disabled_on_its_own() {
        let mut cfg = Config::default();
        cfg.type_.lowercase = false;
        let mut p = CommitParts::parse("Chore: Bump Thing").unwrap();
        let diags = transform(&mut p, &cfg);
        assert_eq!(p.type_, "Chore");
        assert!(!diags.iter().any(|d| d.code == "type.lowercased"));
    }

    #[test]
    fn type_diagnostic_precedes_description_diagnostics() {
        let mut p = CommitParts::parse("Chore: Bump Thing").unwrap();
        let diags = transform(&mut p, &defaults());
        let first = diags.first().expect("at least one diagnostic");
        assert_eq!(first.code, "type.lowercased");
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
        let _diags = transform(&mut p, &cfg);
        assert!(p.render().contains("body text"));
    }

    #[test]
    fn transform_keeps_non_denied_footers_when_configured() {
        let mut cfg = Config::default();
        cfg.footers.keep = true;
        cfg.footers.deny = vec!["Co-Authored-By".into()];
        let raw = "feat: x\n\nbody\n\nCo-Authored-By: a <a@x>\nRefs: #1";
        let mut p = CommitParts::parse(raw).unwrap();
        let _diags = transform(&mut p, &cfg);
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
        let _diags = transform(&mut p, &cfg);
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
        let _diags = transform(&mut p, &cfg);
        assert_eq!(p.description, "bump 123");
    }

    #[test]
    fn transform_trailing_dot_keep() {
        let mut cfg = Config::default();
        cfg.description.trailing_dot = TrailingDot::Keep;
        cfg.description.internal_dots = InternalDots::All;
        let mut p = CommitParts::parse("chore: foo.").unwrap();
        let _diags = transform(&mut p, &cfg);
        assert_eq!(p.description, "foo.");
    }

    #[test]
    fn transform_lowercase_disabled_keeps_case() {
        let mut cfg = Config::default();
        cfg.description.lowercase = false;
        cfg.scope.lowercase = false;
        let mut p = CommitParts::parse("Fix(API): Handle Null").unwrap();
        let _diags = transform(&mut p, &cfg);
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
        let _diags = transform(&mut p, &cfg);
        assert_eq!(p.description, "bar here");
    }

    #[test]
    fn transform_empty_strip_chars_is_noop() {
        let mut cfg = Config::default();
        cfg.description.strip_chars = vec![];
        let mut p = CommitParts::parse("fix: [keep] /these\\").unwrap();
        let _diags = transform(&mut p, &cfg);
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
            let _diags = transform(&mut p, &defaults());
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
