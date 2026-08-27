// #rw(file: rust_alloc_in_loop, rust_default_hasher) clean is a cold developer-tool path with trusted rule-name keys and explicit edit reports
// #rw(file: rust_inline_test_module_size) cleanup tests require private directive and edit helpers

use std::{
    collections::{BTreeSet, HashSet},
    process::ExitCode,
};

use crate::output;
#[cfg(test)]
use googletest::prelude::*;

use super::RunCtx;
use crate::{
    FileCtx, Fix, Rule, Violation,
    infra::{fix, ignore::SuppressionEntry, parse, walk},
    languages::rust::{self, SuppressionAuditError},
};

const DEFAULT_FILE_SIZE_THRESHOLD: usize = 1500;
const PREVIOUS_LINE_OFFSET: usize = 2;

#[derive(Debug)]
struct Removal {
    rel: String,
    line: usize,
    targets: Box<[String]>,
}

#[derive(Debug, Default)]
struct CleanPlan {
    fixes: Vec<(String, Fix)>,
    snapshots: fix::SourceSnapshots,
    removals: Vec<Removal>,
    failures: Vec<Box<str>>,
    generated_files: usize,
}

#[derive(Clone, Copy)]
enum RuleScope {
    Complete,
    Filtered,
}

#[derive(Debug)]
struct AuditedFile {
    rel: String,
    contents: String,
    suppressions: crate::infra::ignore::Suppressions,
    raw_violations: Vec<Violation>,
}

#[derive(Debug)]
enum AuditedPath {
    Generated,
    Source(Box<AuditedSource>),
}

#[derive(Debug)]
struct AuditedSource {
    selected: Option<AuditedFile>,
    workspace_file: crate::languages::workspace::WorkspaceRustFile,
}

#[derive(Debug)]
enum EntryPlanError {
    Render,
}

/// Remove only suppression targets that do not cover an underlying violation.
#[must_use]
pub fn clean_suppressions(ctx: &RunCtx<'_>, dry_run: bool) -> ExitCode {
    let mut plan = build_plan(ctx);

    if !plan.failures.is_empty() {
        return report_failures(plan.failures, ctx.quiet);
    }

    plan.removals
        .sort_by(|a, b| a.rel.cmp(&b.rel).then(a.line.cmp(&b.line)));

    if plan.removals.is_empty() {
        if !ctx.quiet {
            output::success("no unused suppression directives found");

            if plan.generated_files > 0 {
                output::detail(&format!(
                    "skipped {} generated Rust file(s)",
                    plan.generated_files
                ));
            }

            output::blank();
        }

        return ExitCode::SUCCESS;
    }

    let target_count: usize = plan
        .removals
        .iter()
        .map(|removal| removal.targets.len())
        .sum();

    if !ctx.quiet {
        print_removals(&plan.removals);
    }

    if dry_run {
        if !ctx.quiet {
            super::report::print_dry_run(&plan.fixes);
            output::detail(&format!(
                "{target_count} unused suppression target(s) would be removed (dry run)"
            ));
            output::blank();
        }

        return ExitCode::SUCCESS;
    }

    let applied = match fix::apply_fixes(&plan.fixes, &plan.snapshots, ctx.root) {
        Ok(applied) => applied,
        Err(error) => {
            if !ctx.quiet {
                output::error(&format!("failed to apply directive edits: {error}"));
                output::blank();
            }

            return ExitCode::FAILURE;
        }
    };

    if applied != plan.fixes.len() {
        if !ctx.quiet {
            output::error(&format!(
                "applied {applied} of {} planned directive edit(s)",
                plan.fixes.len()
            ));
            output::blank();
        }

        return ExitCode::FAILURE;
    }

    if !ctx.quiet {
        output::success(&format!(
            "removed {target_count} unused suppression target(s) in {applied} directive(s)"
        ));
        output::blank();
    }

    ExitCode::SUCCESS
}

fn report_failures(mut failures: Vec<Box<str>>, quiet: bool) -> ExitCode {
    failures.sort_unstable();

    if !quiet {
        for failure in failures {
            output::error(&failure);
        }

        output::blank();
        output::error("clean aborted without writing because suppression analysis was incomplete");
        output::blank();
    }

    ExitCode::FAILURE
}

