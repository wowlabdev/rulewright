use crate::path::Path;

use super::config::Config;

/// Per-file context passed to line rules with path, raw contents, and pre-split lines.
#[derive(Debug)]
pub struct FileCtx<'a> {
    pub rel: &'a str,
    pub path: &'a Path,
    /// Cargo package that owns this file, if the file belongs to a workspace member.
    pub package_name: Option<&'a str>,
    /// Whether Cargo metadata permits publishing the owning package; `None` outside a package.
    pub package_publishable: Option<bool>,
    pub lines: &'a [&'a str],
    pub contents: &'a str,
    pub config: &'a Config,
}

/// A single lint violation reported by a rule against a file location.
#[derive(Debug, serde::Serialize)]
pub struct Violation {
    pub rel: String,
    pub line: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub column: Option<usize>,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rule: Option<&'static str>,
    #[serde(skip)]
    pub(crate) fixable: bool,
}

impl Violation {
    pub(crate) fn with_rule(mut self, rule: &'static str) -> Self {
        self.rule = Some(rule);

        self
    }

    pub(crate) fn with_column(mut self, column: usize) -> Self {
        self.column = Some(column);

        self
    }

    pub(crate) fn mark_fixable(&mut self) {
        self.fixable = true;
    }

    /// Whether the registered fixer produced an edit for this specific finding.
    #[must_use]
    pub const fn is_fixable(&self) -> bool {
        self.fixable
    }

    /// Return the rule name or `"unknown"` for untagged violations.
    #[must_use]
    pub fn rule_name(&self) -> &str {
        self.rule.unwrap_or("unknown")
    }
}

/// Construct a `Violation` without a rule tag; registry dispatch attaches the rule identifier.
pub fn violation(rel: &str, line: usize, msg: impl Into<String>) -> Violation {
    Violation {
        rel: rel.to_string(),
        line,
        column: None,
        message: msg.into(),
        rule: None,
        fixable: false,
    }
}

impl FileCtx<'_> {
    /// Whether the owning Cargo package explicitly sets `publish = false`.
    #[must_use]
    pub const fn is_explicitly_non_publishable(&self) -> bool {
        matches!(self.package_publishable, Some(false))
    }

    /// Get the source text of a specific line (1-based).
    #[must_use]
    pub fn line(&self, lineno: usize) -> Option<&str> {
        self.lines.get(lineno.checked_sub(1)?).copied()
    }
}
