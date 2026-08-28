//! Shared winnow-based parsers for source code patterns.

use std::collections::BTreeMap;

use winnow::{combinator::alt, error::ContextError, prelude::*};

/// Try to consume `parser` from `input`, advancing it on success.
pub(crate) fn try_parse<'a, O>(
    input: &mut &'a str,
    mut parser: impl Parser<&'a str, O, ContextError>,
) -> Option<O> {
    parser.parse_next(input).ok()
}

/// Check if a parser matches the start of `input` without consuming it.
pub(crate) fn matches<'a, O>(
    input: &'a str,
    mut parser: impl Parser<&'a str, O, ContextError>,
) -> bool {
    parser.parse_peek(input).is_ok()
}

/// Strip any comment prefix (`///`, `//!`, `//`) from a trimmed line, `None` if not a comment.
pub(crate) fn comment_content(line: &str) -> Option<&str> {
    let mut input = line;

    try_parse(&mut input, alt(("///", "//!", "//")))?;

    Some(input)
}

/// Strip a doc comment prefix (`///` or `//!`) from a trimmed line, `None` if not a doc comment.
pub(crate) fn doc_comment_content(line: &str) -> Option<&str> {
    let mut input = line;

    try_parse(&mut input, alt(("///", "//!")))?;

    Some(input)
}

/// Return `true` if a trimmed line starts with `//`.
pub(crate) fn is_comment(line: &str) -> bool {
    matches(line, "//")
}

/// Find `prefix(...)` in a line with balanced parentheses.
pub(crate) fn balanced_extract<'a>(
    line: &'a str,
    prefix: &str,
) -> Option<(&'a str, &'a str, &'a str)> {
    use winnow::token::take_until;
    let mut input = line;
    let before: &str = try_parse(&mut input, take_until(0.., prefix))?;
    let plen = prefix.len();
    // BOUNDS: take_until matched prefix in input, so input.len() >= plen
    let rest = &input[plen..];

    let mut depth: u32 = 1;
    let mut scan = rest;
    let mut pos = 0;

    while let Some(ch) = try_parse(&mut scan, winnow::token::any) {
        match ch {
            '"' => {
                super::scanner::skip_string(&mut scan);
                pos = rest.len() - scan.len();
                continue;
            }

            '\'' => {
                super::scanner::skip_char_literal(&mut scan);
                pos = rest.len() - scan.len();
                continue;
            }

            '(' => depth += 1,

            ')' => {
                depth -= 1;

                if depth == 0 {
                    // BOUNDS: pos is a valid byte offset within rest; pos+1 is safe because ')' is 1 byte
                    return Some((before, &rest[..pos], &rest[pos + 1..]));
                }
            }

            _ => {}
        }

        pos += ch.len_utf8();
    }

    None
}

/// Scope of a rulewright suppression directive.
#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[non_exhaustive]
pub(crate) enum Scope {
    NextLine,
    File,
    Block,
    Fn,
}

impl Scope {
    pub(crate) fn prefix(&self) -> &'static str {
        match self {
            Scope::NextLine => "#rw()",
            Scope::File => "#rw(file:)",
            Scope::Block => "#rw(block:)",
            Scope::Fn => "#rw(fn:)",
        }
    }
}

/// A parsed `#rw(...)` suppression directive with its scope, rules, and reason text.
#[derive(Debug)]
pub(crate) struct Directive {
    pub scope: Scope,
    pub rules: Vec<String>,
    pub values: BTreeMap<String, usize>,
    pub wildcard: bool,
    pub reason: String,
}

/// Outcome of parsing a candidate directive line: valid, missing reason, or malformed.
#[derive(Debug)]
#[non_exhaustive]
pub(crate) enum DirectiveResult {
    Valid(Directive),
    MissingReason(Scope),
    Malformed(String),
}

const DIRECTIVE_PREFIX: &str = "// #rw(";

