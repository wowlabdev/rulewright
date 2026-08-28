// #rw(file: rust_duplicate_strings) The linter harness intentionally repeats source fixtures across independent rule scenarios.
// #rw(file: rust_default_hasher) trusted rule-name keys in cold dev-CLI maps; fast-hasher dependency not warranted

//! Parsing and application of `#rw(...)` suppression directives.

use std::collections::HashMap;

#[cfg(test)]
use googletest::prelude::*;
use winnow::token::any;

use super::{
    parse::{self, DirectiveResult, Scope},
    scanner,
};
use crate::{Violation, violation};

/// One parsed `#rw(...)` directive with its source location, scope, targets, and reason.
#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub(crate) struct SuppressionEntry {
    pub rel: String,
    pub line: usize,
    end_line: Option<usize>,
    pub scope: Scope,
    pub rules: Vec<String>,
    pub values: std::collections::BTreeMap<String, usize>,
    pub wildcard: bool,
    pub reason: String,
}

impl SuppressionEntry {
    /// Return whether this directive's scope includes a source line.
    pub(crate) fn covers_line(&self, line: usize) -> bool {
        match self.scope {
            Scope::File => true,

            Scope::NextLine | Scope::Block | Scope::Fn => self
                .end_line
                .is_some_and(|end| line > self.line && line <= end),
        }
    }

    /// Return whether this directive targets the tagged violation.
    pub(crate) fn suppresses(&self, violation: &Violation) -> bool {
        let Some(rule) = violation.rule else {
            return false;
        };
        let targets_rule = self.wildcard || self.rules.iter().any(|target| target == rule);

        if !targets_rule {
            return false;
        }

        self.covers_line(violation.line)
    }
}

/// Map from 1-based line number to the list of rule names suppressed on that line.
pub(crate) type LineSuppressMap = HashMap<usize, Vec<String>>;

fn brace_depth_change(line: &str) -> (i32, bool) {
    let mut delta: i32 = 0;
    let mut found_open = false;
    let mut input = line;

    while let Some(ch) = parse::try_parse(&mut input, any) {
        match ch {
            '/' if parse::matches(input, '/') => return (delta, found_open),

            '"' => scanner::skip_string(&mut input),

            'r' if parse::matches(input, '#') || parse::matches(input, '"') => {
                scanner::try_skip_raw_string(&mut input);
            }

            '\'' => scanner::skip_char_literal(&mut input),

            '{' => {
                delta += 1;
                found_open = true;
            }

            '}' => {
                delta -= 1;
            }

            _ => {}
        }
    }

    (delta, found_open)
}

/// Pre-computed suppression state for a single file: per-line, per-file, and the raw entries.
#[derive(Clone, Debug, Default, serde::Deserialize, serde::Serialize)]
pub(crate) struct Suppressions {
    pub lines: LineSuppressMap,
    pub file_rules: Vec<String>,
    pub file_all: bool,
    pub entries: Vec<SuppressionEntry>,
}

const WILDCARD: &str = "*";
const NEXT_LINE_OFFSET: usize = 2;

