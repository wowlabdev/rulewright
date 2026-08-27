use crate::path::Path;

use super::config::Config;

/// Per-file context passed to line rules with path, raw contents, and pre-split lines.
#[derive(Debug)]
pub struct FileCtx<'a> {
    pub rel: &'a str,
    pub path: &'a Path,
    /// Cargo package that owns this file, if the file belongs to a workspace member.
    pub package_name: Option<&'a str>,
    pub lines: &'a [&'a str],
    pub contents: &'a str,
    pub config: &'a Config,
}

/// A single lint violation reported by a rule against a file location.
#[derive(Debug)]
pub struct Violation {
    pub rel: String,
    pub line: usize,
    pub message: String,
    pub rule: Option<&'static str>,
}

impl Violation {
    pub(crate) fn with_rule(mut self, rule: &'static str) -> Self {
        self.rule = Some(rule);

        self
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
        message: msg.into(),
        rule: None,
    }
}

impl FileCtx<'_> {
    /// Get the source text of a specific line (1-based).
    #[must_use]
    pub fn line(&self, lineno: usize) -> Option<&str> {
        self.lines.get(lineno.checked_sub(1)?).copied()
    }
}
