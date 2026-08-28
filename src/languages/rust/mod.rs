// #rw(file: rust_default_hasher) trusted rule-name keys in a cold dispatch set; fast-hasher dependency not warranted
// #rw(file: rust_inline_test_module_size) adapter tests require private dispatch and syntax helpers

//! Rust source parsing and rule execution.

#[cfg(test)]
use googletest::prelude::*;

#[cfg(test)]
mod dispatch_tests;
mod rules;
pub(crate) mod test_files;

use std::collections::HashSet;

use line_index::LineIndex;
use ra_ap_syntax::{
    AstNode, Edition, SourceFile, SyntaxNode,
    ast::{self, HasAttrs, HasName},
};

use crate::{
    FileCtx, Rule, RuleCheck, RuleFix, Violation,
    infra::{ignore, scanner},
    languages::Analysis,
    matches_ignore,
};

const DIRECTIVE_RULE: &str = "rust_rulewright_directives";

#[derive(Debug)]
pub(crate) struct SuppressionAudit {
    pub suppressions: ignore::Suppressions,
    pub raw_violations: Vec<Violation>,
    pub workspace_file: crate::languages::workspace::WorkspaceRustFile,
}

#[derive(Debug)]
pub(crate) enum SuppressionAuditError {
    InvalidDirectives(Vec<Violation>),
    RustSyntax,
}

pub struct AstCtx<'a> {
    pub file: &'a FileCtx<'a>,
    pub root: &'a SourceFile,
    pub line_index: &'a LineIndex,
    nodes: Vec<SyntaxNode>,
    pub(crate) test_only_file: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum FileKind {
    Production,
    TestOnly,
}

impl FileKind {
    const fn is_test_only(self) -> bool {
        matches!(self, Self::TestOnly)
    }
}

pub trait RustLocation {
    fn line_in(&self, ctx: &AstCtx<'_>) -> usize;

    fn column_in(&self, _ctx: &AstCtx<'_>) -> Option<usize> {
        None
    }
}

impl<N> RustLocation for &N
where
    N: AstNode,
{
    fn line_in(&self, ctx: &AstCtx<'_>) -> usize {
        ctx.line_index
            .line_col(self.syntax().text_range().start())
            .line as usize
            + 1
    }

    fn column_in(&self, ctx: &AstCtx<'_>) -> Option<usize> {
        Some(
            ctx.line_index
                .line_col(self.syntax().text_range().start())
                .col as usize
                + 1,
        )
    }
}

impl RustLocation for SyntaxNode {
    fn line_in(&self, ctx: &AstCtx<'_>) -> usize {
        ctx.line_index.line_col(self.text_range().start()).line as usize + 1
    }

    fn column_in(&self, ctx: &AstCtx<'_>) -> Option<usize> {
        Some(ctx.line_index.line_col(self.text_range().start()).col as usize + 1)
    }
}

impl AstCtx<'_> {
    pub(crate) fn new<'a>(
        file: &'a FileCtx<'a>,
        root: &'a SourceFile,
        line_index: &'a LineIndex,
        test_only_file: bool,
    ) -> AstCtx<'a> {
        AstCtx {
            file,
            root,
            line_index,
            nodes: root.syntax().descendants().collect(),
            test_only_file,
        }
    }

    pub fn nodes<'a, N>(&'a self) -> impl Iterator<Item = N> + 'a
    where
        N: AstNode + 'a,
    {
        self.nodes.iter().cloned().filter_map(N::cast)
    }

    pub fn is_in_test<N>(&self, node: &N) -> bool
    where
        N: AstNode,
    {
        self.test_only_file || syntax_is_in_test(node.syntax())
    }

    #[expect(
        clippy::needless_pass_by_value,
        reason = "RustLocation is implemented for AST references, so the generic value is already a cheap reference"
    )]
    pub fn line_of(&self, location: impl RustLocation) -> usize {
        location.line_in(self)
    }

    #[expect(
        clippy::needless_pass_by_value,
        reason = "RustLocation is implemented for AST references, so the generic value is already a cheap reference"
    )]
    pub fn violation(&self, location: impl RustLocation, msg: impl Into<String>) -> Violation {
        let line = location.line_in(self);
        let column = location.column_in(self);
        let violation = crate::violation(self.file.rel, line, msg);

        if let Some(column) = column {
            violation.with_column(column)
        } else {
            violation
        }
    }
}

