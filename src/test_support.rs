use std::collections::BTreeMap;

use crate::path::Path;
use line_index::LineIndex;
use ra_ap_syntax::{Edition, SourceFile};

use crate::{
    AstCtx, FileCtx, TomlCtx, Violation,
    infra::{
        config::{Config, RuleConfig},
        fix::Fix,
    },
};

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

pub(crate) fn check_workspace_sources(
    sources: &[(&str, &str)],
    check: fn(&crate::languages::workspace::WorkspaceCtx<'_>) -> Vec<Violation>,
) -> Vec<Violation> {
    let config = test_config();

    check_workspace_sources_with_config(sources, &config, check)
}

pub(crate) fn check_workspace_sources_with_ignore(
    sources: &[(&str, &str)],
    rule: &str,
    ignore: &[&str],
    check: fn(&crate::languages::workspace::WorkspaceCtx<'_>) -> Vec<Violation>,
) -> Vec<Violation> {
    let mut config = test_config();

    config.rules.insert(
        rule.to_owned(),
        RuleConfig {
            enabled: true,
            ignore: ignore.iter().map(|pattern| (*pattern).to_owned()).collect(),
            params: BTreeMap::new(),
        },
    );

    check_workspace_sources_with_config(sources, &config, check)
}

fn check_workspace_sources_with_config(
    sources: &[(&str, &str)],
    config: &Config,
    check: fn(&crate::languages::workspace::WorkspaceCtx<'_>) -> Vec<Violation>,
) -> Vec<Violation> {
    let mut files = Vec::new();

    for (rel, source) in sources {
        let lines: Vec<&str> = source.lines().collect();
        let file = FileCtx {
            rel,
            path: Path::new(rel),
            package_name: None,
            lines: &lines,
            contents: source,
            config,
        };
        let parse = SourceFile::parse(source, Edition::Edition2024);

        assert!(parse.errors().is_empty(), "test source must parse");
        let mut errors = Vec::new();
        let suppressions = crate::infra::ignore::suppressed_lines(rel, &lines, &mut errors, None);

        assert!(errors.is_empty(), "test directives must parse");
        files.push(crate::languages::workspace::extract(
            &file,
            &parse.tree(),
            suppressions,
            false,
        ));
    }

    check(&crate::languages::workspace::WorkspaceCtx {
        files: &files,
        manifests: &[],
        config,
    })
}

pub(crate) fn check_workspace_member(
    member_dir: &str,
    source: &str,
    dependencies: &[(&str, &str)],
    check: fn(&crate::languages::workspace::WorkspaceCtx<'_>) -> Vec<Violation>,
) -> Vec<Violation> {
    let config = test_config();
    let rel = format!("{member_dir}/src/lib.rs");
    let lines: Vec<&str> = source.lines().collect();
    let file = FileCtx {
        rel: &rel,
        path: Path::new(&rel),
        package_name: None,
        lines: &lines,
        contents: source,
        config: &config,
    };
    let parse = SourceFile::parse(source, Edition::Edition2024);

    assert!(parse.errors().is_empty(), "test source must parse");
    let mut errors = Vec::new();
    let suppressions = crate::infra::ignore::suppressed_lines(&rel, &lines, &mut errors, None);

    assert!(errors.is_empty(), "test directives must parse");
    let files = [crate::languages::workspace::extract(
        &file,
        &parse.tree(),
        suppressions,
        false,
    )];
    let manifests = [crate::languages::workspace::WorkspaceManifest {
        rel: format!("{member_dir}/Cargo.toml"),
        dependencies: dependencies
            .iter()
            .enumerate()
            .map(
                |(index, (name, root))| crate::languages::workspace::DependencyRecord {
                    name: (*name).to_owned(),
                    root: root.replace('-', "_"),
                    line: index + 1,
                },
            )
            .collect(),
    }];

    check(&crate::languages::workspace::WorkspaceCtx {
        files: &files,
        manifests: &manifests,
        config: &config,
    })
}

fn test_config() -> Config {
    Config::generate_default(&[])
}

/// Run an AST check against `test.rs` with explicit string-array params for one rule.
///
/// # Panics
///
/// Panics when `source` is not valid Rust syntax.
pub(crate) fn check_source_ast_params(
    source: &str,
    rule: &str,
    params: &[(&str, &[&str])],
    check: fn(&AstCtx<'_>) -> Vec<Violation>,
) -> Vec<Violation> {
    let mut cfg = test_config();
    let mut param_map = BTreeMap::new();

    for (name, values) in params {
        param_map.insert(
            (*name).to_owned(),
            toml::Value::Array(
                values
                    .iter()
                    .map(|value| toml::Value::String((*value).to_owned()))
                    .collect(),
            ),
        );
    }

    cfg.rules.insert(
        rule.to_owned(),
        RuleConfig {
            enabled: true,
            ignore: Vec::new(),
            params: param_map,
        },
    );
    let lines: Vec<&str> = source.lines().collect();
    let file_ctx = FileCtx {
        rel: "test.rs",
        path: Path::new("test.rs"),
        package_name: None,
        lines: &lines,
        contents: source,
        config: &cfg,
    };
    let ra_parse = SourceFile::parse(source, Edition::Edition2024);

    assert!(ra_parse.errors().is_empty(), "test source must parse");
    let root = ra_parse.tree();
    let line_index = LineIndex::new(source);
    let ast_ctx = AstCtx {
        file: &file_ctx,
        root: &root,
        line_index: &line_index,
        test_only_file: false,
    };

    check(&ast_ctx)
}

pub(crate) fn check_source_toml(
    source: &str,
    check: fn(&TomlCtx<'_>) -> Vec<Violation>,
) -> Vec<Violation> {
    let cfg = test_config();
    let lines: Vec<&str> = source.lines().collect();
    let file_ctx = FileCtx {
        rel: "config/policy.toml",
        path: Path::new("config/policy.toml"),
        package_name: None,
        lines: &lines,
        contents: source,
        config: &cfg,
    };
    let parse = taplo::parser::parse(source);
    let dom = parse.clone().into_dom();

    check(&TomlCtx {
        file: &file_ctx,
        parse: &parse,
        dom: &dom,
    })
}

pub(crate) fn check_source_at(
    rel: &str,
    source: &str,
    check: fn(&FileCtx<'_>) -> Vec<Violation>,
) -> Vec<Violation> {
    let cfg = test_config();
    let lines: Vec<&str> = source.lines().collect();
    let ctx = FileCtx {
        rel,
        path: Path::new(rel),
        package_name: package_name_from_path(rel),
        lines: &lines,
        contents: source,
        config: &cfg,
    };

    check(&ctx)
}

pub(crate) fn check_source_ast_at(
    rel: &str,
    source: &str,
    check: fn(&AstCtx<'_>) -> Vec<Violation>,
) -> Vec<Violation> {
    let cfg = test_config();
    let lines: Vec<&str> = source.lines().collect();
    let file_ctx = FileCtx {
        rel,
        path: Path::new(rel),
        package_name: package_name_from_path(rel),
        lines: &lines,
        contents: source,
        config: &cfg,
    };
    let ra_parse = SourceFile::parse(source, Edition::Edition2024);

    assert!(ra_parse.errors().is_empty(), "test source must parse");
    let root = ra_parse.tree();
    let line_index = LineIndex::new(source);
    let ast_ctx = AstCtx {
        file: &file_ctx,
        root: &root,
        line_index: &line_index,
        test_only_file: false,
    };

    check(&ast_ctx)
}

pub(crate) fn check_source_toml_at(
    rel: &str,
    source: &str,
    check: fn(&TomlCtx<'_>) -> Vec<Violation>,
) -> Vec<Violation> {
    let cfg = test_config();
    let lines: Vec<&str> = source.lines().collect();
    let file_ctx = FileCtx {
        rel,
        path: Path::new(rel),
        package_name: None,
        lines: &lines,
        contents: source,
        config: &cfg,
    };
    let parse = taplo::parser::parse(source);
    let dom = parse.clone().into_dom();

    check(&TomlCtx {
        file: &file_ctx,
        parse: &parse,
        dom: &dom,
    })
}

pub(crate) fn check_source(
    source: &str,
    check: fn(&FileCtx<'_>) -> Vec<Violation>,
) -> Vec<Violation> {
    let cfg = test_config();
    let lines: Vec<&str> = source.lines().collect();
    let ctx = FileCtx {
        rel: "test.rs",
        path: Path::new("test.rs"),
        package_name: None,
        lines: &lines,
        contents: source,
        config: &cfg,
    };

    check(&ctx)
}

pub(crate) fn check_source_ast(
    source: &str,
    check: fn(&AstCtx<'_>) -> Vec<Violation>,
) -> Vec<Violation> {
    check_source_ast_at("test.rs", source, check)
}

fn apply_fixes_to_source(source: &str, mut fixes: Vec<Fix>) -> String {
    fixes.sort_by_key(|b| std::cmp::Reverse(b.start_line));
    let mut lines_owned: Vec<String> = source.lines().map(String::from).collect();

    for f in fixes {
        let start = f.start_line.saturating_sub(1);
        let end = f.end_line.min(lines_owned.len());

        if start >= lines_owned.len() || start >= end {
            continue;
        }

        if f.replacement.is_empty() {
            lines_owned.drain(start..end);
        } else {
            let new: Vec<String> = f.replacement.lines().map(String::from).collect();

            lines_owned.splice(start..end, new);
        }
    }

    lines_owned.join("\n")
}

pub(crate) fn apply_line_fixes(
    source: &str,
    check: fn(&FileCtx<'_>) -> Vec<Violation>,
    fix_fn: fn(&FileCtx<'_>, &Violation) -> Option<Fix>,
) -> String {
    let cfg = test_config();
    let lines: Vec<&str> = source.lines().collect();
    let ctx = FileCtx {
        rel: "test.rs",
        path: Path::new("test.rs"),
        package_name: None,
        lines: &lines,
        contents: source,
        config: &cfg,
    };
    let violations = check(&ctx);
    let fixes: Vec<Fix> = violations.iter().filter_map(|v| fix_fn(&ctx, v)).collect();

    apply_fixes_to_source(source, fixes)
}

pub(crate) fn apply_ast_fixes(
    source: &str,
    check: fn(&AstCtx<'_>) -> Vec<Violation>,
    fix_fn: fn(&AstCtx<'_>, &Violation) -> Option<Fix>,
) -> String {
    let cfg = test_config();
    let lines: Vec<&str> = source.lines().collect();
    let file_ctx = FileCtx {
        rel: "test.rs",
        path: Path::new("test.rs"),
        package_name: None,
        lines: &lines,
        contents: source,
        config: &cfg,
    };
    let ra_parse = SourceFile::parse(source, Edition::Edition2024);

    assert!(ra_parse.errors().is_empty(), "test source must parse");
    let root = ra_parse.tree();
    let line_index = LineIndex::new(source);
    let ast_ctx = AstCtx {
        file: &file_ctx,
        root: &root,
        line_index: &line_index,
        test_only_file: false,
    };
    let violations = check(&ast_ctx);
    let fixes: Vec<Fix> = violations
        .iter()
        .filter_map(|v| fix_fn(&ast_ctx, v))
        .collect();

    apply_fixes_to_source(source, fixes)
}

pub(crate) fn apply_ast_tree_fix(
    source: &str,
    check: fn(&AstCtx<'_>) -> Vec<Violation>,
    fix_fn: fn(&AstCtx<'_>, &[Violation]) -> Option<String>,
) -> String {
    let cfg = test_config();
    let lines: Vec<&str> = source.lines().collect();
    let file_ctx = FileCtx {
        rel: "test.rs",
        path: Path::new("test.rs"),
        package_name: None,
        lines: &lines,
        contents: source,
        config: &cfg,
    };
    let ra_parse = SourceFile::parse(source, Edition::Edition2024);

    assert!(ra_parse.errors().is_empty(), "test source must parse");
    let root = ra_parse.tree();
    let line_index = LineIndex::new(source);
    let ast_ctx = AstCtx {
        file: &file_ctx,
        root: &root,
        line_index: &line_index,
        test_only_file: false,
    };
    let violations = check(&ast_ctx);

    fix_fn(&ast_ctx, &violations).unwrap_or_else(|| source.to_owned())
}