fn build_plan(ctx: &RunCtx<'_>) -> CleanPlan {
    let rules: Vec<&Rule> = ctx
        .registry
        .rules()
        .iter()
        .copied()
        .filter(|rule| {
            rule.check.extensions().contains(&"rs")
                && ctx.config.is_enabled(rule.info.name)
                && super::matches_rule_filter(rule.info.name, ctx.rule_filter)
        })
        .collect();
    let selected_rule_names: HashSet<&str> = rules.iter().map(|rule| rule.info.name).collect();
    let rule_scope = if ctx
        .registry
        .rules()
        .iter()
        .filter(|rule| {
            rule.check.extensions().contains(&"rs") && ctx.config.is_enabled(rule.info.name)
        })
        .count()
        == rules.len()
    {
        RuleScope::Complete
    } else {
        RuleScope::Filtered
    };
    let registered_names: HashSet<&str> = ctx
        .registry
        .rules()
        .iter()
        .copied()
        .map(|rule| rule.info.name)
        .collect();
    let mut plan = CleanPlan::default();
    let (selected_rels, paths) = match cleaner_paths(ctx) {
        Ok(paths) => paths,
        Err(error) => {
            plan.failures.push(error.to_string().into_boxed_str());

            return plan;
        }
    };
    let mut audited_files = Vec::new();
    let mut workspace_files = Vec::new();

    for path in paths {
        let rel = match relative_path(ctx, &path) {
            Ok(rel) => rel,
            Err(error) => {
                plan.failures.push(error);
                continue;
            }
        };
        let selected = selected_rels.contains(&rel);

        match audit_path(ctx, &path, rel, selected, &rules, &registered_names) {
            Ok(AuditedPath::Generated) => {
                plan.generated_files += usize::from(selected);
            }
            Ok(AuditedPath::Source(source)) => {
                workspace_files.push(source.workspace_file);

                if let Some(file) = source.selected {
                    audited_files.push(file);
                }
            }
            Err(failures) => plan.failures.extend(failures),
        }
    }

    if !plan.failures.is_empty() {
        return plan;
    }

    for violation in rust::audit_workspace_suppressions(&workspace_files, &rules, ctx.config) {
        if let Some(file) = audited_files
            .iter_mut()
            .find(|file| file.rel == violation.rel)
        {
            file.raw_violations.push(violation);
        }
    }

    for file in &mut audited_files {
        file.raw_violations
            .sort_by(|a, b| a.line.cmp(&b.line).then(a.rule.cmp(&b.rule)));
    }

    for file in audited_files {
        plan_file(
            ctx,
            file,
            &rules,
            &selected_rule_names,
            rule_scope,
            &mut plan,
        );
    }

    plan
}

fn cleaner_paths(
    ctx: &RunCtx<'_>,
) -> Result<(BTreeSet<String>, Vec<crate::path::PathBuf>), Box<str>> {
    let selected_paths = if ctx.dirty {
        walk::git_dirty_paths(ctx.root, ctx.workspace_root, ctx.package_filter, &["rs"])
            .map_err(|error| error.to_string().into_boxed_str())?
    } else {
        walk::rs_paths(ctx.workspace_root, ctx.package_filter)
            .map_err(|error| error.to_string().into_boxed_str())?
    };
    let selected_rels = selected_paths
        .iter()
        .map(|path| relative_path(ctx, path))
        .collect::<Result<_, _>>()?;
    let paths = walk::rs_paths(ctx.workspace_root, &[])
        .map_err(|error| error.to_string().into_boxed_str())?;

    Ok((selected_rels, paths))
}