impl std::fmt::Debug for AstCtx<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AstCtx")
            .field("file", &self.file)
            .field("root", &"<ra_ap_syntax::SourceFile>")
            .field("line_index", &self.line_index)
            .field("node_count", &self.nodes.len())
            .field("test_only_file", &self.test_only_file)
            .finish()
    }
}

// #rw(fn: rust_cyclomatic_complexity) the adapter coordinates parse, suppression, line, AST, workspace, and fix paths
pub(crate) fn analyze(
    file: &FileCtx<'_>,
    rules: &[&Rule],
    registered_names: &HashSet<&str>,
    fix_mode: bool,
    file_kind: FileKind,
) -> Analysis {
    let mut analysis = Analysis::default();
    let mut ignore_errors = Vec::new();
    let ra_parse = SourceFile::parse(file.contents, Edition::Edition2024);
    let ra_root = ra_parse.errors().is_empty().then(|| ra_parse.tree());
    let test_only_file = file_kind.is_test_only();
    let visible_lines = if test_only_file {
        vec![""; file.lines.len()]
    } else {
        ra_root.as_ref().map_or_else(
            || file.lines.to_vec(),
            |syntax| line_rule_view(file.lines, syntax, file.contents, file.rel),
        )
    };
    let line_file = FileCtx {
        rel: file.rel,
        path: file.path,
        package_name: file.package_name,
        package_publishable: file.package_publishable,
        lines: &visible_lines,
        contents: file.contents,
        config: file.config,
    };
    let directive_lines = scanner::directive_source_lines(file.contents, file.lines);
    let mut suppressed = ignore::suppressed_lines(
        line_file.rel,
        &directive_lines,
        &mut ignore_errors,
        Some(registered_names),
    );
    let report_directive_errors = !file.config.allows_suppressions()
        || rules.iter().any(|rule| rule.info.name == DIRECTIVE_RULE)
            && !matches_ignore(file.rel, file.config.ignore_patterns(DIRECTIVE_RULE));

    if report_directive_errors {
        analysis.violations.extend(
            ignore_errors
                .into_iter()
                .map(|violation| violation.with_rule(DIRECTIVE_RULE)),
        );
    }

    if !file.config.allows_suppressions() {
        analysis
            .violations
            .extend(suppressed.entries.iter().map(|entry| {
                crate::violation(
                    file.rel,
                    entry.line,
                    "source suppression directives are disabled by rulewright.toml; fix the code or configure the shared rule scope",
                )
                .with_rule(DIRECTIVE_RULE)
            }));
        suppressed = ignore::Suppressions::default();
    }

    let workspace_suppressions = suppressed.clone();

    for rule in rules {
        let (check, rule_file) = match rule.check {
            RuleCheck::RustLine(check) => (check, &line_file),
            RuleCheck::RustLineFull(check) => (check, file),
            _ => continue,
        };

        if is_suppressed(rule_file, rule, &suppressed) {
            continue;
        }

        let mut violations = ignore::filter(&suppressed, tagged(rule, check(rule_file)));

        if let Some(RuleFix::RustLine(fix)) = rule.fix {
            collect_fixes(
                &mut analysis,
                file.rel,
                &mut violations,
                fix_mode,
                |violation| fix(rule_file, violation),
            );
        }

        analysis.violations.extend(violations);
    }

    if let Some(root) = ra_root {
        let line_index = LineIndex::new(file.contents);
        let ctx = AstCtx::new(file, &root, &line_index, test_only_file);

        for rule in rules {
            let RuleCheck::RustAst(check) = rule.check else {
                continue;
            };

            if is_suppressed(file, rule, &suppressed) {
                continue;
            }

            let mut violations = ignore::filter(&suppressed, tagged(rule, check(&ctx)));

            match rule.fix {
                Some(RuleFix::RustAst(fix)) => {
                    collect_fixes(
                        &mut analysis,
                        file.rel,
                        &mut violations,
                        fix_mode,
                        |violation| fix(&ctx, violation),
                    );
                }

                Some(RuleFix::RustAstTree(fix)) => {
                    mark_tree_fixability(&mut violations, |finding| fix(&ctx, finding));

                    if fix_mode
                        && !violations.is_empty()
                        && let Some(replacement) = fix(&ctx, &violations)
                    {
                        analysis.tree_fixes.push(crate::infra::fix::TreeFix {
                            rel: file.rel.to_owned(),
                            rule: rule.info.name,
                            replacement,
                        });
                    }
                }

                _ => {}
            }

            analysis.violations.extend(violations);
        }

        if rules.iter().any(|rule| {
            matches!(
                rule.check,
                RuleCheck::RustWorkspace(_) | RuleCheck::Workspace(_)
            )
        }) {
            analysis
                .workspace_files
                .push(crate::languages::workspace::extract(
                    file,
                    &root,
                    workspace_suppressions,
                    test_only_file,
                ));
        }
    }

    analysis
}

