// #rw(file: rust_default_hasher) trusted rule-name sets in the cold runner setup path; fast-hasher dependency not warranted
// #rw(file: rust_inline_test_module_size) runner tests require private collection and dispatch helpers

//! Rule execution, caching, reporting, fixing, and suppression cleanup.

mod baseline;
mod cache;
mod clean;
mod report;

use std::collections::{BTreeMap, HashSet};

use crate::output;
use crate::{
    file,
    path::{Path, PathBuf},
};
use rayon::prelude::*;
use report::{print_dry_run, print_grouped, print_json};

use crate::{
    Config, FileCtx, Fix, Rule, RuleRegistry, Violation,
    infra::{
        fix::{self, TreeFix},
        walk,
    },
    languages::{self, workspace::WorkspaceCtx},
};

#[rustfmt::skip]
pub use clean::clean_suppressions;

#[rustfmt::skip]
pub use report::report_suppressions;

struct FileResult {
    path: PathBuf,
    checksum: crate::checksum::Checksum,
    analysis: languages::Analysis,
}
type CollectedAnalysis = (
    Vec<Violation>,
    Vec<(String, Fix)>,
    Vec<TreeFix>,
    fix::SourceSnapshots,
);
struct FixPlan {
    fixes: Vec<(String, Fix)>,
    tree_fixes: Vec<TreeFix>,
    snapshots: fix::SourceSnapshots,
}
type AnalysisResult = Result<CollectedAnalysis, AnalysisError>;
type FileAnalysis = Result<Option<FileResult>, String>;
struct AnalysisDispatch<'a> {
    rules: &'a [&'static Rule],
    registered_names: &'a HashSet<&'a str>,
    collect_fixes: bool,
    test_only_files: &'a HashSet<String>,
}
type CollectedResults = (
    Vec<Violation>,
    Vec<(String, Fix)>,
    Vec<TreeFix>,
    fix::SourceSnapshots,
    Vec<languages::workspace::WorkspaceRustFile>,
    Vec<languages::workspace::WorkspaceManifest>,
);

#[derive(Debug, thiserror::Error)]
pub(crate) enum AnalysisError {
    #[error(transparent)]
    Discovery(#[from] walk::PathDiscoveryError),
    #[error("failed to read source files: {0}")]
    Read(Box<str>),
}

const MAX_FIX_ITERATIONS: usize = 10;

/// Resolve the target directory Cargo uses for workspace tooling.
#[must_use]
pub fn cargo_target_dir(root: &Path, workspace_root: &Path) -> PathBuf {
    resolve_cargo_target_dir(
        root,
        workspace_root,
        std::env::var_os("CARGO_TARGET_DIR").map(PathBuf::from),
    )
}

/// One Cargo workspace package and its canonical package root.
#[derive(Clone, Debug)]
pub struct PackageRoot {
    pub name: String,
    pub root: PathBuf,
}

fn package_name_for_path<'a>(packages: &'a [PackageRoot], path: &Path) -> Option<&'a str> {
    packages
        .iter()
        .filter(|package| path.starts_with(&package.root))
        .max_by_key(|package| package.root.components().count())
        .map(|package| package.name.as_str())
}

fn resolve_cargo_target_dir(
    root: &Path,
    workspace_root: &Path,
    configured: Option<PathBuf>,
) -> PathBuf {
    match configured {
        Some(path) if path.is_absolute() => path,
        Some(path) => root.join(path),
        None => workspace_root.join("target"),
    }
}

fn matches_rule_filter(name: &str, filter: &[String]) -> bool {
    filter.is_empty() || filter.iter().any(|r| r == name)
}

/// How the runner should handle auto-fixable violations.
#[derive(Clone, Copy, Debug)]
// #rw(rust_non_exhaustive_on_public) internal enum used only by the rulewright binary
pub enum FixMode {
    Off,
    Apply,
    DryRun,
}

/// Findings output selected by the CLI or an embedding rule pack.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ReportFormat {
    #[default]
    Human,
    Json,
}

/// Inputs needed by `run` and `collect_violations`.
#[derive(Debug)]
pub struct RunCtx<'a> {
    pub registry: &'a RuleRegistry,
    pub rule_filter: &'a [String],
    pub package_filter: &'a [String],
    pub packages: &'a [PackageRoot],
    pub config: &'a Config,
    pub root: &'a Path,
    pub workspace_root: &'a Path,
    pub quiet: bool,
    pub dirty: bool,
}