/// Pre-compute the suppression map, pushing violations for malformed or unknown directives.
// #rw(fn: rust_cyclomatic_complexity) suppression parser inherently branches on directive variants
pub(crate) fn suppressed_lines(
    rel: &str,
    lines: &[&str],
    errors: &mut Vec<Violation>,
    registered_rules: Option<&std::collections::HashSet<&str>>,
) -> Suppressions {
    let mut map: LineSuppressMap = HashMap::new();
    let mut file_rules: Vec<String> = Vec::new();
    let mut file_all = false;
    let mut entries: Vec<SuppressionEntry> = Vec::new();

    for (i, line) in lines.iter().enumerate() {
        match parse::directive(line) {
            Some(DirectiveResult::Valid(d)) => {
                if let Some(known) = registered_rules {
                    for name in &d.rules {
                        if !known.contains(name.as_str()) {
                            let msg = format!(
                                "unknown rule `{name}` in #rw directive — \
                                 check for typos (use `rulewright --list` to see all rules)"
                            );

                            errors.push(violation(rel, i + 1, msg));
                        }
                    }
                }

                let scope = d.scope;
                let wildcard = d.wildcard;
                let rule_names: Vec<String> = if wildcard {
                    vec![WILDCARD.to_string()]
                } else {
                    d.rules
                };

                let directive_line = i + 1;
                let end_line = match scope {
                    Scope::File => {
                        if wildcard {
                            file_all = true;
                        } else {
                            file_rules.extend(
                                rule_names
                                    .iter()
                                    .filter(|rule| !d.values.contains_key(rule.as_str()))
                                    .cloned(),
                            );
                        }

                        None
                    }

                    Scope::NextLine => {
                        map.entry(i + NEXT_LINE_OFFSET)
                            .or_default()
                            .extend(rule_names.clone());

                        Some(i + NEXT_LINE_OFFSET)
                    }

                    Scope::Block => {
                        let mut end_line = directive_line;

                        for (j, bline) in lines.iter().enumerate().skip(i + 1) {
                            let t = bline.trim();

                            if t.is_empty() || matches!(t, "}" | "};" | ")" | ");") {
                                break;
                            }

                            end_line = j + 1;
                            map.entry(j + 1).or_default().extend(rule_names.clone());
                        }

                        Some(end_line)
                    }

                    Scope::Fn => {
                        let mut depth: i32 = 0;
                        let mut found_open = false;
                        let mut end_line = directive_line;

                        for (j, fline) in lines.iter().enumerate().skip(i + 1) {
                            let (delta, has_open) = brace_depth_change(fline);

                            depth += delta;
                            found_open |= has_open;
                            end_line = j + 1;
                            map.entry(j + 1).or_default().extend(rule_names.clone());

                            if found_open && depth <= 0 {
                                break;
                            }
                        }

                        Some(end_line)
                    }
                };

                entries.push(SuppressionEntry {
                    rel: rel.to_string(),
                    line: directive_line,
                    end_line,
                    scope,
                    rules: rule_names,
                    values: d.values,
                    wildcard,
                    reason: d.reason,
                });
            }

            Some(DirectiveResult::MissingReason(scope)) => {
                let name = scope.prefix();

                errors.push(violation(
                    rel,
                    i + 1,
                    format!("{name} requires a reason after the closing `)`"),
                ));
            }

            Some(DirectiveResult::Malformed(msg)) => {
                errors.push(violation(rel, i + 1, format!("#rw directive error: {msg}")));
            }

            None => {}
        }
    }

    Suppressions {
        lines: map,
        file_rules,
        file_all,
        entries,
    }
}

/// Return `true` if `rule` is suppressed for the entire file (via `#rw(file: ...)` or `#rw(file: *)`).
pub(crate) fn is_file_suppressed(suppressed: &Suppressions, rule: &str) -> bool {
    suppressed.file_all || suppressed.file_rules.iter().any(|r| r == rule)
}