/// Analyze directive usage without applying source suppressions or config path ignores.
pub(crate) fn audit_suppressions(
    file: &FileCtx<'_>,
    rules: &[&Rule],
    registered_names: &HashSet<&str>,
) -> Result<SuppressionAudit, SuppressionAuditError> {
    let ra_parse = SourceFile::parse(file.contents, Edition::Edition2024);

    if !ra_parse.errors().is_empty() {
        return Err(SuppressionAuditError::RustSyntax);
    }

    let root = ra_parse.tree();
    let visible_lines = line_rule_view(file.lines, &root, file.contents, file.rel);
    let directive_lines = scanner::directive_source_lines(file.contents, file.lines);
    let audit_file = FileCtx {
        rel: file.rel,
        path: file.path,
        package_name: file.package_name,
        package_publishable: file.package_publishable,
        lines: &visible_lines,
        contents: file.contents,
        config: file.config,
    };
    let mut directive_errors = Vec::new();
    let suppressions = ignore::suppressed_lines(
        file.rel,
        &directive_lines,
        &mut directive_errors,
        Some(registered_names),
    );

    if !directive_errors.is_empty() {
        return Err(SuppressionAuditError::InvalidDirectives(directive_errors));
    }

    let line_index = LineIndex::new(file.contents);
    let ast = AstCtx::new(&audit_file, &root, &line_index, false);
    let mut raw_violations = Vec::new();

    for rule in rules {
        let violations = match rule.check {
            RuleCheck::RustLine(check) => check(&audit_file),
            RuleCheck::RustLineFull(check) => check(file),
            RuleCheck::RustAst(check) => check(&ast),
            RuleCheck::RustWorkspace(_) | RuleCheck::Workspace(_) | RuleCheck::Toml(_) => continue,
        };

        raw_violations.extend(tagged(rule, violations));
    }

    raw_violations.sort_by(|a, b| a.line.cmp(&b.line).then(a.rule.cmp(&b.rule)));
    let workspace_file =
        crate::languages::workspace::extract(file, &root, suppressions.clone(), false);

    Ok(SuppressionAudit {
        suppressions,
        raw_violations,
        workspace_file,
    })
}

/// Evaluate raw Rust workspace-rule violations once without applying source suppressions.
pub(crate) fn audit_workspace_suppressions(
    files: &[crate::languages::workspace::WorkspaceRustFile],
    rules: &[&Rule],
    config: &crate::Config,
) -> Vec<Violation> {
    let ctx = crate::languages::workspace::WorkspaceCtx {
        files,
        manifests: &[],
        config,
    };
    let mut violations = Vec::new();

    for rule in rules {
        let RuleCheck::RustWorkspace(check) = rule.check else {
            continue;
        };

        violations.extend(
            check(&ctx)
                .into_iter()
                .map(|violation| violation.with_rule(rule.info.name)),
        );
    }

    violations.sort_by(|a, b| {
        a.rel
            .cmp(&b.rel)
            .then(a.line.cmp(&b.line))
            .then(a.rule.cmp(&b.rule))
    });

    violations
}

fn line_rule_view<'a>(
    lines: &[&'a str],
    syntax: &SourceFile,
    contents: &str,
    rel: &str,
) -> Vec<&'a str> {
    let mut excluded = vec![false; lines.len()];
    let line_index = LineIndex::new(contents);
    let rule_implementation = rel.starts_with("src/languages/") && rel.contains("/rules/");

    for item in syntax.syntax().descendants().filter_map(ast::Item::cast) {
        if is_line_rule_metadata(&item, rule_implementation)
            || item.attrs().any(|attr| is_test_attribute(&attr))
        {
            exclude_item(&mut excluded, &line_index, item.syntax());
        }
    }

    lines
        .iter()
        .zip(excluded)
        .map(|(&line, excluded)| if excluded { "" } else { line })
        .collect()
}

