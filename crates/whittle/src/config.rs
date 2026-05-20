//! Whittle configuration: TOML-driven rules for subject normalization + linting.

use serde::Deserialize;

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    #[serde(default)]
    pub scope: ScopeConfig,
    #[serde(default)]
    pub description: DescriptionConfig,
    #[serde(default)]
    pub body: BodyConfig,
    #[serde(default)]
    pub footers: FootersConfig,
    #[serde(default)]
    pub rules: RulesConfig,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct ScopeConfig {
    pub lowercase: bool,
    pub replace: Vec<Replace>,
}

impl Default for ScopeConfig {
    fn default() -> Self {
        Self {
            lowercase: true,
            replace: vec![
                Replace {
                    from: "/".into(),
                    to: "-".into(),
                    regex: false,
                },
                Replace {
                    from: "\\".into(),
                    to: "-".into(),
                    regex: false,
                },
            ],
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct DescriptionConfig {
    pub lowercase: bool,
    pub collapse_whitespace: bool,
    pub trailing_dot: TrailingDot,
    pub strip_chars: Vec<char>,
    pub internal_dots: InternalDots,
    pub replace: Vec<Replace>,
}

impl Default for DescriptionConfig {
    fn default() -> Self {
        Self {
            lowercase: true,
            collapse_whitespace: true,
            trailing_dot: TrailingDot::Strip,
            strip_chars: vec!['/', '\\', '[', ']', '{', '}'],
            internal_dots: InternalDots::KeepInNumbers,
            replace: vec![
                Replace {
                    from: r"\band\b".into(),
                    to: "&".into(),
                    regex: true,
                },
                Replace {
                    from: "\u{201C}".into(),
                    to: "\"".into(),
                    regex: false,
                },
                Replace {
                    from: "\u{201D}".into(),
                    to: "\"".into(),
                    regex: false,
                },
                Replace {
                    from: "\u{2018}".into(),
                    to: "'".into(),
                    regex: false,
                },
                Replace {
                    from: "\u{2019}".into(),
                    to: "'".into(),
                    regex: false,
                },
                Replace {
                    from: "\u{2014}".into(),
                    to: "-".into(),
                    regex: false,
                },
                Replace {
                    from: "\u{2013}".into(),
                    to: "-".into(),
                    regex: false,
                },
                Replace {
                    from: "!{2,}".into(),
                    to: "!".into(),
                    regex: true,
                },
                Replace {
                    from: r"\?{2,}".into(),
                    to: "?".into(),
                    regex: true,
                },
            ],
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TrailingDot {
    Keep,
    Strip,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum InternalDots {
    All,
    None,
    KeepInNumbers,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Replace {
    pub from: String,
    pub to: String,
    #[serde(default)]
    pub regex: bool,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct BodyConfig {
    pub keep: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct FootersConfig {
    pub keep: bool,
    pub deny: Vec<String>,
}

impl Default for FootersConfig {
    fn default() -> Self {
        Self {
            keep: false,
            deny: vec!["Co-Authored-By".into(), "Co-authored-by".into()],
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct RulesConfig {
    pub max_subject_length: usize,
    pub require_conventional: bool,
    pub allowed_types: Vec<String>,
}

impl Default for RulesConfig {
    fn default() -> Self {
        Self {
            max_subject_length: 72,
            require_conventional: true,
            allowed_types: [
                "feat", "fix", "refactor", "perf", "docs", "test", "chore", "build", "ci", "style",
                "revert",
            ]
            .iter()
            .map(std::string::ToString::to_string)
            .collect(),
        }
    }
}

impl Config {
    /// # Errors
    /// Returns an error if `text` is not valid TOML or does not match the `Config` schema.
    pub fn from_toml(text: &str) -> anyhow::Result<Self> {
        Ok(toml::from_str(text)?)
    }

    /// # Errors
    /// Returns an error if `path` cannot be read or its contents are not a valid `Config`.
    pub fn load_or_default(path: Option<&std::path::Path>) -> anyhow::Result<Self> {
        match path {
            Some(p) => Self::from_toml(&std::fs::read_to_string(p)?),
            None => Ok(Self::default()),
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;

    #[test]
    fn defaults_match_python_script_behavior() {
        let c = Config::default();
        assert!(c.scope.lowercase);
        assert_eq!(c.scope.replace.len(), 2);
        assert!(c.scope.replace.iter().any(|r| r.from == "/" && r.to == "-"));
        assert!(
            c.scope
                .replace
                .iter()
                .any(|r| r.from == "\\" && r.to == "-")
        );
        assert!(c.description.lowercase);
        assert!(c.description.collapse_whitespace);
        assert_eq!(c.description.trailing_dot, TrailingDot::Strip);
        assert_eq!(c.description.internal_dots, InternalDots::KeepInNumbers);
        assert!(c.description.strip_chars.contains(&'/'));
        assert!(c.description.strip_chars.contains(&'\\'));
        assert!(c.description.strip_chars.contains(&'['));
        assert!(c.description.strip_chars.contains(&']'));
        assert!(c.description.strip_chars.contains(&'{'));
        assert!(c.description.strip_chars.contains(&'}'));
        assert!(!c.body.keep);
        assert!(!c.footers.keep);
        assert!(c.footers.deny.iter().any(|s| s == "Co-Authored-By"));
        assert_eq!(c.rules.max_subject_length, 72);
        assert!(c.rules.require_conventional);
        assert!(c.rules.allowed_types.contains(&"feat".to_string()));
    }

    #[test]
    fn empty_toml_yields_defaults() {
        let c = Config::from_toml("").unwrap();
        let d = Config::default();
        assert_eq!(c.rules.max_subject_length, d.rules.max_subject_length);
        assert_eq!(c.description.lowercase, d.description.lowercase);
    }

    #[test]
    fn partial_overrides_preserve_other_defaults() {
        let c = Config::from_toml("[rules]\nmax_subject_length = 50\n").unwrap();
        assert_eq!(c.rules.max_subject_length, 50);
        assert!(c.rules.require_conventional);
        assert!(c.description.lowercase); // default preserved
    }

    #[test]
    fn unknown_top_level_key_rejected() {
        let res = Config::from_toml("foo = 1\n");
        assert!(res.is_err());
    }

    #[test]
    fn unknown_nested_key_rejected() {
        let res = Config::from_toml("[description]\nfoo = 1\n");
        assert!(res.is_err());
    }

    #[test]
    fn trailing_dot_keep_parses() {
        let c = Config::from_toml("[description]\ntrailing_dot = \"keep\"\n").unwrap();
        assert_eq!(c.description.trailing_dot, TrailingDot::Keep);
    }

    #[test]
    fn internal_dots_all_parses() {
        let c = Config::from_toml("[description]\ninternal_dots = \"all\"\n").unwrap();
        assert_eq!(c.description.internal_dots, InternalDots::All);
    }

    #[test]
    fn internal_dots_invalid_value_rejected() {
        let res = Config::from_toml("[description]\ninternal_dots = \"sometimes\"\n");
        assert!(res.is_err());
    }

    #[test]
    fn replace_rule_round_trip() {
        let toml = r#"
[description]
replace = [
  { from = "and", to = "&" },
  { from = "\\bfoo\\b", to = "bar", regex = true },
]
"#;
        let c = Config::from_toml(toml).unwrap();
        assert_eq!(c.description.replace.len(), 2);
        assert!(!c.description.replace[0].regex);
        assert!(c.description.replace[1].regex);
    }

    #[test]
    fn strip_chars_parses_as_chars() {
        let c = Config::from_toml("[description]\nstrip_chars = [\"!\", \"?\"]\n").unwrap();
        assert_eq!(c.description.strip_chars, vec!['!', '?']);
    }

    #[test]
    fn allowed_types_override() {
        let c = Config::from_toml("[rules]\nallowed_types = [\"feat\", \"fix\"]\n").unwrap();
        assert_eq!(
            c.rules.allowed_types,
            vec!["feat".to_string(), "fix".to_string()]
        );
    }

    #[test]
    fn body_keep_parses() {
        let c = Config::from_toml("[body]\nkeep = true\n").unwrap();
        assert!(c.body.keep);
    }

    #[test]
    fn footers_keep_with_deny_parses() {
        let toml = "[footers]\nkeep = true\ndeny = [\"X\", \"Y\"]\n";
        let c = Config::from_toml(toml).unwrap();
        assert!(c.footers.keep);
        assert_eq!(c.footers.deny, vec!["X".to_string(), "Y".to_string()]);
    }
}
