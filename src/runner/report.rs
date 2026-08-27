// #rw(file: rust_alloc_in_loop) violation report renderer
// #rw(file: rust_default_hasher) trusted rule-name sets in the cold report path; fast-hasher dependency not warranted

use std::{collections::BTreeMap, process::ExitCode};

use crate::output;
use crate::{
    file,
    path::{Path, PathBuf},
};
use tabled::Tabled;

use crate::{
    Config, Fix, RuleRegistry, Violation,
    infra::{ignore, walk},
};

/// Print every `#rw(...)` directive found across the selected packages as a grouped table.
pub fn report_suppressions(
    registry: &RuleRegistry,
    package_filter: &[String],
    config: &Config,
    root: &Path,
    workspace_root: &Path,
    quiet: bool,
) -> ExitCode {
    let paths = match walk::rs_paths(workspace_root, package_filter) {
        Ok(paths) => paths,
        Err(error) => {
            if !quiet {
                output::error(&format!("suppression report aborted: {error}"));
            }

            return ExitCode::FAILURE;
        }
    };

    let registered_names: std::collections::HashSet<&str> = registry
        .rules()
        .iter()
        .copied()
        .map(|rule| rule.info.name)
        .collect();

    let all_entries = match load_suppressions(&paths, root, &registered_names) {
        Ok(entries) => entries,
        Err(failures) => return report_read_failures(failures, quiet),
    };

    if quiet {
        // #rw(rust_println) quiet mode only prints count
        eprintln!("{}", all_entries.len());

        return ExitCode::SUCCESS;
    }

    if all_entries.is_empty() {
        output::success("no suppression directives found");
        output::blank();

        return ExitCode::SUCCESS;
    }

    let _ = config;
    let mut by_rule: BTreeMap<String, Vec<&ignore::SuppressionEntry>> = BTreeMap::new();

    for entry in &all_entries {
        if entry.wildcard {
            by_rule.entry("*".to_string()).or_default().push(entry);
        } else {
            for rule in &entry.rules {
                // #rw(rust_clone_in_loop) need owned key for map
                by_rule.entry(rule.clone()).or_default().push(entry);
            }
        }
    }

    let mut rows: Vec<SuppressionRow> = Vec::with_capacity(by_rule.len());

    for (rule, entries) in by_rule {
        output::header(&format!("{rule} ({} suppression(s))", entries.len()));

        for e in &entries {
            output::detail(&format!(
                "  {}:{} [{}] {}",
                e.rel,
                e.line,
                e.scope.prefix(),
                e.reason,
            ));
        }

        output::blank();
        rows.push(SuppressionRow {
            rule,
            count: entries.len(),
        });
    }

    rows.sort_by_key(|a| std::cmp::Reverse(a.count));
    output::separator();
    output::blank();
    output::table(rows);
    output::blank();
    output::detail(&format!(
        "{} directive(s) across {} file(s)",
        all_entries.len(),
        {
            let mut files: std::collections::HashSet<&str> = std::collections::HashSet::new();
            for e in &all_entries {
                files.insert(&e.rel);
            }
            files.len()
        }
    ));
    output::blank();

    ExitCode::SUCCESS
}

fn load_suppressions(
    paths: &[PathBuf],
    root: &Path,
    registered_names: &std::collections::HashSet<&str>,
) -> Result<Vec<ignore::SuppressionEntry>, Vec<String>> {
    let mut all_entries = Vec::new();
    let mut failures = Vec::with_capacity(paths.len());

    for path in paths {
        let relative = path.strip_prefix(root).unwrap_or(path);
        let Some(rel) = relative.to_slash() else {
            failures.push(format!(
                "{}: non-UTF-8 source paths cannot be reported safely",
                path.display()
            ));
            continue;
        };
        let contents = match file::read_text(path) {
            Ok(contents) => contents,
            Err(error) => {
                failures.push(format!("{rel}: failed to read source: {error}"));
                continue;
            }
        };
        let lines: Vec<&str> = contents.lines().collect();
        let directive_lines = crate::infra::scanner::directive_source_lines(&contents, &lines);
        let mut errors = Vec::new();
        let suppressed =
            ignore::suppressed_lines(&rel, &directive_lines, &mut errors, Some(registered_names));

        all_entries.extend(suppressed.entries);
    }

    if failures.is_empty() {
        Ok(all_entries)
    } else {
        failures.sort_unstable();
        failures.dedup();

        Err(failures)
    }
}

