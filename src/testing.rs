//! Supported fixture helpers for downstream rule-pack tests.

use line_index::LineIndex;
use ra_ap_syntax::{Edition, SourceFile};

use crate::{AstCtx, Config, FileCtx, Fix, Path, TomlCtx, Violation};

fn package_name_from_path(rel: &str) -> Option<&str> {
    let components: Vec<&str> = rel.split('/').collect();
    let source = components
        .iter()
        .position(|component| *component == "src")?;

    source
        .checked_sub(1)
        .and_then(|index| components.get(index).copied())
        .or(Some("fixture"))
}

/// Run a Rust line-rule callback against one fixture source file.
#[must_use]
pub fn check_source(source: &str, check: fn(&FileCtx<'_>) -> Vec<Violation>) -> Vec<Violation> {
    check_source_at("test.rs", source, check)
}

/// Run a Rust line-rule callback at an explicit workspace-relative path.
#[must_use]
pub fn check_source_at(
    rel: &str,
    source: &str,
    check: fn(&FileCtx<'_>) -> Vec<Violation>,
) -> Vec<Violation> {
    let config = Config::generate_default(&[]);
    let lines: Vec<&str> = source.lines().collect();
    let context = FileCtx {
        rel,
        path: Path::new(rel),
        package_name: package_name_from_path(rel),
        package_publishable: None,
        lines: &lines,
        contents: source,
        config: &config,
    };

    check(&context)
}

/// Run a Rust AST-rule callback against one fixture source file.
///
/// # Panics
///
/// Panics when `source` is not valid Rust syntax.
#[must_use]
pub fn check_source_ast(source: &str, check: fn(&AstCtx<'_>) -> Vec<Violation>) -> Vec<Violation> {
    check_source_ast_at("test.rs", source, check)
}

/// Run a Rust AST-rule callback at an explicit workspace-relative path.
///
/// # Panics
///
/// Panics when `source` is not valid Rust syntax.
#[must_use]
pub fn check_source_ast_at(
    rel: &str,
    source: &str,
    check: fn(&AstCtx<'_>) -> Vec<Violation>,
) -> Vec<Violation> {
    check_source_ast_at_with_publishability(rel, source, None, check)
}

pub(crate) fn check_source_ast_at_with_publishability(
    rel: &str,
    source: &str,
    package_publishable: Option<bool>,
    check: fn(&AstCtx<'_>) -> Vec<Violation>,
) -> Vec<Violation> {
    let config = Config::generate_default(&[]);
    let lines: Vec<&str> = source.lines().collect();
    let file = FileCtx {
        rel,
        path: Path::new(rel),
        package_name: package_name_from_path(rel),
        package_publishable,
        lines: &lines,
        contents: source,
        config: &config,
    };
    let parse = SourceFile::parse(source, Edition::Edition2024);

    assert!(parse.errors().is_empty(), "test source must parse");
    let root = parse.tree();
    let line_index = LineIndex::new(source);
    let context = AstCtx::new(&file, &root, &line_index, false);

    check(&context)
}

/// Run a TOML-rule callback against one fixture source file.
#[must_use]
pub fn check_source_toml(
    source: &str,
    check: fn(&TomlCtx<'_>) -> Vec<Violation>,
) -> Vec<Violation> {
    check_source_toml_at("config/policy.toml", source, check)
}

/// Run a TOML-rule callback at an explicit workspace-relative path.
#[must_use]
pub fn check_source_toml_at(
    rel: &str,
    source: &str,
    check: fn(&TomlCtx<'_>) -> Vec<Violation>,
) -> Vec<Violation> {
    let config = Config::generate_default(&[]);
    let lines: Vec<&str> = source.lines().collect();
    let file = FileCtx {
        rel,
        path: Path::new(rel),
        package_name: None,
        package_publishable: None,
        lines: &lines,
        contents: source,
        config: &config,
    };
    let parse = taplo::parser::parse(source);
    let dom = parse.clone().into_dom();

    check(&TomlCtx {
        file: &file,
        parse: &parse,
        dom: &dom,
    })
}

fn apply_fixes_to_source(source: &str, mut fixes: Vec<Fix>) -> String {
    if let [replacement] = fixes.as_slice()
        && replacement.start_line == 1
        && replacement.end_line >= source.lines().count().max(1)
    {
        return replacement.replacement.clone();
    }

    fixes.sort_by_key(|fix| std::cmp::Reverse(fix.start_line));
    let mut lines: Vec<String> = source.lines().map(String::from).collect();

    for fix in fixes {
        let start = fix.start_line.saturating_sub(1);
        let end = fix.end_line.min(lines.len());

        if start >= lines.len() || start >= end {
            continue;
        }

        if fix.replacement.is_empty() {
            lines.drain(start..end);
        } else {
            lines.splice(start..end, fix.replacement.lines().map(String::from));
        }
    }

    lines.join("\n")
}

/// Apply every fix produced by a Rust line-rule callback to one fixture source file.
#[must_use]
pub fn apply_line_fixes(
    source: &str,
    check: fn(&FileCtx<'_>) -> Vec<Violation>,
    fix: fn(&FileCtx<'_>, &Violation) -> Option<Fix>,
) -> String {
    let config = Config::generate_default(&[]);
    let lines: Vec<&str> = source.lines().collect();
    let context = FileCtx {
        rel: "test.rs",
        path: Path::new("test.rs"),
        package_name: None,
        package_publishable: None,
        lines: &lines,
        contents: source,
        config: &config,
    };
    let fixes = check(&context)
        .iter()
        .filter_map(|violation| fix(&context, violation))
        .collect();

    apply_fixes_to_source(source, fixes)
}

/// Apply every fix produced by a Rust AST-rule callback to one fixture source file.
///
/// # Panics
///
/// Panics when `source` is not valid Rust syntax.
#[must_use]
pub fn apply_ast_fixes(
    source: &str,
    check: fn(&AstCtx<'_>) -> Vec<Violation>,
    fix: fn(&AstCtx<'_>, &Violation) -> Option<Fix>,
) -> String {
    let config = Config::generate_default(&[]);
    let lines: Vec<&str> = source.lines().collect();
    let file = FileCtx {
        rel: "test.rs",
        path: Path::new("test.rs"),
        package_name: None,
        package_publishable: None,
        lines: &lines,
        contents: source,
        config: &config,
    };
    let parse = SourceFile::parse(source, Edition::Edition2024);

    assert!(parse.errors().is_empty(), "test source must parse");
    let root = parse.tree();
    let line_index = LineIndex::new(source);
    let context = AstCtx::new(&file, &root, &line_index, false);
    let fixes = check(&context)
        .iter()
        .filter_map(|violation| fix(&context, violation))
        .collect();

    apply_fixes_to_source(source, fixes)
}

/// Apply a coordinated Rust AST-tree fix to one fixture source file.
///
/// # Panics
///
/// Panics when `source` is not valid Rust syntax.
#[must_use]
pub fn apply_ast_tree_fix(
    source: &str,
    check: fn(&AstCtx<'_>) -> Vec<Violation>,
    fix: fn(&AstCtx<'_>, &[Violation]) -> Option<String>,
) -> String {
    let config = Config::generate_default(&[]);
    let lines: Vec<&str> = source.lines().collect();
    let file = FileCtx {
        rel: "test.rs",
        path: Path::new("test.rs"),
        package_name: None,
        package_publishable: None,
        lines: &lines,
        contents: source,
        config: &config,
    };
    let parse = SourceFile::parse(source, Edition::Edition2024);

    assert!(parse.errors().is_empty(), "test source must parse");
    let root = parse.tree();
    let line_index = LineIndex::new(source);
    let context = AstCtx::new(&file, &root, &line_index, false);
    let violations = check(&context);

    fix(&context, &violations).unwrap_or_else(|| source.to_owned())
}

/// Apply every fix produced by a TOML-rule callback to one fixture source file.
#[must_use]
pub fn apply_toml_fixes(
    source: &str,
    check: fn(&TomlCtx<'_>) -> Vec<Violation>,
    fix: fn(&TomlCtx<'_>, &Violation) -> Option<Fix>,
) -> String {
    apply_toml_fixes_at("config/policy.toml", source, check, fix)
}

/// Apply every fix produced by a TOML-rule callback at an explicit workspace-relative path.
#[must_use]
pub fn apply_toml_fixes_at(
    rel: &str,
    source: &str,
    check: fn(&TomlCtx<'_>) -> Vec<Violation>,
    fix: fn(&TomlCtx<'_>, &Violation) -> Option<Fix>,
) -> String {
    let config = Config::generate_default(&[]);
    let lines: Vec<&str> = source.lines().collect();
    let file = FileCtx {
        rel,
        path: Path::new(rel),
        package_name: None,
        package_publishable: None,
        lines: &lines,
        contents: source,
        config: &config,
    };
    let parse = taplo::parser::parse(source);
    let dom = parse.clone().into_dom();
    let context = TomlCtx {
        file: &file,
        parse: &parse,
        dom: &dom,
    };
    let fixes = check(&context)
        .iter()
        .filter_map(|violation| fix(&context, violation))
        .collect();

    apply_fixes_to_source(source, fixes)
}
