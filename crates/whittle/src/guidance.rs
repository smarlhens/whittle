//! Turns diagnostics into instructions: what to write instead, not just what changed.

use crate::config::{Config, DescriptionConfig, InternalDots, Replace, TrailingDot};
use crate::diagnostic::Diagnostic;

/// Instructions for the rules `diags` tripped, in a stable order and deduped.
#[must_use]
pub fn guidance(diags: &[Diagnostic], config: &Config) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut push = |line: String| {
        if !out.contains(&line) {
            out.push(line);
        }
    };

    for d in diags {
        match d.code {
            "commit.not-conventional" => push(
                "write the subject as a Conventional Commit: `type(scope): description`, \
                 e.g. `fix(api): handle null ids`"
                    .to_string(),
            ),
            "type.disallowed" => push(format!(
                "use one of these types: {}",
                config.rules.allowed_types.join(", ")
            )),
            "subject.too-long" => push(format!(
                "keep the whole subject line to {} characters or fewer",
                config.rules.max_subject_length
            )),
            "type.lowercased" => push("write the type in lowercase".to_string()),
            "scope.lowercased" => push("write the scope in lowercase".to_string()),
            "scope.replaced" => {
                if let (Some(b), Some(a)) = (&d.before, &d.after) {
                    push(format!(
                        "in the scope, `{b}` becomes `{a}` — write the result directly"
                    ));
                }
            }
            "description.lowercased" => {
                push("write the description in lowercase".to_string());
            }
            "description.replaced" => {
                if let (Some(b), Some(a)) = (&d.before, &d.after) {
                    push(format!(
                        "in the description, `{b}` becomes `{a}` — write the result directly"
                    ));
                }
            }
            "description.chars-stripped" => {
                push(strip_chars_line(&config.description.strip_chars));
            }
            "description.dots-stripped" => push(dots_rule(config)),
            "description.trailing-dot" => {
                push("do not end the subject with a `.`".to_string());
            }
            "description.whitespace-collapsed" => {
                push("separate words with a single space".to_string());
            }
            "description.trimmed" => {
                push("no leading or trailing space in the description".to_string());
            }
            "body.dropped" => push(
                "the body is discarded — put everything that matters in the subject line"
                    .to_string(),
            ),
            "footers.dropped" => push(footers_dropped_line()),
            "footer.denied" => push(denied_footers_line(&config.footers.deny)),
            "normalize.invalid-result" => push(
                "write a description of plain words: after normalization it must not be empty"
                    .to_string(),
            ),
            "subject.needs-rewording" => push(
                "say the same thing in words rather than symbols: name the thing, do not paste \
                 its path, filename or package spec"
                    .to_string(),
            ),
            "message.reformatted" => push(
                "adopt the suggested message below as-is — its formatting (blank lines, footer \
                 spacing, breaking-change marker) is corrected structurally, not by any rule above"
                    .to_string(),
            ),
            _ => {}
        }
    }
    out
}

/// Above this, skip analysis — `subject.too-long` already rejects it.
const MAX_MANGLING_CHECK_LEN: usize = 500;

/// Tokens that would come out fused with their neighbour, as `(before, after)`.
/// Only checks `strip_chars`/`internal_dots`, never `.replace` — a `.replace`
/// rule's `to` value is the author's own deliberate choice.
#[must_use]
pub fn mangled_description(description: &str, config: &DescriptionConfig) -> Vec<(String, String)> {
    if description.len() > MAX_MANGLING_CHECK_LEN {
        return Vec::new();
    }
    description
        .split_whitespace()
        .filter_map(|token| mangled_token(token, config))
        .collect()
}

