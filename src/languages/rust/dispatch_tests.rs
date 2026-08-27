use std::collections::BTreeMap;

use super::*;
use crate::{infra::config::Config, path::Path};

fn analyze_selected(source: &str, selected: &[&str], fix_mode: bool) -> Analysis {
    let metadata = crate::all_rules();
    let config = Config::generate_default(
        &metadata
            .iter()
            .map(|rule| (rule.name, rule.params))
            .collect::<Vec<_>>(),
    );
    let lines: Vec<&str> = source.lines().collect();
    let file = FileCtx {
        rel: "dispatch.rs",
        path: Path::new("dispatch.rs"),
        package_name: None,
        lines: &lines,
        contents: source,
        config: &config,
    };
    let rules: Vec<&Rule> = inventory::iter::<Rule>
        .into_iter()
        .filter(|rule| selected.contains(&rule.info.name))
        .collect();
    let registered: HashSet<&str> = inventory::iter::<Rule>
        .into_iter()
        .map(|rule| rule.info.name)
        .collect();

    analyze(&file, &rules, &registered, fix_mode, FileKind::Production)
}

#[gtest]
fn production_line_rules_see_code_but_not_test_only_fixtures() -> Result<()> {
    let home = ["/", "home", "/dev"].concat();
    let source = r#"
const PRODUCTION: &str = "$HOME_PATH/production";

#[cfg(test)]
mod tests {
    const FIXTURE: &str = "$HOME_PATH/fixture";
}
"#
    .replace("$HOME_PATH", &home);
    let analysis = analyze_selected(&source, &["rust_abs_home_path"], false);

    verify_eq!(analysis.violations.len(), 1)?;

    verify_eq!(analysis.violations[0].line, 2)
}

#[gtest]
fn full_source_marker_rules_see_explicit_regions_inside_tests() -> Result<()> {
    let source = r#"
#[test]
fn tables() {
    #[rustfmt::skip]
    let rows = [
        // #rw:aligned
        (SHORT, "first"),
        (MUCH_LONGER_NAME, "second"),
    ];

    // #rw:sorted(asc)
    use zebra::Value;
    use alpha::Value;
}
"#;
    let analysis = analyze_selected(source, &["rust_aligned", "rust_sorted"], true);
    let rules: HashSet<&str> = analysis
        .violations
        .iter()
        .filter_map(|violation| violation.rule)
        .collect();

    verify_true!(rules.contains("rust_aligned"))?;
    verify_true!(rules.contains("rust_sorted"))?;

    verify_eq!(analysis.fixes.len(), 2)
}

#[gtest]
fn full_source_marker_rules_honor_function_suppressions_inside_tests() -> Result<()> {
    let source = r#"
// #rw(fn: rust_sorted, rust_aligned) layout fixture intentionally exercises malformed rows
#[test]
fn tables() {
    #[rustfmt::skip]
    let rows = [
        // #rw:aligned
        (SHORT, "first"),
        (MUCH_LONGER_NAME, "second"),
    ];

    // #rw:sorted(asc)
    let zebra = 1;
    let alpha = 2;
}
"#;
    let analysis = analyze_selected(source, &["rust_aligned", "rust_sorted"], true);

    verify_true!(analysis.violations.is_empty())?;

    verify_true!(analysis.fixes.is_empty())
}

#[gtest]
fn ast_dispatch_honors_each_rules_test_policy() -> Result<()> {
    let source = r"
fn production() {
    dbg!(1);
}

#[test]
fn test_only() {
    dbg!(2);
    assert_eq!(LIMIT, 10);
}
";
    let analysis = analyze_selected(source, &["rust_dbg", "rust_tautological_assert"], false);
    let dbg_lines: Vec<usize> = analysis
        .violations
        .iter()
        .filter(|violation| violation.rule == Some("rust_dbg"))
        .map(|violation| violation.line)
        .collect();
    let tautological_lines: Vec<usize> = analysis
        .violations
        .iter()
        .filter(|violation| violation.rule == Some("rust_tautological_assert"))
        .map(|violation| violation.line)
        .collect();

    verify_eq!(dbg_lines, [3])?;

    verify_eq!(tautological_lines, [9])
}

#[gtest]
fn ast_test_classification_respects_cfg_boolean_semantics() -> Result<()> {
    let production_only = ["not(", "test", ")"].concat();
    let source = r"
#[cfg(test)]
mod tests {
    fn nested() {}
}

#[test]
fn direct() {}

#[cfg(all(test, unix))]
fn all_test() {}

#[cfg(any(test, unix))]
fn any_test_or_unix() {}

#[cfg($PRODUCTION_ONLY)]
fn production_only() {}

#[cfg_attr(test, allow(dead_code))]
fn conditionally_decorated() {}

fn production() {}
"
    .replace("$PRODUCTION_ONLY", &production_only);
    let config = Config::generate_default(&[]);
    let lines: Vec<&str> = source.lines().collect();
    let file = FileCtx {
        rel: "classification.rs",
        path: Path::new("classification.rs"),
        package_name: None,
        lines: &lines,
        contents: &source,
        config: &config,
    };
    let parse = SourceFile::parse(&source, Edition::Edition2024);

    verify_true!(parse.errors().is_empty())?;

    let root = parse.tree();
    let line_index = LineIndex::new(&source);
    let ctx = AstCtx {
        file: &file,
        root: &root,
        line_index: &line_index,
        test_only_file: false,
    };
    let classification: BTreeMap<String, bool> = ctx
        .nodes::<ast::Fn>()
        .filter_map(|function| {
            function
                .name()
                .map(|name| (name.text().to_string(), ctx.is_in_test(&function)))
        })
        .collect();

    verify_eq!(classification.get("nested"), Some(&true))?;
    verify_eq!(classification.get("direct"), Some(&true))?;
    verify_eq!(classification.get("all_test"), Some(&true))?;
    verify_eq!(classification.get("any_test_or_unix"), Some(&false))?;
    verify_eq!(classification.get("production_only"), Some(&false))?;
    verify_eq!(classification.get("conditionally_decorated"), Some(&false))?;

    verify_eq!(classification.get("production"), Some(&false))
}

#[gtest]
fn malformed_rust_still_runs_lexical_rules_without_inventing_ast_results() -> Result<()> {
    let source = "fn broken( {    \n";
    let analysis = analyze_selected(source, &["rust_style", "rust_dbg"], false);

    verify_true!(
        analysis
            .violations
            .iter()
            .any(|violation| violation.rule == Some("rust_style"))
    )?;

    verify_true!(
        analysis
            .violations
            .iter()
            .all(|violation| violation.rule != Some("rust_dbg"))
    )
}