fn audit_path(
    ctx: &RunCtx<'_>,
    path: &crate::path::Path,
    rel: String,
    selected: bool,
    rules: &[&Rule],
    registered_names: &HashSet<&str>,
) -> Result<AuditedPath, Vec<Box<str>>> {
    let contents = match crate::file::read_text(path) {
        Ok(contents) => contents,
        Err(error) => {
            return Err(vec![
                format!("{rel}: failed to read source: {error}").into_boxed_str(),
            ]);
        }
    };

    if is_generated(&contents) {
        return Ok(AuditedPath::Generated);
    }

    let lines: Vec<&str> = contents.lines().collect();
    let file = FileCtx {
        rel: &rel,
        path,
        package_name: super::package_name_for_path(ctx.packages, path),
        lines: &lines,
        contents: &contents,
        config: ctx.config,
    };
    let audit = match rust::audit_suppressions(&file, rules, registered_names) {
        Ok(audit) => audit,
        Err(SuppressionAuditError::RustSyntax) => {
            return Err(vec![
                format!("{rel}: Rust syntax could not be parsed").into_boxed_str(),
            ]);
        }
        Err(SuppressionAuditError::InvalidDirectives(errors)) => {
            return Err(errors
                .into_iter()
                .map(|error| {
                    format!("{}:{}: {}", error.rel, error.line, error.message).into_boxed_str()
                })
                .collect());
        }
    };

    let selected = selected.then_some(AuditedFile {
        rel,
        contents,
        suppressions: audit.suppressions,
        raw_violations: audit.raw_violations,
    });

    Ok(AuditedPath::Source(Box::new(AuditedSource {
        selected,
        workspace_file: audit.workspace_file,
    })))
}

fn relative_path(ctx: &RunCtx<'_>, path: &crate::path::Path) -> Result<String, Box<str>> {
    let relative = path
        .strip_prefix(ctx.root)
        .unwrap_or(path)
        .to_slash()
        .ok_or_else(|| {
            format!(
                "{}: non-UTF-8 source paths cannot be analyzed safely",
                path.display()
            )
            .into_boxed_str()
        })?;

    Ok(relative)
}

fn plan_file(
    ctx: &RunCtx<'_>,
    audited: AuditedFile,
    rules: &[&Rule],
    selected_rules: &HashSet<&str>,
    rule_scope: RuleScope,
    plan: &mut CleanPlan,
) {
    let AuditedFile {
        rel,
        contents,
        suppressions,
        raw_violations,
    } = audited;
    let lines: Vec<&str> = contents.lines().collect();

    plan.snapshots
        .insert(rel.clone(), crate::checksum::bytes(&contents));

    for entry in suppressions.entries {
        let source_line = lines
            .get(entry.line.saturating_sub(1))
            .copied()
            .unwrap_or("");
        let budget_active = entry.values.contains_key("rust_too_many_lines_in_file")
            && lines.len() > file_size_threshold(ctx.config, rules);
        let Ok(planned) = plan_entry(
            &entry,
            &raw_violations,
            source_line,
            budget_active,
            selected_rules,
            rule_scope,
        ) else {
            plan.failures.push(
                format!(
                    "{rel}:{}: failed to render suppression directive",
                    entry.line
                )
                .into_boxed_str(),
            );
            continue;
        };
        let Some((mut fix, stale)) = planned else {
            continue;
        };

        if fix.replacement.is_empty() && redundant_separator_after(&entry, &lines) {
            fix.end_line = fix.end_line.saturating_add(1);
        }

        // #rw(rust_clone_in_loop) the edit and report entry independently own the stable relative path
        plan.fixes.push((rel.clone(), fix));
        plan.removals.push(Removal {
            // #rw(rust_clone_in_loop) the edit and report entry independently own the stable relative path
            rel: rel.clone(),
            line: entry.line,
            targets: stale.into_boxed_slice(),
        });
    }
}

fn redundant_separator_after(entry: &SuppressionEntry, lines: &[&str]) -> bool {
    let next_is_blank = lines
        .get(entry.line)
        .is_some_and(|line| line.trim().is_empty());
    let previous_is_blank = entry
        .line
        .checked_sub(PREVIOUS_LINE_OFFSET)
        .and_then(|index| lines.get(index))
        .is_some_and(|line| line.trim().is_empty());

    next_is_blank
        && (entry.line == 1 || previous_is_blank || matches!(entry.scope, parse::Scope::File))
}