fn exclude_item(excluded: &mut [bool], line_index: &LineIndex, node: &SyntaxNode) {
    let start = line_index.line_col(node.text_range().start()).line as usize;
    let end = line_index.line_col(node.text_range().end()).line as usize + 1;
    let range_start = start.min(excluded.len());
    let range_end = end.min(excluded.len());

    if let Some(region) = excluded.get_mut(range_start..range_end) {
        region.fill(true);
    }
}

fn is_line_rule_metadata(item: &ast::Item, rule_implementation: bool) -> bool {
    if let ast::Item::Const(item_const) = item
        && item_const.name().is_some_and(|name| {
            let text = name.text();

            text == "EXAMPLES" || rule_implementation && text.contains("PATTERN")
        })
    {
        return true;
    }

    let ast::Item::MacroCall(item_macro) = item else {
        return false;
    };

    let macro_token = item_macro
        .path()
        .and_then(|path| path.segment())
        .and_then(|segment| segment.syntax().last_token());

    macro_token.is_some_and(|token| {
        matches!(
            token.text(),
            "line_rule"
                | "full_line_rule"
                | "ast_rule"
                | "toml_rule"
                | "rulewright_test"
                | "rulewright_ast_test"
                | "rulewright_toml_test"
        )
    })
}

fn is_test_attribute(attr: &ast::Attr) -> bool {
    if attr.simple_name().as_deref() == Some("test") {
        return true;
    }

    let Some(ast::Meta::CfgMeta(meta)) = attr.meta() else {
        return false;
    };

    meta.cfg_predicate()
        .is_some_and(|predicate| cfg_requires_test(&predicate))
}

fn syntax_is_in_test(node: &SyntaxNode) -> bool {
    node.ancestors()
        .filter_map(ast::Item::cast)
        .any(|item| item.attrs().any(|attr| is_test_attribute(&attr)))
}

fn cfg_requires_test(predicate: &ast::CfgPredicate) -> bool {
    match predicate {
        ast::CfgPredicate::CfgAtom(atom) => atom
            .ident_token()
            .is_some_and(|identifier| identifier.text() == "test"),

        ast::CfgPredicate::CfgComposite(composite) => {
            let operator = composite.keyword().map(|keyword| keyword.text().to_owned());
            let predicates: Vec<ast::CfgPredicate> = composite.cfg_predicates().collect();

            match operator.as_deref() {
                Some("all") => predicates.iter().any(cfg_requires_test),
                Some("any") => !predicates.is_empty() && predicates.iter().all(cfg_requires_test),
                _ => false,
            }
        }
    }
}

fn is_suppressed(file: &FileCtx<'_>, rule: &Rule, suppressed: &ignore::Suppressions) -> bool {
    matches_ignore(file.rel, file.config.ignore_patterns(rule.info.name))
        || ignore::is_file_suppressed(suppressed, rule.info.name)
}

fn tagged(rule: &Rule, violations: Vec<Violation>) -> Vec<Violation> {
    violations
        .into_iter()
        .map(|violation| violation.with_rule(rule.info.name))
        .collect()
}

fn collect_fixes(
    analysis: &mut Analysis,
    rel: &str,
    violations: &mut [Violation],
    retain_fixes: bool,
    fix: impl Fn(&Violation) -> Option<crate::Fix>,
) {
    for violation in violations {
        let Some(edit) = fix(violation) else {
            continue;
        };

        violation.mark_fixable();

        if retain_fixes {
            analysis.fixes.push((rel.to_owned(), edit));
        }
    }
}