/// Parse a rulewright suppression directive from a source line, `None` if it is not one.
// #rw(fn: rust_cyclomatic_complexity) directive grammar validates scopes, wildcards, values, and reasons in one parser
pub(crate) fn directive(line: &str) -> Option<DirectiveResult> {
    use winnow::token::take_until;

    let trimmed = line.trim();
    let mut input = trimmed;

    try_parse(&mut input, DIRECTIVE_PREFIX)?;

    let Some(inside) = try_parse(&mut input, take_until(0.., ")")) else {
        return Some(DirectiveResult::Malformed("missing closing `)`".into()));
    };
    let _ = try_parse(&mut input, ")");
    let after = input.trim();

    if inside.is_empty() {
        return Some(DirectiveResult::Malformed(
            "empty `#rw()` — need rule names".into(),
        ));
    }

    let (scope, rule_part) = parse_scope(inside);

    if rule_part == "*" {
        if after.is_empty() {
            return Some(DirectiveResult::MissingReason(scope));
        }

        return Some(DirectiveResult::Valid(Directive {
            scope,
            rules: Vec::new(),
            values: BTreeMap::new(),
            wildcard: true,
            reason: after.to_string(),
        }));
    }

    let mut rules = Vec::new();
    let mut values = BTreeMap::new();

    for target in rule_part
        .split(',')
        .map(str::trim)
        .filter(|target| !target.is_empty())
    {
        let (rule, value) = target
            .split_once('=')
            .map_or((target, None), |(rule, value)| {
                (rule.trim(), Some(value.trim()))
            });

        if rule.is_empty() {
            return Some(DirectiveResult::Malformed(
                "empty rule name before `=`".into(),
            ));
        }

        if let Some(value) = value {
            if scope != Scope::File {
                return Some(DirectiveResult::Malformed(
                    "numeric directive values require file scope".into(),
                ));
            }

            let Ok(value) = value.parse::<usize>() else {
                return Some(DirectiveResult::Malformed(
                    "directive value must be a positive integer".into(),
                ));
            };

            if value == 0 {
                return Some(DirectiveResult::Malformed(
                    "directive value must be a positive integer".into(),
                ));
            }

            values.insert(rule.to_owned(), value);
        }

        rules.push(rule.to_owned());
    }

    if rules.is_empty() {
        return Some(DirectiveResult::Malformed(
            "no rule names in `#rw()`".into(),
        ));
    }

    if rules.iter().any(|rule| rule == "*") {
        return Some(DirectiveResult::Malformed(
            "wildcard `*` must be the only target in `#rw()`".into(),
        ));
    }

    if after.is_empty() {
        return Some(DirectiveResult::MissingReason(scope));
    }

    Some(DirectiveResult::Valid(Directive {
        scope,
        rules,
        values,
        wildcard: false,
        reason: after.to_string(),
    }))
}

fn parse_scope(inside: &str) -> (Scope, &str) {
    let mut input = inside;

    match try_parse(
        &mut input,
        alt((
            "file:".value(Scope::File),
            "block:".value(Scope::Block),
            "fn:".value(Scope::Fn),
        )),
    ) {
        Some(scope) => (scope, input.trim()),
        None => (Scope::NextLine, inside.trim()),
    }
}

/// Rewrite only a directive's target list while preserving its surrounding syntax.
pub(crate) fn rewrite_directive_rules(line: &str, scope: &Scope, rules: &[&str]) -> Option<String> {
    if rules.is_empty() {
        return None;
    }

    let marker = line.find(DIRECTIVE_PREFIX)?;
    let inside_start = marker + DIRECTIVE_PREFIX.len();
    let close_offset = line.get(inside_start..)?.find(')')?;
    let inside_end = inside_start + close_offset;
    let inside = line.get(inside_start..inside_end)?;
    let scope_len = match scope {
        Scope::NextLine => 0,
        Scope::File => "file:".len(),
        Scope::Block => "block:".len(),
        Scope::Fn => "fn:".len(),
    };
    let targets = inside.get(scope_len..)?;
    let leading = targets.len() - targets.trim_start().len();
    let trailing = targets.len() - targets.trim_end().len();
    let targets_start = inside_start + scope_len + leading;
    let targets_end = inside_end.checked_sub(trailing)?;
    let replacement = rules.join(", ");

    Some(format!(
        "{}{}{}",
        line.get(..targets_start)?,
        replacement,
        line.get(targets_end..)?
    ))
}

/// Return `true` if the line starts with `#[allow(` or `#![allow(`.
pub(crate) fn is_allow_attr(line: &str) -> bool {
    matches(line, alt(("#[allow(", "#![allow(")))
}

/// Extract comment text after a `// ` prefix, `""` for bare `//`, `None` if not a `// ` comment.
pub(crate) fn prose_comment_content(line: &str) -> Option<&str> {
    let trimmed = line.trim();
    let mut input = trimmed;

    try_parse(&mut input, "//")?;

    if input.is_empty() {
        return Some("");
    }

    try_parse(&mut input, " ")?;

    Some(input)
}

/// Return `true` if the line contains a `//` comment anywhere after code.
pub(crate) fn has_inline_comment(line: &str) -> bool {
    find_comment_start(line).is_some()
}

/// Parse the character immediately after the `//` prefix on a trimmed line.
pub(crate) fn char_after_slashes(line: &str) -> Option<char> {
    let mut input = line;

    try_parse(&mut input, "//")?;

    try_parse(&mut input, winnow::token::any)
}

/// Extract the field name from a `redundant field initializer` violation message.
pub(crate) fn redundant_field_name(message: &str) -> Option<&str> {
    use winnow::token::take_until;
    let mut input = message;

    try_parse(&mut input, "redundant field initializer `")?;
    let name: &str = try_parse(&mut input, take_until(0.., ":"))?;

    if name.is_empty() { None } else { Some(name) }
}

/// Return the portion of `line` starting from a `//` comment (may be inline).
pub(crate) fn find_comment_start(line: &str) -> Option<&str> {
    use winnow::token::take_until;
    let mut input = line;
    let _: &str = try_parse(&mut input, take_until(0.., "//"))?;

    Some(input)
}

/// Find `keyword` in `haystack` and return the substring following it, or `None` if not found.
pub(crate) fn find_keyword_suffix<'a>(haystack: &'a str, keyword: &str) -> Option<&'a str> {
    use winnow::token::take_until;
    let mut input = haystack;
    let _: &str = try_parse(&mut input, take_until(0.., keyword))?;

    input.get(keyword.len()..)
}

#[cfg(test)]
mod tests;