fn report_read_failures(failures: Vec<String>, quiet: bool) -> ExitCode {
    if !quiet {
        for failure in failures {
            output::error(&failure);
        }

        output::error("suppression report aborted because analysis was incomplete");
    }

    ExitCode::FAILURE
}

#[derive(Tabled)]
#[tabled(crate = "tabled")]
struct SuppressionRow {
    #[tabled(rename = "Rule")]
    rule: String,
    #[tabled(rename = "Count")]
    count: usize,
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    #[cfg(unix)]
    #[test]
    fn suppression_report_rejects_non_utf8_path_identity() {
        use std::os::unix::ffi::OsStringExt as _;

        let directory = crate::temporary::Directory::new().expect("temporary directory");
        let name = std::ffi::OsString::from_vec(b"source-\xff.rs".to_vec());
        let path = directory.path().join(std::path::PathBuf::from(name));

        file::write_text(&path, "fn source() {}\n").expect("fixture source");
        let errors = load_suppressions(&[path], directory.path(), &HashSet::new())
            .expect_err("lossy report identity must fail");

        assert!(errors[0].contains("non-UTF-8 source paths"));
    }
}

pub(super) fn print_grouped(
    violations: &[Violation],
    registry: &RuleRegistry,
    packages: &[super::PackageRoot],
    root: &Path,
) {
    let mut by_package: BTreeMap<&str, Vec<&Violation>> = BTreeMap::new();
    let mut by_rule: BTreeMap<&str, usize> = BTreeMap::new();

    for v in violations {
        by_package
            .entry(package_name(&v.rel, packages, root))
            .or_default()
            .push(v);
        *by_rule.entry(v.rule_name()).or_default() += 1;
    }

    for (package_name, violations) in &by_package {
        output::header(&format!("{package_name} ({} issues)", violations.len()));

        for v in violations {
            output::error(&format!("{}:{}: {}", v.rel, v.line, v.message));
        }

        output::blank();
    }

    output::separator();
    output::blank();

    let mut fixable_rules: std::collections::HashSet<&str> = std::collections::HashSet::new();

    for rule in registry.rules() {
        if rule.fix.is_some() {
            fixable_rules.insert(rule.info.name);
        }
    }

    let mut rule_counts: Vec<RuleSummaryRow> = by_rule
        .into_iter()
        .map(|(rule, count)| RuleSummaryRow {
            rule: rule.to_string(),
            fixable: if fixable_rules.contains(rule) {
                "yes".to_string()
            } else {
                String::new()
            },
            count,
        })
        .collect();

    rule_counts.sort_by_key(|a| std::cmp::Reverse(a.count));

    output::table(rule_counts);
    output::blank();

    let fixable_count: usize = violations
        .iter()
        .filter(|v| fixable_rules.contains(v.rule_name()))
        .count();

    output::error(&format!(
        "{} violation(s) in {} package(s)",
        violations.len(),
        by_package.len()
    ));

    if fixable_count > 0 {
        output::detail(&format!(
            "{fixable_count} of these can be auto-fixed with --fix"
        ));
    }
}

#[derive(Tabled)]
#[tabled(crate = "tabled")]
struct RuleSummaryRow {
    #[tabled(rename = "Rule")]
    rule: String,
    #[tabled(rename = "Fixable")]
    fixable: String,
    #[tabled(rename = "Count")]
    count: usize,
}

// #rw(fn: rust_alloc_in_loop) terminal output requires format! per fix
pub(super) fn print_dry_run(fixes: &[(String, Fix)]) {
    let mut by_file: BTreeMap<&str, Vec<&Fix>> = BTreeMap::new();

    for (rel, fix) in fixes {
        by_file.entry(rel).or_default().push(fix);
    }

    for (rel, file_fixes) in &by_file {
        output::header(&format!("{rel} ({} fix(es))", file_fixes.len()));

        for f in file_fixes {
            if f.replacement.is_empty() {
                output::detail(&format!("  delete lines {}-{}", f.start_line, f.end_line));
            } else {
                let new_lines = f.replacement.lines().count();

                output::detail(&format!(
                    "  replace lines {}-{} ({} line(s))",
                    f.start_line, f.end_line, new_lines
                ));
            }
        }

        output::blank();
    }
}

fn package_name<'a>(rel: &'a str, packages: &'a [super::PackageRoot], root: &Path) -> &'a str {
    super::package_name_for_path(packages, &root.join(rel)).unwrap_or(rel)
}