fn mangled_token(token: &str, config: &DescriptionConfig) -> Option<(String, String)> {
    let after_strip: String = if config.strip_chars.is_empty() {
        token.to_string()
    } else {
        token
            .chars()
            .filter(|c| !config.strip_chars.contains(c))
            .collect()
    };
    let strip_fuses = after_strip != token && deletion_fuses_words(token, &after_strip);

    let after_dots = match config.internal_dots {
        InternalDots::All => after_strip.clone(),
        InternalDots::None => after_strip.replace('.', ""),
        InternalDots::KeepInNumbers => crate::transform::strip_dots_outside_numbers(&after_strip),
    };
    let dots_fuses = after_dots != after_strip && deletion_fuses_words(&after_strip, &after_dots);

    (strip_fuses || dots_fuses).then(|| (token.to_string(), after_dots))
}

/// Whether the deletion placed two real words (alphabetic runs of 2+ chars,
/// digits excluded) directly adjacent. `before`/`after` must be a
/// deletion-only transform (see `mangled_description`).
fn deletion_fuses_words(before: &str, after: &str) -> bool {
    let b: Vec<char> = before.chars().collect();
    let a: Vec<char> = after.chars().collect();
    let mut ai = 0;
    let mut i = 0;
    while i < b.len() {
        if ai < a.len() && b[i] == a[ai] {
            ai += 1;
            i += 1;
            continue;
        }
        while i < b.len() && !(ai < a.len() && b[i] == a[ai]) {
            i += 1;
        }
        if word_run_len_before(&a, ai) >= 2 && word_run_len_after(&a, ai) >= 2 {
            return true;
        }
    }
    false
}

fn word_run_len_before(a: &[char], at: usize) -> usize {
    a[..at]
        .iter()
        .rev()
        .take_while(|&&c| is_word_char(c))
        .count()
}

fn word_run_len_after(a: &[char], at: usize) -> usize {
    a[at..].iter().take_while(|&&c| is_word_char(c)).count()
}

/// Alphabetic, or a combining mark (so decomposed Unicode reads as one letter).
/// Digits excluded: a digit run fusing with a suffix (`24.x` -> `24x`) is fine.
fn is_word_char(c: char) -> bool {
    c.is_alphabetic() || is_combining_mark(c)
}

fn is_combining_mark(c: char) -> bool {
    matches!(c as u32, 0x0300..=0x036F | 0x1AB0..=0x1AFF | 0x1DC0..=0x1DFF | 0x20D0..=0x20FF | 0xFE20..=0xFE2F)
}

/// Describe a `replace` table, or `None` when it is empty.
fn replace_rule_line(field: &str, rules: &[Replace]) -> Option<String> {
    if rules.is_empty() {
        return None;
    }
    let pairs = rules
        .iter()
        .map(|r| {
            if r.regex {
                format!("the pattern `{}` becomes `{}`", r.from, r.to)
            } else {
                format!("`{}` becomes `{}`", r.from, r.to)
            }
        })
        .collect::<Vec<_>>()
        .join(", ");
    Some(format!(
        "in the {field}, {pairs} — write the result directly"
    ))
}

fn footers_dropped_line() -> String {
    "all footers are discarded, including `Co-Authored-By` — do not rely on trailers".to_string()
}

fn denied_footers_line(deny: &[String]) -> String {
    if deny.is_empty() {
        return footers_dropped_line();
    }
    format!("these footers are discarded: {}", deny.join(", "))
}

fn strip_chars_line(chars: &[char]) -> String {
    format!(
        "do not use these characters in the description: {}",
        chars
            .iter()
            .map(|c| format!("`{c}`"))
            .collect::<Vec<_>>()
            .join(" ")
    )
}

fn dots_rule(config: &Config) -> String {
    match config.description.internal_dots {
        InternalDots::None => {
            "do not use `.` in the description at all, including at the end — write file names \
             without their extension"
                .to_string()
        }
        InternalDots::KeepInNumbers => {
            "use `.` only between digits, as in `1.2.3`, and never at the end of the subject — \
             write file names without their extension, e.g. `release workflow` not `release.yml`"
                .to_string()
        }
        InternalDots::All => "dots are kept in the description".to_string(),
    }
}