fn print_removals(removals: &[Removal]) {
    for removal in removals {
        output::detail(&format!(
            "{}:{} remove {}",
            removal.rel,
            removal.line,
            removal.targets.join(", ")
        ));
    }

    output::blank();
}

fn plan_entry(
    entry: &SuppressionEntry,
    raw_violations: &[Violation],
    source_line: &str,
    budget_active: bool,
    selected_rules: &HashSet<&str>,
    rule_scope: RuleScope,
) -> Result<Option<(Fix, Vec<String>)>, EntryPlanError> {
    if entry.wildcard {
        if matches!(rule_scope, RuleScope::Filtered) {
            return Ok(None);
        }

        return Ok((!raw_violations
            .iter()
            .any(|violation| entry.suppresses(violation)))
        .then(|| (Fix::delete(entry.line, entry.line), vec!["*".to_string()])));
    }

    let active: Vec<&str> = entry
        .rules
        .iter()
        .filter(|target| {
            !selected_rules.contains(target.as_str())
                || (target.as_str() == "rust_too_many_lines_in_file" && budget_active)
                || raw_violations.iter().any(|violation| {
                    violation.rule == Some(target.as_str()) && entry.covers_line(violation.line)
                })
        })
        .map(String::as_str)
        .collect();
    let stale: Vec<String> = entry
        .rules
        .iter()
        .filter(|target| {
            selected_rules.contains(target.as_str()) && !active.contains(&target.as_str())
        })
        .cloned()
        .collect();

    if stale.is_empty() {
        return Ok(None);
    }

    let fix = if active.is_empty() {
        Fix::delete(entry.line, entry.line)
    } else {
        let rendered: Vec<String> = active
            .iter()
            .map(|target| {
                entry.values.get(*target).map_or_else(
                    || (*target).to_owned(),
                    |value| format!("{target} = {value}"),
                )
            })
            .collect();
        let rendered_refs: Vec<&str> = rendered.iter().map(String::as_str).collect();
        let replacement = parse::rewrite_directive_rules(source_line, &entry.scope, &rendered_refs)
            .ok_or(EntryPlanError::Render)?;

        Fix::replace_line(entry.line, replacement)
    };

    Ok(Some((fix, stale)))
}

fn file_size_threshold(config: &crate::Config, rules: &[&Rule]) -> usize {
    let parameter = rules
        .iter()
        .find(|rule| rule.info.name == "rust_too_many_lines_in_file")
        .and_then(|rule| rule.info.params.first());

    parameter.map_or(DEFAULT_FILE_SIZE_THRESHOLD, |param| {
        config.get_usize("rust_too_many_lines_in_file", param)
    })
}