/// Run Rulewright rules with the selected findings output. Returns `true` if clean.
#[must_use]
pub fn run(
    ctx: &RunCtx<'_>,
    fix: FixMode,
    format: ReportFormat,
    baseline_path: Option<&Path>,
    write_baseline_path: Option<&Path>,
) -> bool {
    let total = enabled_rules(ctx).len();

    if total == 0 {
        if !ctx.quiet {
            output::error("no rules matched (check rulewright.toml and --rule filter)");
        }

        return false;
    }

    if !ctx.quiet && format == ReportFormat::Human {
        output::detail(&format!("running {total} rule(s)"));
        output::blank();
    }

    let collect_fixes = !matches!(fix, FixMode::Off);
    let (mut all_violations, fixes, tree_fixes, snapshots) =
        match collect_violations(ctx, collect_fixes, None) {
            Ok(result) => result,
            Err(error) => {
                if !ctx.quiet {
                    output::error(&format!("rulewright analysis aborted: {error}"));
                }

                return false;
            }
        };

    if let Some(path) = write_baseline_path {
        return match baseline::write(path, &all_violations) {
            Ok(()) => {
                if !ctx.quiet {
                    output::success(&format!(
                        "wrote {} finding(s) to {}",
                        all_violations.len(),
                        path.display()
                    ));
                    output::blank();
                }

                true
            }
            Err(error) => {
                if !ctx.quiet {
                    output::error(&error.to_string());
                }

                false
            }
        };
    }

    if let Some(path) = baseline_path {
        all_violations = match baseline::filter(path, all_violations) {
            Ok(violations) => violations,
            Err(error) => {
                if !ctx.quiet {
                    output::error(&error.to_string());
                }

                return false;
            }
        };
    }

    if collect_fixes && (!fixes.is_empty() || !tree_fixes.is_empty()) {
        return apply_and_verify(
            ctx,
            fix,
            FixPlan {
                fixes,
                tree_fixes,
                snapshots,
            },
            total,
            format,
        );
    }

    if all_violations.is_empty() {
        if format == ReportFormat::Json {
            print_json(&[], ctx.registry);
        } else if !ctx.quiet {
            output::success(&format!("all {total} rule(s) passed"));
            output::blank();
        }

        true
    } else {
        if format == ReportFormat::Json {
            print_json(&all_violations, ctx.registry);
        } else if !ctx.quiet {
            print_grouped(&all_violations, ctx.registry, ctx.packages, ctx.root);
        }

        false
    }
}

