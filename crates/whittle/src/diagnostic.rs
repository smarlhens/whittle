//! Structured diagnostics: typed fields consumers can branch on, not just
//! prose. [`Diagnostic::message`] derives human text from them.

use serde::Serialize;

/// Warning: whittle can rewrite it. Error: no rewrite fixes it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Warning,
    Error,
}

impl Severity {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Warning => "warning",
            Self::Error => "error",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Field {
    Type,
    Scope,
    Description,
    Subject,
    Body,
    Footers,
    Message,
}

impl Field {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Type => "type",
            Self::Scope => "scope",
            Self::Description => "description",
            Self::Subject => "subject",
            Self::Body => "body",
            Self::Footers => "footers",
            Self::Message => "message",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Diagnostic {
    /// Stable, dotted identifier, e.g. `description.trailing-dot`.
    pub code: &'static str,
    pub severity: Severity,
    pub field: Field,
    pub before: Option<String>,
    /// None means the content is removed entirely.
    pub after: Option<String>,
    pub detail: Option<String>,
    /// Written to disk (`fix`) vs still pending (`check`).
    pub applied: bool,
    pub message_override: Option<String>,
}

impl Diagnostic {
    #[must_use]
    pub const fn transform(code: &'static str, field: Field) -> Self {
        Self {
            code,
            severity: Severity::Warning,
            field,
            before: None,
            after: None,
            detail: None,
            applied: false,
            message_override: None,
        }
    }

    #[must_use]
    pub const fn error(code: &'static str, field: Field) -> Self {
        Self {
            code,
            severity: Severity::Error,
            field,
            before: None,
            after: None,
            detail: None,
            applied: false,
            message_override: None,
        }
    }

    #[must_use]
    pub fn rewrite(mut self, before: &str, after: &str) -> Self {
        self.before = Some(before.to_string());
        self.after = Some(after.to_string());
        self
    }

    #[must_use]
    pub fn removing(mut self, before: &str) -> Self {
        self.before = Some(before.to_string());
        self.after = None;
        self
    }

    #[must_use]
    pub fn detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    #[must_use]
    pub fn message_override(mut self, message: impl Into<String>) -> Self {
        self.message_override = Some(message.into());
        self
    }

    pub fn mark_applied(diags: &mut [Self]) {
        for d in diags {
            d.applied = true;
        }
    }

    #[must_use]
    pub fn message(&self) -> String {
        if let Some(m) = &self.message_override {
            return m.clone();
        }
        let would = if self.applied { "" } else { "would be " };
        let suffix = self
            .detail
            .as_ref()
            .map_or_else(String::new, |d| format!(" ({d})"));

        match self.code {
            "body.dropped" => format!("body {would}removed{suffix}"),
            "footers.dropped" => format!("all footers {would}removed{suffix}"),
            "footer.denied" => {
                let token = self.detail.as_deref().unwrap_or("?");
                format!("footer `{token}` {would}removed")
            }
            _ => match (&self.before, &self.after) {
                (Some(b), Some(a)) => {
                    let verb = if self.applied { " rewritten" } else { "" };
                    format!("{}{verb}: \"{b}\" -> \"{a}\"", self.field.as_str())
                }
                (Some(b), None) => format!("{}: \"{b}\" {would}removed", self.field.as_str()),
                _ => format!("{}: {}", self.field.as_str(), self.code),
            },
        }
    }
}

/// Disambiguates `after: null`: deleted vs. rejected.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Rewrite,
    Removal,
    /// `after`, if present, is illustrative only — never adopt it.
    Violation,
}

impl Kind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Rewrite => "rewrite",
            Self::Removal => "removal",
            Self::Violation => "violation",
        }
    }
}

/// Serializable view of a [`Diagnostic`].
#[derive(Debug, Serialize)]
pub struct DiagnosticJson<'a> {
    pub code: &'a str,
    pub severity: Severity,
    pub kind: Kind,
    pub field: Field,
    pub before: Option<&'a str>,
    pub after: Option<&'a str>,
    pub detail: Option<&'a str>,
    pub applied: bool,
    pub message: String,
}

impl<'a> From<&'a Diagnostic> for DiagnosticJson<'a> {
    fn from(d: &'a Diagnostic) -> Self {
        let kind = match (d.severity, d.after.is_some()) {
            (Severity::Error, _) => Kind::Violation,
            (Severity::Warning, true) => Kind::Rewrite,
            (Severity::Warning, false) => Kind::Removal,
        };
        Self {
            code: d.code,
            severity: d.severity,
            kind,
            field: d.field,
            before: d.before.as_deref(),
            after: d.after.as_deref(),
            detail: d.detail.as_deref(),
            applied: d.applied,
            message: d.message(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generic_rewrite_message() {
        let d = Diagnostic::transform("description.trailing-dot", Field::Description)
            .rewrite("Add Foo.", "add foo");
        assert_eq!(d.message(), "description: \"Add Foo.\" -> \"add foo\"");
    }

    #[test]
    fn pending_body_reads_as_would() {
        let d = Diagnostic::transform("body.dropped", Field::Body)
            .removing("why")
            .detail("3 lines");
        assert_eq!(d.message(), "body would be removed (3 lines)");
    }

    #[test]
    fn applied_body_reads_as_past_tense() {
        let mut d = Diagnostic::transform("body.dropped", Field::Body).removing("why");
        d.applied = true;
        assert_eq!(d.message(), "body removed");
    }

    #[test]
    fn denied_footer_names_the_token() {
        let d = Diagnostic::transform("footer.denied", Field::Footers)
            .removing("a <a@b>")
            .detail("Co-Authored-By");
        assert_eq!(d.message(), "footer `Co-Authored-By` would be removed");
    }

    #[test]
    fn override_wins_over_derived_text() {
        let d = Diagnostic::error("subject.too-long", Field::Subject)
            .message_override("subject is 80 chars; max allowed is 72");
        assert_eq!(d.message(), "subject is 80 chars; max allowed is 72");
    }

    #[test]
    fn mark_applied_flips_tense_in_bulk() {
        let mut diags = vec![Diagnostic::transform("body.dropped", Field::Body).removing("x")];
        Diagnostic::mark_applied(&mut diags);
        assert_eq!(diags[0].message(), "body removed");
    }

    #[test]
    fn transforms_warn_and_rules_error() {
        assert_eq!(
            Diagnostic::transform("body.dropped", Field::Body).severity,
            Severity::Warning
        );
        assert_eq!(
            Diagnostic::error("subject.too-long", Field::Subject).severity,
            Severity::Error
        );
    }
}