fn is_generated(contents: &str) -> bool {
    const GENERATED_HEADER_LINES: usize = 8;

    contents
        .lines()
        .take(GENERATED_HEADER_LINES)
        .any(|line| line.contains("@generated"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{infra::ignore, violation};

    fn entries(source: &str) -> Result<Vec<SuppressionEntry>> {
        let lines: Vec<&str> = source.lines().collect();
        let mut errors = Vec::new();
        let suppressions = ignore::suppressed_lines("fixture.rs", &lines, &mut errors, None);

        verify_true!(errors.is_empty())?;

        Ok(suppressions.entries)
    }

    fn tagged(line: usize, rule: &'static str) -> Violation {
        violation("fixture.rs", line, "fixture violation").with_rule(rule)
    }

    fn plan_entry_all(
        entry: &SuppressionEntry,
        raw_violations: &[Violation],
        source_line: &str,
        budget_active: bool,
    ) -> Result<Option<(Fix, Vec<String>)>, EntryPlanError> {
        let selected_rules = entry.rules.iter().map(String::as_str).collect();

        plan_entry(
            entry,
            raw_violations,
            source_line,
            budget_active,
            &selected_rules,
            RuleScope::Complete,
        )
    }

    #[gtest]
    fn clean_fails_without_changing_source_when_persistence_is_locked() -> Result<()> {
        let directory = crate::temporary::Directory::new().or_fail()?;
        let source = directory.path().join("source.rs");
        let contents = "// #rw(rust_dbg) stale fixture\nfn clean() {}\n";

        crate::file::write_text(&source, contents).or_fail()?;
        let registry = crate::RuleRegistry::with_builtins().or_fail()?;
        let metadata = registry.metadata();
        let config = crate::Config::generate_default(
            &metadata
                .iter()
                .map(|rule| (rule.name, rule.params))
                .collect::<Vec<_>>(),
        );
        let ctx = RunCtx {
            registry: &registry,
            rule_filter: &[],
            package_filter: &[],
            packages: &[],
            config: &config,
            root: directory.path(),
            workspace_root: directory.path(),
            quiet: true,
            dirty: false,
        };
        let _lock =
            crate::lock::Lock::try_acquire(&directory.path().join(".rulewright.lock")).or_fail()?;

        verify_eq!(clean_suppressions(&ctx, false), ExitCode::FAILURE)?;

        verify_eq!(crate::file::read_text(&source).or_fail()?, contents)
    }

    #[gtest]
    fn clean_rule_filter_leaves_unrelated_directive_bytes_unchanged() -> Result<()> {
        let directory = crate::temporary::Directory::new().or_fail()?;
        let source = directory.path().join("source.rs");
        let contents = "// #rw(rust_dbg) stale fixture\nfn first() {}\n// #rw(rust_panic) keep these bytes exactly\nfn second() {}\n";

        crate::file::write_text(&source, contents).or_fail()?;
        let registry = crate::RuleRegistry::with_builtins().or_fail()?;
        let metadata = registry.metadata();
        let config = crate::Config::generate_default(
            &metadata
                .iter()
                .map(|rule| (rule.name, rule.params))
                .collect::<Vec<_>>(),
        );
        let rule_filter = vec!["rust_dbg".to_string()];
        let ctx = RunCtx {
            registry: &registry,
            rule_filter: &rule_filter,
            package_filter: &[],
            packages: &[],
            config: &config,
            root: directory.path(),
            workspace_root: directory.path(),
            quiet: true,
            dirty: false,
        };

        verify_eq!(clean_suppressions(&ctx, false), ExitCode::SUCCESS)?;

        verify_eq!(
            crate::file::read_text(&source).or_fail()?,
            "fn first() {}\n// #rw(rust_panic) keep these bytes exactly\nfn second() {}\n"
        )
    }

    #[gtest]
    fn generated_header_is_detected_without_scanning_body() -> Result<()> {
        verify_true!(is_generated(
            "//! @generated by codegen\npub fn generated() {}"
        ))?;
        verify_false!(is_generated(
            "\n\n\n\n\n\n\n\n// @generated appears too late\npub fn handwritten() {}\n"
        ))?;

        Ok(())
    }

    #[gtest]
    fn mixed_targets_rewrite_only_the_target_list() -> Result<()> {
        let source =
            "    // #rw(fn: rust_dbg, rust_panic) keep this exact reason\nfn f() { dbg!(1); }";
        let entry = entries(source)?.pop().or_fail()?;

        let (fix, stale) = plan_entry_all(
            &entry,
            &[tagged(2, "rust_dbg")],
            source.lines().next().or_fail()?,
            false,
        )
        .or_fail()?
        .or_fail()?;

        verify_eq!(stale, ["rust_panic"])?;
        verify_eq!(
            fix.replacement,
            "    // #rw(fn: rust_dbg) keep this exact reason"
        )?;

        Ok(())
    }

    #[gtest]
    fn fully_stale_directive_deletes_only_its_line() -> Result<()> {
        let source = "// #rw(rust_panic) stale target\nlet value = 1;";
        let entry = entries(source)?.pop().or_fail()?;

        let (fix, stale) = plan_entry_all(&entry, &[], source.lines().next().or_fail()?, false)
            .or_fail()?
            .or_fail()?;

        verify_eq!(stale, ["rust_panic"])?;
        verify_true!(fix.replacement.is_empty())?;
        verify_eq!((fix.start_line, fix.end_line), (1, 1))?;

        Ok(())
    }

    #[gtest]
    fn stale_file_directive_deletes_its_required_separator() -> Result<()> {
        let source = "// #rw(file: rust_panic) stale target\n\nfn retained() {}";
        let entry = entries(source)?.pop().or_fail()?;
        let lines: Vec<&str> = source.lines().collect();

        verify_true!(redundant_separator_after(&entry, &lines))
    }

    #[gtest]
    fn wildcard_is_atomic() -> Result<()> {
        let source = "// #rw(*) fixture wildcard\ndbg!(1);";
        let entry = entries(source)?.pop().or_fail()?;

        verify_true!(
            plan_entry_all(
                &entry,
                &[tagged(2, "rust_dbg")],
                source.lines().next().or_fail()?,
                false,
            )
            .or_fail()?
            .is_none()
        )?;
        let (_, stale) = plan_entry_all(&entry, &[], source.lines().next().or_fail()?, false)
            .or_fail()?
            .or_fail()?;

        verify_eq!(stale, ["*"])?;

        Ok(())
    }

    #[gtest]
    fn every_scope_keeps_a_target_with_a_covered_violation() -> Result<()> {
        let cases = [
            ("// #rw(rust_dbg) next\ndbg!(1);", 2),
            ("// #rw(file: rust_dbg) file\nfn f() {}", 20),
            ("// #rw(block: rust_dbg) block\ndbg!(1);\n", 2),
            (
                "// #rw(fn: rust_dbg) function\nfn f() {\n    dbg!(1);\n}",
                3,
            ),
        ];

        for (source, violation_line) in cases {
            let entry = entries(source)?.pop().or_fail()?;

            verify_true!(
                plan_entry_all(
                    &entry,
                    &[tagged(violation_line, "rust_dbg")],
                    source.lines().next().or_fail()?,
                    false,
                )
                .or_fail()?
                .is_none()
            )?;
        }

        Ok(())
    }

    #[gtest]
    fn overlapping_active_directives_are_both_preserved() -> Result<()> {
        let source =
            "// #rw(file: rust_dbg) first\n// #rw(file: rust_dbg) second\nfn f() { dbg!(1); }";
        let raw = [tagged(3, "rust_dbg")];

        for entry in entries(source)? {
            verify_true!(
                plan_entry_all(
                    &entry,
                    &raw,
                    source.lines().nth(entry.line - 1).or_fail()?,
                    false,
                )
                .or_fail()?
                .is_none()
            )?;
        }

        Ok(())
    }

    #[gtest]
    fn file_size_budget_is_preserved_above_base_threshold() -> Result<()> {
        let source =
            "// #rw(file: rust_too_many_lines_in_file = 1600) cohesive generated-style table";
        let entry = entries(source)?.pop().or_fail()?;

        verify_true!(
            plan_entry_all(&entry, &[], source, true)
                .or_fail()?
                .is_none()
        )?;

        Ok(())
    }

    #[gtest]
    fn file_size_budget_is_stale_below_base_threshold() -> Result<()> {
        let source = "// #rw(file: rust_too_many_lines_in_file = 1600) old budget";
        let entry = entries(source)?.pop().or_fail()?;
        let (fix, stale) = plan_entry_all(&entry, &[], source, false)
            .or_fail()?
            .or_fail()?;

        verify_eq!(stale, ["rust_too_many_lines_in_file"])?;
        verify_true!(fix.replacement.is_empty())?;

        Ok(())
    }

    #[gtest]
    fn mixed_directive_preserves_numeric_budget_when_other_target_is_stale() -> Result<()> {
        let source = "// #rw(file: rust_too_many_lines_in_file = 1600, rust_dbg) exact reason";
        let entry = entries(source)?.pop().or_fail()?;
        let (fix, stale) = plan_entry_all(&entry, &[], source, true)
            .or_fail()?
            .or_fail()?;

        verify_eq!(stale, ["rust_dbg"])?;
        verify_eq!(
            fix.replacement,
            "// #rw(file: rust_too_many_lines_in_file = 1600) exact reason"
        )?;

        Ok(())
    }
}