fn enabled_rules(ctx: &RunCtx<'_>) -> Vec<&'static Rule> {
    ctx.registry
        .rules()
        .iter()
        .copied()
        .filter(|rule| {
            ctx.config.is_enabled(rule.info.name)
                && matches_rule_filter(rule.info.name, ctx.rule_filter)
        })
        .collect()
}

// #rw(fn: rust_alloc_in_loop, rust_cyclomatic_complexity) fixpoint orchestration branches by edit kind and terminal state
fn apply_and_verify(
    ctx: &RunCtx<'_>,
    fix: FixMode,
    plan: FixPlan,
    total: usize,
    format: ReportFormat,
) -> bool {
    let FixPlan {
        mut fixes,
        mut tree_fixes,
        mut snapshots,
    } = plan;

    if matches!(fix, FixMode::DryRun) {
        if !ctx.quiet {
            let initial_edits = fixes.len() + tree_fixes.len();
            let mut previews = fixes;

            for tree_fix in &tree_fixes {
                previews.extend(tree_fix_previews(ctx.root, tree_fix));
            }

            print_dry_run(&previews);
            output::detail(&format!(
                "{initial_edits} initial edit(s) planned (dry run, no files changed; fixpoint passes may discover more)"
            ));
            output::blank();
        }

        return false;
    }

    let mut applied_total = 0;

    for _ in 0..MAX_FIX_ITERATIONS {
        let applied = if tree_fixes.is_empty() {
            fix::apply_fixes(&fixes, &snapshots, ctx.root)
        } else {
            tree_fixes.sort_by(|a, b| a.rule.cmp(b.rule).then(a.rel.cmp(&b.rel)));
            let rule = tree_fixes[0].rule;
            let selected: Vec<TreeFix> = tree_fixes
                .iter()
                .filter(|tree_fix| tree_fix.rule == rule)
                .cloned()
                .collect();

            fix::apply_tree_fixes(&selected, &snapshots, ctx.root)
        };
        let applied = match applied {
            Ok(applied) => applied,
            Err(error) => {
                if !ctx.quiet {
                    output::error(&format!("failed to apply fixes: {error}"));
                }

                return false;
            }
        };

        applied_total += applied;
        let (remaining, next_fixes, next_tree_fixes, next_snapshots) =
            match collect_violations(ctx, true, None) {
                Ok(result) => result,
                Err(error) => {
                    if !ctx.quiet {
                        output::error(&format!("rulewright analysis aborted after fixes: {error}"));
                    }

                    return false;
                }
            };

        if remaining.is_empty() {
            if format == ReportFormat::Json {
                print_json(&[], ctx.registry);
            } else if !ctx.quiet {
                output::success(&format!("applied {applied_total} fix(es)"));
                output::blank();
                output::success(&format!("all {total} rule(s) passed after fixes"));
                output::blank();
            }

            return true;
        }

        if next_fixes.is_empty() && next_tree_fixes.is_empty() {
            if format == ReportFormat::Json {
                print_json(&remaining, ctx.registry);
            } else if !ctx.quiet {
                output::success(&format!("applied {applied_total} fix(es)"));
                output::blank();
                print_grouped(&remaining, ctx.registry, ctx.packages, ctx.root);
            }

            return false;
        }

        if applied == 0 {
            if format == ReportFormat::Json {
                print_json(&remaining, ctx.registry);
            } else if !ctx.quiet {
                output::error("fix conflict: fixable violations remain but no edit made progress");
                print_grouped(&remaining, ctx.registry, ctx.packages, ctx.root);
            }

            return false;
        }

        fixes = next_fixes;
        tree_fixes = next_tree_fixes;
        snapshots = next_snapshots;
    }

    let remaining = match collect_violations(ctx, false, None) {
        Ok((remaining, _, _, _)) => remaining,
        Err(error) => {
            if !ctx.quiet {
                output::error(&format!("rulewright analysis aborted after fixes: {error}"));
            }

            return false;
        }
    };

    if format == ReportFormat::Json {
        print_json(&remaining, ctx.registry);
    } else if !ctx.quiet {
        output::error("fix conflict: reached the 10-iteration fixpoint cap");
        print_grouped(&remaining, ctx.registry, ctx.packages, ctx.root);
    }

    false
}

fn tree_fix_previews(root: &Path, tree_fix: &TreeFix) -> Vec<(String, Fix)> {
    use imara_diff::{Algorithm, diff, intern::InternedInput};

    let original = file::read_text(&root.join(&tree_fix.rel)).unwrap_or_default();
    let input = InternedInput::new(original.as_str(), tree_fix.replacement.as_str());
    let mut changes = Vec::new();

    diff(Algorithm::Histogram, &input, |before, after| {
        changes.push((before, after));
    });

    changes
        .into_iter()
        .map(|(before, after)| {
            let start_line = before.start as usize + 1;
            let end_line = before.end as usize;
            let inserted_lines = (after.end - after.start) as usize;
            // BOUNDS: imara-diff produces each `after` range from `input.after`.
            let mut replacement = input.after[after.start as usize..after.end as usize]
                .iter()
                .map(|token| {
                    // BOUNDS: every token in `input.after` was allocated by this interner.
                    input.interner[*token]
                })
                .collect::<Vec<_>>()
                .join("\n");

            if replacement.is_empty() && inserted_lines > 0 {
                replacement = "\n".repeat(inserted_lines);
            }

            (
                tree_fix.rel.clone(),
                Fix {
                    start_line,
                    end_line,
                    replacement,
                },
            )
        })
        .collect()
}

/// Walk the target workspace in parallel and collect all violations and optional fixes.
pub(crate) fn collect_violations(
    ctx: &RunCtx<'_>,
    fix_mode: bool,
    paths_override: Option<Vec<PathBuf>>,
) -> AnalysisResult {
    let rules = enabled_rules(ctx);

    if rules.is_empty() {
        return Ok((Vec::new(), Vec::new(), Vec::new(), BTreeMap::default()));
    }

    let mut extensions: Vec<&str> = rules
        .iter()
        .flat_map(|rule| rule.check.extensions().iter().copied())
        .collect();

    extensions.sort_unstable();
    extensions.dedup();

    let (paths, target_rels) = analysis_paths(ctx, paths_override, &extensions)?;
    let test_only_files = languages::rust::test_files::discover(&paths, ctx.root);
    let registered_names: HashSet<&str> = ctx
        .registry
        .rules()
        .iter()
        .copied()
        .map(|rule| rule.info.name)
        .collect();
    let cache = (!fix_mode)
        .then(|| cache::Session::open(ctx, &rules))
        .flatten();

    if let Some(result) =
        complete_cache_result(cache.as_ref(), &rules, target_rels.as_ref(), ctx.dirty)
    {
        return Ok(result);
    }

    let dispatch = AnalysisDispatch {
        rules: &rules,
        registered_names: &registered_names,
        collect_fixes: fix_mode,
        test_only_files: &test_only_files,
    };
    let analyzed = analyze_paths(&paths, cache.as_ref(), ctx, &dispatch);
    let results = require_complete_analysis(analyzed)?;

    persist_file_cache(cache.as_ref(), ctx.root, &results);

    let (
        mut all_violations,
        mut all_fixes,
        mut all_tree_fixes,
        source_snapshots,
        workspace_files,
        workspace_manifests,
    ) = merge_file_results(results);
    let workspace_ctx = WorkspaceCtx {
        files: &workspace_files,
        manifests: &workspace_manifests,
        config: ctx.config,
    };

    all_violations.extend(workspace_violations(cache.as_ref(), &rules, &workspace_ctx));
    all_violations.sort_by(compare_violations);
    persist_workspace_cache(cache.as_ref());
    persist_complete_cache(cache.as_ref(), &all_violations);
    retain_targets(
        &rules,
        target_rels.as_ref(),
        ctx.dirty,
        &mut all_violations,
        &mut all_fixes,
        &mut all_tree_fixes,
    );

    Ok((all_violations, all_fixes, all_tree_fixes, source_snapshots))
}

fn complete_cache_result(
    cache: Option<&cache::Session>,
    rules: &[&'static Rule],
    target_rels: Option<&HashSet<String>>,
    dirty: bool,
) -> Option<CollectedAnalysis> {
    let mut violations = cache?.complete(rules)?;

    if let Some(target_rels) = target_rels {
        let retain_workspace = dirty && !target_rels.is_empty();

        violations.retain(|violation| {
            target_rels.contains(&violation.rel)
                || (retain_workspace && is_workspace_violation(violation, rules))
        });
    }

    violations.sort_by(compare_violations);

    Some((violations, Vec::new(), Vec::new(), BTreeMap::default()))
}

fn analyze_paths(
    paths: &[PathBuf],
    cache: Option<&cache::Session>,
    ctx: &RunCtx<'_>,
    dispatch: &AnalysisDispatch<'_>,
) -> Vec<FileAnalysis> {
    paths
        .par_iter()
        .map(|path| {
            if let Some(result) = cache.and_then(|cache| cache.restore(path, dispatch.rules)) {
                return Ok(Some(result));
            }

            analyze_path(path, ctx, dispatch)
        })
        .collect()
}

fn persist_file_cache(cache: Option<&cache::Session>, root: &Path, results: &[FileResult]) {
    if let Some(cache) = cache {
        cache.persist(root, results);
    }
}

fn persist_complete_cache(cache: Option<&cache::Session>, violations: &[Violation]) {
    if let Some(cache) = cache {
        cache.persist_complete(violations);
    }
}

fn persist_workspace_cache(cache: Option<&cache::Session>) {
    if let Some(cache) = cache {
        cache.persist_workspace();
    }
}

fn retain_targets(
    rules: &[&'static Rule],
    target_rels: Option<&HashSet<String>>,
    dirty: bool,
    violations: &mut Vec<Violation>,
    fixes: &mut Vec<(String, Fix)>,
    tree_fixes: &mut Vec<TreeFix>,
) {
    let Some(target_rels) = target_rels else {
        return;
    };

    let retain_workspace = dirty && !target_rels.is_empty();

    violations.retain(|violation| {
        target_rels.contains(&violation.rel)
            || (retain_workspace && is_workspace_violation(violation, rules))
    });
    fixes.retain(|(rel, _)| target_rels.contains(rel));
    tree_fixes.retain(|tree_fix| target_rels.contains(&tree_fix.rel));
}

fn is_workspace_violation(violation: &Violation, rules: &[&'static Rule]) -> bool {
    rules.iter().any(|rule| {
        rule.info.name == violation.rule_name()
            && matches!(
                rule.check,
                crate::RuleCheck::RustWorkspace(_) | crate::RuleCheck::Workspace(_)
            )
    })
}

fn compare_violations(left: &Violation, right: &Violation) -> std::cmp::Ordering {
    left.rel
        .cmp(&right.rel)
        .then(left.line.cmp(&right.line))
        .then(left.rule_name().cmp(right.rule_name()))
        .then(left.message.cmp(&right.message))
}

fn analysis_paths(
    ctx: &RunCtx<'_>,
    paths_override: Option<Vec<PathBuf>>,
    extensions: &[&str],
) -> Result<(Vec<PathBuf>, Option<HashSet<String>>), AnalysisError> {
    let requested = match paths_override {
        Some(paths) => Some(paths),
        None if ctx.dirty => Some(walk::git_dirty_paths(
            ctx.root,
            ctx.workspace_root,
            ctx.package_filter,
            extensions,
        )?),
        None if !ctx.package_filter.is_empty() => Some(walk::source_paths(
            ctx.workspace_root,
            ctx.package_filter,
            extensions,
        )?),
        None => None,
    };
    let Some(requested) = requested else {
        return Ok((
            walk::source_paths(ctx.workspace_root, &[], extensions)?,
            None,
        ));
    };
    let mut target_rels: HashSet<String> = requested
        .iter()
        .map(|path| relative_path(path, ctx.root))
        .collect::<Result<_, _>>()?;
    let paths = walk::source_paths(ctx.workspace_root, &[], extensions)?;
    let analyzed_rels = paths
        .iter()
        .map(|path| relative_path(path, ctx.root))
        .collect::<Result<HashSet<_>, _>>()?;

    target_rels.retain(|rel| analyzed_rels.contains(rel));

    Ok((paths, Some(target_rels)))
}

fn analyze_path(path: &Path, ctx: &RunCtx<'_>, dispatch: &AnalysisDispatch<'_>) -> FileAnalysis {
    let rel = relative_path(path, ctx.root).map_err(|error| error.to_string())?;
    let contents = file::read_text(path).map_err(|error| format!("{rel}: {error}"))?;
    let lines: Vec<&str> = contents.lines().collect();
    let file = FileCtx {
        rel: &rel,
        path,
        package_name: package_name_for_path(ctx.packages, path),
        lines: &lines,
        contents: &contents,
        config: ctx.config,
    };
    let analysis = match path.extension().and_then(|extension| extension.to_str()) {
        Some("rs") => languages::rust::analyze(
            &file,
            dispatch.rules,
            dispatch.registered_names,
            dispatch.collect_fixes,
            if dispatch.test_only_files.contains(&rel) {
                languages::rust::FileKind::TestOnly
            } else {
                languages::rust::FileKind::Production
            },
        ),
        Some("toml") => languages::toml::analyze(&file, dispatch.rules, dispatch.collect_fixes),
        _ => return Ok(None),
    };

    Ok(Some(FileResult {
        path: path.to_path_buf(),
        checksum: crate::checksum::bytes(&contents),
        analysis,
    }))
}

fn relative_path(path: &Path, root: &Path) -> Result<String, AnalysisError> {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_slash()
        .ok_or_else(|| {
            AnalysisError::Read(
                format!(
                    "{}: non-UTF-8 source paths cannot be analyzed safely",
                    path.display()
                )
                .into_boxed_str(),
            )
        })
}

fn require_complete_analysis(
    analyzed: Vec<FileAnalysis>,
) -> Result<Vec<FileResult>, AnalysisError> {
    let mut failures = Vec::new();
    let mut results = Vec::new();

    for result in analyzed {
        match result {
            Ok(Some(result)) => results.push(result),
            Ok(None) => {}
            Err(error) => failures.push(error),
        }
    }

    if failures.is_empty() {
        Ok(results)
    } else {
        failures.sort_unstable();
        failures.dedup();

        Err(AnalysisError::Read(failures.join("; ").into_boxed_str()))
    }
}

fn merge_file_results(results: Vec<FileResult>) -> CollectedResults {
    let mut all_violations = Vec::new();
    let mut all_fixes = Vec::new();
    let mut all_tree_fixes = Vec::new();
    let mut source_snapshots = fix::SourceSnapshots::new();
    let mut workspace_files = Vec::new();
    let mut workspace_manifests = Vec::new();

    for result in results {
        for rel in result
            .analysis
            .fixes
            .iter()
            .map(|(rel, _)| rel)
            .chain(result.analysis.tree_fixes.iter().map(|fix| &fix.rel))
        {
            source_snapshots.insert(rel.to_owned(), result.checksum);
        }

        all_violations.extend(result.analysis.violations);
        all_fixes.extend(result.analysis.fixes);
        all_tree_fixes.extend(result.analysis.tree_fixes);
        workspace_files.extend(result.analysis.workspace_files);
        workspace_manifests.extend(result.analysis.workspace_manifests);
    }

    (
        all_violations,
        all_fixes,
        all_tree_fixes,
        source_snapshots,
        workspace_files,
        workspace_manifests,
    )
}

fn workspace_violations(
    cache: Option<&cache::Session>,
    rules: &[&'static Rule],
    workspace_ctx: &WorkspaceCtx<'_>,
) -> Vec<Violation> {
    let mut violations = Vec::new();

    for rule in rules {
        let (crate::RuleCheck::RustWorkspace(check) | crate::RuleCheck::Workspace(check)) =
            rule.check
        else {
            continue;
        };
        let patterns = workspace_ctx.config.ignore_patterns(rule.info.name);
        let files: Vec<_> = workspace_ctx
            .files
            .iter()
            .filter(|file| !crate::matches_ignore(&file.rel, patterns))
            .cloned()
            .collect();
        let manifests: Vec<_> = workspace_ctx
            .manifests
            .iter()
            .filter(|manifest| !crate::matches_ignore(&manifest.rel, patterns))
            .cloned()
            .collect();
        let rule_ctx = WorkspaceCtx {
            files: &files,
            manifests: &manifests,
            config: workspace_ctx.config,
        };
        let compute = || {
            check(&rule_ctx)
                .into_iter()
                .map(|violation| violation.with_rule(rule.info.name))
                .filter(|violation| {
                    rule_ctx
                        .files
                        .iter()
                        .find(|file| file.rel == violation.rel)
                        .is_none_or(|file| {
                            !crate::infra::ignore::is_file_suppressed(
                                &file.suppressions,
                                rule.info.name,
                            ) && !file
                                .suppressions
                                .entries
                                .iter()
                                .any(|entry| entry.suppresses(violation))
                        })
                })
                .collect()
        };
        let rule_violations = if let Some(cache) = cache {
            cache.workspace_violations(rule, &rule_ctx, compute)
        } else {
            compute()
        };

        violations.extend(rule_violations);
    }

    violations
}

#[cfg(test)]
mod tests {
    use googletest::prelude::*;

    use super::*;
    use crate::infra::ignore::Suppressions;

    fn input_count_rule(ctx: &WorkspaceCtx<'_>) -> Vec<Violation> {
        (ctx.files.len() > 1 || ctx.manifests.len() > 1)
            .then(|| crate::violation("kept.rs", 1, "ignored input leaked into workspace rule"))
            .into_iter()
            .collect()
    }

    static INPUT_COUNT_RULE: Rule = Rule::workspace(
        crate::RuleInfo::new(
            "fixture_workspace_input_count",
            "Check the filtered input count.",
            "The fixture detects ignored records reaching a workspace rule.",
            crate::Severity::Low,
            &[],
            &[],
        ),
        input_count_rule,
    );

    #[gtest]
    fn violation_order_has_deterministic_tie_breakers() -> Result<()> {
        let mut violations = [
            crate::violation("same.rs", 1, "second").with_rule("z_rule"),
            crate::violation("same.rs", 1, "second").with_rule("a_rule"),
            crate::violation("same.rs", 1, "first").with_rule("a_rule"),
            crate::violation("later.rs", 1, "only").with_rule("a_rule"),
        ];

        violations.sort_by(compare_violations);
        let ordered: Vec<_> = violations
            .iter()
            .map(|violation| {
                (
                    violation.rel.as_str(),
                    violation.line,
                    violation.rule_name(),
                    violation.message.as_str(),
                )
            })
            .collect();

        verify_eq!(
            ordered,
            [
                ("later.rs", 1, "a_rule", "only"),
                ("same.rs", 1, "a_rule", "first"),
                ("same.rs", 1, "a_rule", "second"),
                ("same.rs", 1, "z_rule", "second"),
            ]
        )
    }

    #[gtest]
    fn tree_fix_preview_reports_only_the_changed_hunks() -> Result<()> {
        let directory = crate::temporary::Directory::new().or_fail()?;
        let source = directory.path().join("src/lib.rs");

        crate::directory::ensure(source.parent().or_fail()?).or_fail()?;
        file::write_text(&source, "first\nsecond\nthird\nfourth\n").or_fail()?;
        let previews = tree_fix_previews(
            directory.path(),
            &TreeFix {
                rel: "src/lib.rs".to_owned(),
                rule: "rust_fixture",
                replacement: "first\nchanged\nthird\nchanged again\n".to_owned(),
            },
        );
        let spans: Vec<_> = previews
            .iter()
            .map(|(_, preview)| {
                (
                    preview.start_line,
                    preview.end_line,
                    preview.replacement.as_str(),
                )
            })
            .collect();

        verify_eq!(spans, [(2, 2, "changed"), (4, 4, "changed again")])
    }

    #[gtest]
    fn tree_fix_preview_counts_inserted_blank_lines() -> Result<()> {
        let directory = crate::temporary::Directory::new().or_fail()?;
        let source = directory.path().join("src/lib.rs");

        crate::directory::ensure(source.parent().or_fail()?).or_fail()?;
        file::write_text(&source, "first\nsecond\n").or_fail()?;
        let previews = tree_fix_previews(
            directory.path(),
            &TreeFix {
                rel: "src/lib.rs".to_owned(),
                rule: "rust_fixture",
                replacement: "first\n\nsecond\n".to_owned(),
            },
        );
        let (_, preview) = previews.first().or_fail()?;

        verify_eq!(preview.start_line, 2)?;
        verify_eq!(preview.end_line, 1)?;

        verify_eq!(preview.replacement.lines().count(), 1)
    }

    #[gtest]
    fn cargo_target_dir_honors_absolute_relative_and_default_paths() -> Result<()> {
        let directory = crate::temporary::Directory::new().or_fail()?;
        let root = directory.path();
        let workspace_root = root.join("crates");
        let absolute = root.join("persistent-target");

        verify_eq!(
            resolve_cargo_target_dir(root, &workspace_root, Some(absolute.clone())),
            absolute
        )?;
        verify_eq!(
            resolve_cargo_target_dir(root, &workspace_root, Some(PathBuf::from("shared-target"))),
            root.join("shared-target")
        )?;

        verify_eq!(
            resolve_cargo_target_dir(root, &workspace_root, None),
            workspace_root.join("target")
        )
    }

    #[gtest]
    fn workspace_rule_ignores_filter_files_and_manifests_before_dispatch() -> Result<()> {
        let mut config = Config::generate_default(&[(INPUT_COUNT_RULE.info.name, &[])]);

        config
            .rules
            .get_mut(INPUT_COUNT_RULE.info.name)
            .or_fail()?
            .ignore
            .push("ignored/**".to_owned());
        let files = [
            languages::workspace::WorkspaceRustFile {
                rel: "kept.rs".to_owned(),
                structs: Vec::new(),
                functions: Vec::new(),
                strings: Vec::new(),
                crate_roots: HashSet::new(),
                suppressions: Suppressions::default(),
            },
            languages::workspace::WorkspaceRustFile {
                rel: "ignored/source.rs".to_owned(),
                structs: Vec::new(),
                functions: Vec::new(),
                strings: Vec::new(),
                crate_roots: HashSet::new(),
                suppressions: Suppressions::default(),
            },
        ];
        let manifests = [
            crate::WorkspaceManifest {
                rel: "Cargo.toml".to_owned(),
                dependencies: Vec::new(),
            },
            crate::WorkspaceManifest {
                rel: "ignored/Cargo.toml".to_owned(),
                dependencies: Vec::new(),
            },
        ];
        let ctx = WorkspaceCtx {
            files: &files,
            manifests: &manifests,
            config: &config,
        };

        verify_true!(workspace_violations(None, &[&INPUT_COUNT_RULE], &ctx).is_empty())
    }

    #[gtest]
    fn source_read_failures_are_fatal_and_deterministic() -> Result<()> {
        let directory = crate::temporary::Directory::new().or_fail()?;
        let workspace_root = directory.path().join("crates");

        crate::directory::create(&workspace_root).or_fail()?;
        let first = workspace_root.join("a.rs");
        let second = workspace_root.join("b.rs");

        file::write_bytes(&first, [0xff]).or_fail()?;
        file::write_bytes(&second, [0xff]).or_fail()?;

        let metadata = crate::all_rules();
        let registered: Vec<_> = metadata
            .iter()
            .map(|rule| (rule.name, rule.params))
            .collect();
        let config = Config::generate_default(&registered);
        let registry = RuleRegistry::with_builtins().or_fail()?;
        let ctx = RunCtx {
            registry: &registry,
            rule_filter: &[],
            package_filter: &[],
            packages: &[],
            config: &config,
            root: directory.path(),
            workspace_root: &workspace_root,
            quiet: true,
            dirty: false,
        };
        let error = collect_violations(&ctx, false, Some(vec![second, first]))
            .unwrap_err()
            .to_string();

        verify_that!(error.as_str(), contains_substring("crates/a.rs"))?;
        verify_that!(error.as_str(), contains_substring("crates/b.rs"))?;

        verify_true!(error.find("crates/a.rs").or_fail()? < error.find("crates/b.rs").or_fail()?)
    }

    #[cfg(unix)]
    #[gtest]
    fn non_utf8_source_path_fails_instead_of_using_a_lossy_identity() -> Result<()> {
        use std::os::unix::ffi::OsStringExt as _;

        let directory = crate::temporary::Directory::new().or_fail()?;
        let name = std::ffi::OsString::from_vec(b"source-\xff.rs".to_vec());
        let source = directory.path().join(std::path::PathBuf::from(name));

        let registry = RuleRegistry::with_builtins().or_fail()?;
        let metadata = registry.metadata();
        let config = Config::generate_default(
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
        let error = collect_violations(&ctx, false, Some(vec![source]))
            .unwrap_err()
            .to_string();

        verify_that!(error, contains_substring("non-UTF-8 source paths"))
    }

    #[gtest]
    fn restricted_analysis_retains_complete_workspace_context() -> Result<()> {
        let directory = crate::temporary::Directory::new().or_fail()?;
        let workspace_root = directory.path().join("crates");

        crate::directory::create(&workspace_root).or_fail()?;
        let requested = workspace_root.join("changed.rs");
        let context_only = workspace_root.join("unchanged.rs");

        file::write_text(&requested, "fn changed() {}\n").or_fail()?;
        file::write_text(&context_only, "fn unchanged() {}\n").or_fail()?;

        let metadata = crate::all_rules();
        let registered: Vec<_> = metadata
            .iter()
            .map(|rule| (rule.name, rule.params))
            .collect();
        let config = Config::generate_default(&registered);
        let registry = RuleRegistry::with_builtins().or_fail()?;
        let ctx = RunCtx {
            registry: &registry,
            rule_filter: &[],
            package_filter: &[],
            packages: &[],
            config: &config,
            root: directory.path(),
            workspace_root: &workspace_root,
            quiet: true,
            dirty: false,
        };
        let (paths, target_rels) = analysis_paths(&ctx, Some(vec![requested]), &["rs"])?;

        verify_that!(paths, len(eq(2)))?;

        verify_eq!(
            target_rels,
            Some(HashSet::from(["crates/changed.rs".to_string()]))
        )
    }

    #[gtest]
    fn restricted_target_is_empty_when_requested_source_is_not_analyzed() -> Result<()> {
        let directory = crate::temporary::Directory::new().or_fail()?;
        let requested = directory.path().join("ignored.rs");

        file::write_text(&requested, "fn ignored() {}\n").or_fail()?;
        file::write_text(&directory.path().join(".rulewrightignore"), "ignored.rs\n").or_fail()?;
        let registry = RuleRegistry::with_builtins().or_fail()?;
        let metadata = registry.metadata();
        let config = Config::generate_default(
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
        let (paths, target_rels) = analysis_paths(&ctx, Some(vec![requested]), &["rs"])?;

        verify_true!(paths.is_empty())?;

        verify_eq!(target_rels, Some(HashSet::new()))
    }

    #[gtest]
    fn cfg_test_path_modules_are_classified_before_rule_dispatch() -> Result<()> {
        let directory = crate::temporary::Directory::new().or_fail()?;
        let source_dir = directory.path().join("src");

        crate::directory::ensure(&source_dir).or_fail()?;
        file::write_text(
            &source_dir.join("lib.rs"),
            "#[cfg(test)]\n#[path = \"validation.rs\"]\nmod tests;\n",
        )
        .or_fail()?;
        file::write_text(
            &source_dir.join("validation.rs"),
            "fn fixture() -> usize { 42 }\n",
        )
        .or_fail()?;
        let registry = RuleRegistry::with_builtins().or_fail()?;
        let metadata = registry.metadata();
        let config = Config::generate_default(
            &metadata
                .iter()
                .map(|rule| (rule.name, rule.params))
                .collect::<Vec<_>>(),
        );
        let rule_filter = vec!["rust_magic_numbers".to_owned()];
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
        let (violations, _, _, _) = collect_violations(&ctx, false, None).or_fail()?;

        verify_true!(violations.is_empty())
    }

    #[gtest]
    fn package_filtered_analysis_retains_complete_workspace_context() -> Result<()> {
        let directory = crate::temporary::Directory::new().or_fail()?;
        let workspace_root = directory.path().join("crates");
        let selected_dir = workspace_root.join("selected");
        let context_dir = workspace_root.join("context");

        crate::directory::ensure(&selected_dir).or_fail()?;
        crate::directory::ensure(&context_dir).or_fail()?;
        file::write_text(&selected_dir.join("selected.rs"), "fn selected() {}\n").or_fail()?;
        file::write_text(&context_dir.join("context.rs"), "fn context() {}\n").or_fail()?;

        let metadata = crate::all_rules();
        let registered: Vec<_> = metadata
            .iter()
            .map(|rule| (rule.name, rule.params))
            .collect();
        let config = Config::generate_default(&registered);
        let package_filter = vec!["selected".to_string()];
        let registry = RuleRegistry::with_builtins().or_fail()?;
        let ctx = RunCtx {
            registry: &registry,
            rule_filter: &[],
            package_filter: &package_filter,
            packages: &[],
            config: &config,
            root: directory.path(),
            workspace_root: &workspace_root,
            quiet: true,
            dirty: false,
        };
        let (paths, target_rels) = analysis_paths(&ctx, None, &["rs"])?;

        verify_that!(paths, len(eq(2)))?;

        verify_eq!(
            target_rels,
            Some(HashSet::from(["crates/selected/selected.rs".to_string()]))
        )
    }

    #[gtest]
    fn fixpoint_rechecks_unfixed_files_in_the_selected_scope() -> Result<()> {
        let directory = crate::temporary::Directory::new().or_fail()?;
        let workspace_root = directory.path().join("crates");
        let crate_dir = workspace_root.join("demo");

        crate::directory::ensure(&crate_dir).or_fail()?;
        let fixable = crate_dir.join("fixable.rs");

        file::write_text(
            &fixable,
            "pub fn value() {\n    let value = 1;\n    std::hint::black_box(value);\n}\n",
        )
        .or_fail()?;
        file::write_text(
            &crate_dir.join("unfixable.rs"),
            "pub fn explode() {\n    panic!();\n}\n",
        )
        .or_fail()?;

        let metadata = crate::all_rules();
        let registered: Vec<_> = metadata
            .iter()
            .map(|rule| (rule.name, rule.params))
            .collect();
        let config = Config::generate_default(&registered);
        let rule_filter = vec!["rust_padding".to_string(), "rust_panic".to_string()];
        let package_filter = vec!["demo".to_string()];
        let registry = RuleRegistry::with_builtins().or_fail()?;
        let ctx = RunCtx {
            registry: &registry,
            rule_filter: &rule_filter,
            package_filter: &package_filter,
            packages: &[],
            config: &config,
            root: directory.path(),
            workspace_root: &workspace_root,
            quiet: true,
            dirty: false,
        };

        verify_false!(run(&ctx, FixMode::Apply, ReportFormat::Human, None, None,))?;

        let fixed = file::read_text(&fixable).or_fail()?;

        verify_that!(
            fixed,
            contains_substring("let value = 1;\n\n    std::hint::black_box(value);")
        )
    }
}