/// The full active ruleset, for `whittle rules`.
#[must_use]
pub fn rules(config: &Config) -> Vec<String> {
    let mut out = Vec::new();
    if config.rules.require_conventional {
        out.push(
            "write the subject as a Conventional Commit: `type(scope): description`".to_string(),
        );
    }
    if !config.rules.allowed_types.is_empty() {
        out.push(format!(
            "use one of these types: {}",
            config.rules.allowed_types.join(", ")
        ));
    }
    out.push(format!(
        "keep the whole subject line to {} characters or fewer",
        config.rules.max_subject_length
    ));
    if config.type_.lowercase {
        out.push("write the type in lowercase".to_string());
    }
    if config.scope.lowercase {
        out.push("write the scope in lowercase".to_string());
    }
    if let Some(line) = replace_rule_line("scope", &config.scope.replace) {
        out.push(line);
    }
    if config.description.lowercase {
        out.push("write the description in lowercase".to_string());
    }
    if let Some(line) = replace_rule_line("description", &config.description.replace) {
        out.push(line);
    }
    if !config.description.strip_chars.is_empty() {
        out.push(strip_chars_line(&config.description.strip_chars));
    }
    out.push(dots_rule(config));
    if config.description.trailing_dot == TrailingDot::Strip
        && config.description.internal_dots == InternalDots::All
    {
        out.push("do not end the subject with a `.`".to_string());
    }
    if config.description.collapse_whitespace {
        out.push("separate words with a single space".to_string());
    }
    out.push("no leading or trailing space in the description".to_string());
    if !config.body.keep {
        out.push(
            "the body is discarded — put everything that matters in the subject line".to_string(),
        );
    }
    if config.footers.keep {
        if !config.footers.deny.is_empty() {
            out.push(denied_footers_line(&config.footers.deny));
        }
    } else {
        out.push(footers_dropped_line());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagnostic::Field;

    fn diag(code: &'static str) -> Diagnostic {
        Diagnostic::transform(code, Field::Description)
    }

    #[test]
    fn only_tripped_rules_are_emitted() {
        let g = guidance(&[diag("description.trailing-dot")], &Config::default());
        assert_eq!(g.len(), 1);
        assert!(g[0].contains("do not end the subject"));
    }

    #[test]
    fn repeated_codes_yield_one_line() {
        let g = guidance(
            &[
                diag("description.dots-stripped"),
                diag("description.dots-stripped"),
            ],
            &Config::default(),
        );
        assert_eq!(g.len(), 1);
    }

    #[test]
    fn dots_rule_follows_internal_dots_setting() {
        let mut cfg = Config::default();
        cfg.description.internal_dots = InternalDots::None;
        let g = guidance(&[diag("description.dots-stripped")], &cfg);
        assert!(g[0].contains("at all"), "got: {g:?}");

        cfg.description.internal_dots = InternalDots::KeepInNumbers;
        let g = guidance(&[diag("description.dots-stripped")], &cfg);
        assert!(g[0].contains("only between digits"), "got: {g:?}");
    }

    #[test]
    fn lossy_transforms_are_called_out() {
        let g = guidance(
            &[diag("body.dropped"), diag("footers.dropped")],
            &Config::default(),
        );
        assert!(g.iter().any(|l| l.contains("body is discarded")));
        assert!(g.iter().any(|l| l.contains("Co-Authored-By")));
    }

    #[test]
    fn disallowed_type_lists_the_allowed_ones() {
        let mut cfg = Config::default();
        cfg.rules.allowed_types = vec!["feat".into(), "fix".into()];
        let g = guidance(&[diag("type.disallowed")], &cfg);
        assert_eq!(g[0], "use one of these types: feat, fix");
    }

    fn ruined(description: &str) -> Vec<(String, String)> {
        mangled_description(description, &Config::default().description)
    }

    #[test]
    fn fusing_two_words_is_mangling() {
        assert_eq!(
            ruined("restore actions/checkout in release.yml"),
            vec![
                (
                    "actions/checkout".to_string(),
                    "actionscheckout".to_string()
                ),
                ("release.yml".to_string(), "releaseyml".to_string()),
            ]
        );
    }

    #[test]
    fn three_words_fused_report_the_authored_spelling() {
        assert_eq!(
            ruined("handle src/lib.rs error"),
            vec![("src/lib.rs".to_string(), "srclibrs".to_string())]
        );
    }

    #[test]
    fn a_short_segment_still_counts_once_it_joins_a_longer_run() {
        assert_eq!(
            ruined("handle src/a.rs error"),
            vec![("src/a.rs".to_string(), "srcars".to_string())]
        );
    }

    #[test]
    fn trailing_dot_removal_is_not_mangling() {
        assert!(ruined("bump a & b.").is_empty());
    }

    #[test]
    fn bracket_removal_at_token_edges_is_not_mangling() {
        assert!(ruined("handle [null] ids").is_empty());
    }

    #[test]
    fn version_pins_are_not_mangling() {
        assert!(ruined("pin node 24.x").is_empty());
        assert!(ruined("drop the 0.x fallback").is_empty());
        assert!(ruined("bump serde to 1.2.3-rc.1").is_empty());
        assert!(ruined("fix e.g. spacing").is_empty());
    }

    #[test]
    fn a_replaced_word_is_not_examined_here() {
        assert!(ruined("bump a and b").is_empty());
    }

    #[test]
    fn a_replace_rule_causing_fusion_is_not_flagged() {
        let mut cfg = Config::default().description;
        cfg.replace = vec![Replace {
            from: ":".into(),
            to: String::new(),
            regex: false,
        }];
        assert!(mangled_description("handle src:lib error", &cfg).is_empty());
    }

    #[test]
    fn isolated_punctuation_tokens_fuse_nothing() {
        assert!(ruined("bump a & b .").is_empty());
        assert!(ruined("split a / b comparison").is_empty());
    }

    #[test]
    fn no_spaces_around_the_separator_still_fuses() {
        assert_eq!(
            ruined("split release/notes.here"),
            vec![(
                "release/notes.here".to_string(),
                "releasenoteshere".to_string()
            )]
        );
    }

    #[test]
    fn lowercasing_alone_is_not_mangling() {
        assert!(ruined("Bump A").is_empty());
    }

    #[test]
    fn a_repeated_word_does_not_hide_a_real_fusion() {
        assert_eq!(
            ruined("rename node.js to nodejs"),
            vec![("node.js".to_string(), "nodejs".to_string())]
        );
    }

    #[test]
    fn decomposed_unicode_still_fuses() {
        let nfd_cafe = "caf\u{0065}\u{0301}.txt"; // café.txt, NFD
        assert_eq!(
            ruined(&format!("rename {nfd_cafe} today")),
            vec![(nfd_cafe.to_string(), "cafe\u{0301}txt".to_string())]
        );
    }

    #[test]
    fn unchanged_description_yields_nothing() {
        assert!(ruined("bump a & b").is_empty());
    }

    #[test]
    fn a_description_over_the_length_cap_is_skipped() {
        let huge = "a".repeat(MAX_MANGLING_CHECK_LEN + 1);
        assert!(mangled_description(&huge, &Config::default().description).is_empty());
    }

    #[test]
    fn unknown_codes_are_ignored() {
        assert!(guidance(&[diag("something.else")], &Config::default()).is_empty());
    }

    #[test]
    fn rules_cover_the_defaults() {
        let r = rules(&Config::default());
        assert!(r.iter().any(|l| l.contains("Conventional Commit")));
        assert!(r.iter().any(|l| l.contains("72 characters")));
        assert!(r.iter().any(|l| l.contains("body is discarded")));
        assert!(r.iter().any(|l| l.contains("Co-Authored-By")));
        assert!(r.iter().any(|l| l.contains("only between digits")));
    }

    #[test]
    fn rules_reflect_a_keep_everything_config() {
        let mut cfg = Config::default();
        cfg.body.keep = true;
        cfg.footers.keep = true;
        cfg.footers.deny = vec![];
        let r = rules(&cfg);
        assert!(!r.iter().any(|l| l.contains("discarded")), "got: {r:?}");
    }
}