/// Filter out violations that are suppressed for their specific rule on that line.
pub(crate) fn filter(suppressed: &Suppressions, violations: Vec<Violation>) -> Vec<Violation> {
    if suppressed.lines.is_empty() {
        return violations;
    }

    violations
        .into_iter()
        .filter(|v| {
            let Some(rule) = v.rule else { return true };

            suppressed
                .lines
                .get(&v.line)
                .is_none_or(|rules| !rules.iter().any(|r| r == rule || r == WILDCARD))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_errors() -> Vec<Violation> {
        Vec::new()
    }

    #[gtest]
    fn empty_directive_is_error() -> Result<()> {
        let lines = vec!["// #rw()", "let x = 1;"];
        let mut errors = make_errors();
        let sup = suppressed_lines("t.rs", &lines, &mut errors, None);

        verify_true!(sup.lines.is_empty())?;
        verify_eq!(errors.len(), 1)?;

        verify_true!(errors[0].message.contains("empty"))
    }

    #[gtest]
    fn missing_reason_is_error() -> Result<()> {
        let lines = vec!["// #rw(rust_panic)", "let x = 1;"];
        let mut errors = make_errors();
        let sup = suppressed_lines("t.rs", &lines, &mut errors, None);

        verify_true!(sup.lines.is_empty())?;
        verify_eq!(errors.len(), 1)?;
        verify_true!(errors[0].message.contains("requires a reason"))?;

        Ok(())
    }

    #[gtest]
    fn next_line_works() -> Result<()> {
        let lines = vec![
            "// #rw(rust_panic) startup invariant",
            "panic!(\"oh no\");",
            "let y = 2;",
        ];
        let mut errors = make_errors();
        let sup = suppressed_lines("t.rs", &lines, &mut errors, None);

        verify_eq!(sup.lines.get(&2).or_fail()?, &["rust_panic"])?;
        verify_false!(sup.lines.contains_key(&3))?;
        verify_true!(errors.is_empty())?;

        Ok(())
    }

    #[gtest]
    fn multi_rule_works() -> Result<()> {
        let lines = vec![
            "// #rw(rust_panic, rust_dbg) error path",
            "let msg = format!(\"err: {}\", e);",
        ];
        let mut errors = make_errors();
        let sup = suppressed_lines("t.rs", &lines, &mut errors, None);
        let rules = sup.lines.get(&2).or_fail()?;

        verify_eq!(rules, &["rust_panic", "rust_dbg"])?;
        verify_true!(errors.is_empty())?;

        Ok(())
    }

    #[gtest]
    fn file_scope_works() -> Result<()> {
        let lines = vec![
            "// #rw(file: rust_commented_code) generated file",
            "let x = 1;",
        ];
        let mut errors = make_errors();
        let sup = suppressed_lines("t.rs", &lines, &mut errors, None);

        verify_true!(errors.is_empty())?;
        verify_true!(is_file_suppressed(&sup, "rust_commented_code"))?;
        verify_false!(is_file_suppressed(&sup, "rust_style"))?;

        Ok(())
    }

    #[gtest]
    fn block_scope_works() -> Result<()> {
        let lines = vec![
            "// #rw(block: rust_wildcard_imports) barrel re-exports",
            "use foo::*;",
            "use bar::*;",
            "",
            "use baz::*;",
        ];
        let mut errors = make_errors();
        let sup = suppressed_lines("t.rs", &lines, &mut errors, None);

        verify_eq!(sup.lines.get(&2).or_fail()?, &["rust_wildcard_imports"])?;
        verify_eq!(sup.lines.get(&3).or_fail()?, &["rust_wildcard_imports"])?;
        verify_false!(sup.lines.contains_key(&5))?;
        verify_true!(errors.is_empty())?;

        Ok(())
    }

    #[gtest]
    fn fn_scope_works() -> Result<()> {
        let lines = vec![
            "// #rw(fn: rust_panic) error reporting function",
            "fn validate() {",
            "    let x = format!(\"a\");",
            "    let y = format!(\"b\");",
            "}",
            "fn other() {}",
        ];
        let mut errors = make_errors();
        let sup = suppressed_lines("t.rs", &lines, &mut errors, None);

        verify_true!(sup.lines.contains_key(&2))?;
        verify_true!(sup.lines.contains_key(&3))?;
        verify_true!(sup.lines.contains_key(&4))?;
        verify_true!(sup.lines.contains_key(&5))?;
        verify_false!(sup.lines.contains_key(&6))?;
        verify_true!(errors.is_empty())?;

        Ok(())
    }

    #[gtest]
    // #rw(fn: rust_duplicate_strings) This linter scenario intentionally repeats source fixtures used to verify independent paths.
    fn fn_scope_ignores_braces_in_strings() -> Result<()> {
        let lines = vec![
            // #rw(rust_duplicate_strings) This linter fixture intentionally repeats source text across independent rule-harness scenarios.
            "// #rw(fn: rust_panic) error reporting function",
            "fn validate() {",
            "    let s = \"{ not a real brace }\";",
            "    let x = format!(\"a\");",
            "}",
            "fn other() {}",
        ];
        let mut errors = make_errors();
        let sup = suppressed_lines("t.rs", &lines, &mut errors, None);

        verify_true!(sup.lines.contains_key(&2))?;
        verify_true!(sup.lines.contains_key(&3))?;
        verify_true!(sup.lines.contains_key(&4))?;
        verify_true!(sup.lines.contains_key(&5))?;
        verify_false!(sup.lines.contains_key(&6))?;
        verify_true!(errors.is_empty())?;

        Ok(())
    }

    #[gtest]
    // #rw(fn: rust_duplicate_strings) This linter scenario intentionally repeats source fixtures used to verify independent paths.
    fn fn_scope_ignores_braces_in_comments() -> Result<()> {
        let lines = vec![
            // #rw(rust_duplicate_strings) This linter fixture intentionally repeats source text across independent rule-harness scenarios.
            "// #rw(fn: rust_panic) error reporting function",
            "fn validate() {",
            "    // closing } here doesn't count",
            "    let x = format!(\"a\");",
            "}",
            "fn other() {}",
        ];
        let mut errors = make_errors();
        let sup = suppressed_lines("t.rs", &lines, &mut errors, None);

        verify_true!(sup.lines.contains_key(&2))?;
        verify_true!(sup.lines.contains_key(&3))?;
        verify_true!(sup.lines.contains_key(&4))?;
        verify_true!(sup.lines.contains_key(&5))?;
        verify_false!(sup.lines.contains_key(&6))?;
        verify_true!(errors.is_empty())?;

        Ok(())
    }

    #[gtest]
    // #rw(fn: rust_duplicate_strings) This linter scenario intentionally repeats source fixtures used to verify independent paths.
    fn fn_scope_ignores_braces_in_char_literals() -> Result<()> {
        let lines = vec![
            // #rw(rust_duplicate_strings) This linter fixture intentionally repeats source text across independent rule-harness scenarios.
            "// #rw(fn: rust_panic) error reporting function",
            "fn validate() {",
            "    let c = '{';",
            "    let d = '}';",
            "    let x = format!(\"a\");",
            "}",
            "fn other() {}",
        ];
        let mut errors = make_errors();
        let sup = suppressed_lines("t.rs", &lines, &mut errors, None);

        verify_true!(sup.lines.contains_key(&2))?;
        verify_true!(sup.lines.contains_key(&3))?;
        verify_true!(sup.lines.contains_key(&4))?;
        verify_true!(sup.lines.contains_key(&5))?;
        verify_true!(sup.lines.contains_key(&6))?;
        verify_false!(sup.lines.contains_key(&7))?;
        verify_true!(errors.is_empty())?;

        Ok(())
    }
}

#[cfg(test)]
mod filter_tests {
    use super::*;

    fn make_errors() -> Vec<Violation> {
        Vec::new()
    }

    fn empty_suppressions() -> Suppressions {
        Suppressions {
            lines: HashMap::new(),
            file_rules: Vec::new(),
            file_all: false,
            entries: Vec::new(),
        }
    }

    #[gtest]
    fn wildcard_works() -> Result<()> {
        let lines = vec!["// #rw(*) generated code", "let x = 1;"];
        let mut errors = make_errors();
        let sup = suppressed_lines("t.rs", &lines, &mut errors, None);

        verify_eq!(sup.lines.get(&2).or_fail()?, &["*"])?;
        verify_true!(errors.is_empty())?;

        Ok(())
    }

    #[gtest]
    fn wildcard_file_works() -> Result<()> {
        let lines = vec!["// #rw(file: *) fully generated"];
        let mut errors = make_errors();
        let sup = suppressed_lines("t.rs", &lines, &mut errors, None);

        verify_true!(sup.file_all)?;
        verify_true!(is_file_suppressed(&sup, "anything"))?;
        verify_true!(errors.is_empty())?;

        Ok(())
    }

    #[gtest]
    fn wildcard_suppresses_any_rule() -> Result<()> {
        let mut sup = empty_suppressions();

        sup.lines.insert(2, vec!["*".to_string()]);

        let violations = vec![
            violation("t.rs", 2, "a").with_rule("panic"),
            violation("t.rs", 2, "b").with_rule("unwrap_in_lib"),
        ];
        let filtered = filter(&sup, violations);

        verify_true!(filtered.is_empty())?;

        Ok(())
    }

    #[gtest]
    fn suppression_only_applies_to_named_rule() -> Result<()> {
        let mut sup = empty_suppressions();

        sup.lines.insert(2, vec!["panic".to_string()]);

        let violations = vec![
            violation("t.rs", 2, "a").with_rule("panic"),
            violation("t.rs", 2, "b").with_rule("unwrap_in_lib"),
        ];
        let filtered = filter(&sup, violations);

        verify_eq!(filtered.len(), 1)?;
        verify_eq!(filtered[0].rule, Some("unwrap_in_lib"))?;

        Ok(())
    }

    #[gtest]
    fn filter_removes_matching_rule_only() -> Result<()> {
        let mut sup = empty_suppressions();

        sup.lines.insert(2, vec!["panic".to_string()]);
        sup.lines.insert(3, vec!["panic".to_string()]);

        let violations = vec![
            violation("t.rs", 1, "a").with_rule("panic"),
            violation("t.rs", 2, "b").with_rule("panic"),
            violation("t.rs", 3, "c").with_rule("style"),
            violation("t.rs", 4, "d").with_rule("panic"),
        ];
        let filtered = filter(&sup, violations);

        verify_eq!(filtered.len(), 3)?;
        verify_eq!(filtered[0].line, 1)?;
        verify_eq!(filtered[1].line, 3)?;
        verify_eq!(filtered[2].line, 4)?;

        Ok(())
    }
}