fn mark_tree_fixability(
    violations: &mut [Violation],
    fix: impl Fn(&[Violation]) -> Option<String>,
) {
    for violation in violations {
        let fixable = fix(std::slice::from_ref(&*violation)).is_some();

        if fixable {
            violation.mark_fixable();
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::path::Path;

    use super::*;
    use crate::infra::config::Config;

    fn analyze_all(source: &str) -> Analysis {
        let metadata = crate::all_rules();
        let registered_meta: Vec<_> = metadata
            .iter()
            .map(|rule| (rule.name, rule.params))
            .collect();
        let config = Config::generate_default(&registered_meta);
        let lines: Vec<&str> = source.lines().collect();
        let file = FileCtx {
            rel: "adapter.rs",
            path: Path::new("adapter.rs"),
            package_name: None,
            package_publishable: None,
            lines: &lines,
            contents: source,
            config: &config,
        };
        let rules: Vec<&Rule> = inventory::iter::<Rule>
            .into_iter()
            .filter(|rule| config.is_enabled(rule.info.name))
            .collect();
        let registered = rules.iter().map(|rule| rule.info.name).collect();

        analyze(&file, &rules, &registered, false, FileKind::Production)
    }

    #[gtest]
    fn fixability_is_recorded_per_finding_without_retaining_edits() -> Result<()> {
        let mut analysis = Analysis::default();
        let mut violations = vec![
            crate::violation("fixture.rs", 1, "fixable"),
            crate::violation("fixture.rs", 2, "manual"),
        ];

        collect_fixes(
            &mut analysis,
            "fixture.rs",
            &mut violations,
            false,
            |violation| {
                (violation.line == 1).then(|| crate::Fix {
                    start_line: 1,
                    end_line: 1,
                    replacement: "fixed".to_owned(),
                })
            },
        );

        verify_true!(violations[0].is_fixable())?;
        verify_false!(violations[1].is_fixable())?;
        verify_true!(analysis.fixes.is_empty())
    }

    #[gtest]
    fn coordinated_fixability_is_evaluated_one_finding_at_a_time() -> Result<()> {
        let mut violations = vec![
            crate::violation("fixture.rs", 1, "fixable"),
            crate::violation("fixture.rs", 2, "manual"),
        ];

        mark_tree_fixability(&mut violations, |findings| {
            (findings.len() == 1 && findings[0].line == 1).then(|| "fixed".to_owned())
        });

        verify_true!(violations[0].is_fixable())?;
        verify_false!(violations[1].is_fixable())
    }

    #[gtest]
    fn clean_fixture_has_no_violations() -> Result<()> {
        let analysis = analyze_all(include_str!("../../../tests/fixtures/clean.rs"));

        verify_true!(analysis.violations.is_empty())?;

        Ok(())
    }

    #[gtest]
    fn dirty_source_reports_registered_rules() -> Result<()> {
        let analysis =
            analyze_all("fn f() { dbg!(1); let first = 42; let second = 42; let third = 42; }");
        let rules: Vec<_> = analysis
            .violations
            .iter()
            .filter_map(|violation| violation.rule)
            .collect();

        verify_true!(rules.contains(&"rust_dbg"))?;
        verify_true!(rules.contains(&"rust_magic_numbers"))?;

        Ok(())
    }

    #[gtest]
    fn adapter_applies_line_suppressions_after_tagging() -> Result<()> {
        let analysis =
            analyze_all("fn f() {\n    // #rw(rust_dbg) intentional diagnostic\n    dbg!(1);\n}\n");

        verify_true!(
            analysis
                .violations
                .iter()
                .all(|violation| violation.rule != Some("rust_dbg"))
        )?;

        Ok(())
    }

    #[gtest]
    fn directive_lookalikes_do_not_suppress_ast_rules() -> Result<()> {
        let source = concat!(
            "const RAW: &str = r#\"\n",
            "// #rw(file: rust_panic) raw string lookalike\n",
            "\"#;\n",
            "/*\n",
            "// #rw(file: rust_panic) block comment lookalike\n",
            "*/\n",
            "fn production() { panic!(); }\n",
        );
        let analysis = analyze_all(source);

        verify_true!(
            analysis
                .violations
                .iter()
                .any(|violation| violation.rule == Some("rust_panic"))
        )?;

        Ok(())
    }

    #[gtest]
    fn rules_ignore_examples_and_test_literals() -> Result<()> {
        let source = r#"
use crate::Example;

const EXAMPLES: &[Example] = &[
    Example { label: "path", code: "const P: &str = \"/home/dev/file\";", pass: false },
    Example { label: "box", code: "type T = Box<Vec<u8>>;", pass: false },
    Example { label: "directive", code: "// #rw(rust_unknown) fixture", pass: false },
];

#[cfg(test)]
mod tests {
    const URL: &str = "https://example.com";
    const BOXED: &str = "Box<Vec<u8>>";
}
"#;
        let config = Config::generate_default(&[]);
        let lines: Vec<&str> = source.lines().collect();
        let file = FileCtx {
            rel: "fixture_regions.rs",
            path: Path::new("fixture_regions.rs"),
            package_name: None,
            package_publishable: None,
            lines: &lines,
            contents: source,
            config: &config,
        };
        let rules: Vec<&Rule> = inventory::iter::<Rule>
            .into_iter()
            .filter(|rule| {
                matches!(
                    rule.info.name,
                    "rust_abs_home_path"
                        | "rust_box_vec"
                        | "rust_hardcoded_url"
                        | "rust_rulewright_directives"
                )
            })
            .collect();
        let registered: HashSet<&str> = inventory::iter::<Rule>
            .into_iter()
            .map(|rule| rule.info.name)
            .collect();

        let analysis = analyze(&file, &rules, &registered, true, FileKind::Production);

        verify_true!(analysis.violations.is_empty())?;
        verify_true!(analysis.fixes.is_empty())?;

        Ok(())
    }

    #[gtest]
    fn line_rules_still_analyze_production_source() -> Result<()> {
        let source = "const PATH: &str = \"/home/dev/file\";";
        let config = Config::generate_default(&[]);
        let lines: Vec<&str> = source.lines().collect();
        let file = FileCtx {
            rel: "production.rs",
            path: Path::new("production.rs"),
            package_name: None,
            package_publishable: None,
            lines: &lines,
            contents: source,
            config: &config,
        };
        let rules: Vec<&Rule> = inventory::iter::<Rule>
            .into_iter()
            .filter(|rule| rule.info.name == "rust_abs_home_path")
            .collect();
        let registered: HashSet<&str> = inventory::iter::<Rule>
            .into_iter()
            .map(|rule| rule.info.name)
            .collect();

        let analysis = analyze(&file, &rules, &registered, false, FileKind::Production);

        verify_eq!(analysis.violations.len(), 1)?;

        Ok(())
    }

    #[gtest]
    fn directive_diagnostics_run_only_when_the_rule_is_selected() -> Result<()> {
        let source = "// #rw(rust_not_registered) intentional fixture\nfn clean() {}\n";
        let metadata = crate::all_rules();
        let config = Config::generate_default(
            &metadata
                .iter()
                .map(|rule| (rule.name, rule.params))
                .collect::<Vec<_>>(),
        );
        let lines: Vec<&str> = source.lines().collect();
        let file = FileCtx {
            rel: "fixture.rs",
            path: Path::new("fixture.rs"),
            package_name: None,
            package_publishable: None,
            lines: &lines,
            contents: source,
            config: &config,
        };
        let registered = HashSet::from(["rust_dbg"]);
        let dbg_rule = inventory::iter::<Rule>
            .into_iter()
            .find(|rule| rule.info.name == "rust_dbg")
            .or_fail()?;

        let analysis = analyze(&file, &[dbg_rule], &registered, false, FileKind::Production);

        verify_true!(analysis.violations.is_empty())
    }

    #[gtest]
    fn directive_diagnostics_honor_the_rule_path_ignore() -> Result<()> {
        let source = "// #rw(rust_not_registered) intentional fixture\nfn clean() {}\n";
        let metadata = crate::all_rules();
        let mut config = Config::generate_default(
            &metadata
                .iter()
                .map(|rule| (rule.name, rule.params))
                .collect::<Vec<_>>(),
        );

        config
            .rules
            .get_mut(DIRECTIVE_RULE)
            .or_fail()?
            .ignore
            .push("ignored/**".to_owned());
        let lines: Vec<&str> = source.lines().collect();
        let file = FileCtx {
            rel: "ignored/fixture.rs",
            path: Path::new("ignored/fixture.rs"),
            package_name: None,
            package_publishable: None,
            lines: &lines,
            contents: source,
            config: &config,
        };
        let registered = HashSet::from(["rust_dbg"]);
        let directive_rule = inventory::iter::<Rule>
            .into_iter()
            .find(|rule| rule.info.name == DIRECTIVE_RULE)
            .or_fail()?;

        let analysis = analyze(
            &file,
            &[directive_rule],
            &registered,
            false,
            FileKind::Production,
        );

        verify_true!(analysis.violations.is_empty())
    }
}
