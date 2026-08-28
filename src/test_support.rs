use std::collections::BTreeMap;

use crate::path::Path;
use line_index::LineIndex;
use ra_ap_syntax::{Edition, SourceFile};

use crate::{
    AstCtx, FileCtx, Violation,
    infra::config::{Config, RuleConfig},
};

pub(crate) use crate::testing::{
    apply_ast_fixes, apply_ast_tree_fix, apply_line_fixes, check_source_ast_at, check_source_at,
    check_source_toml_at,
};

pub(crate) fn check_source_ast_publishability(
    source: &str,
    publishable: bool,
    check: fn(&AstCtx<'_>) -> Vec<Violation>,
) -> Vec<Violation> {
    crate::testing::check_source_ast_at_with_publishability(
        "fixture/src/lib.rs",
        source,
        Some(publishable),
        check,
    )
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
            package_publishable: None,
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
        package_publishable: None,
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

pub(crate) fn check_source_params(
    source: &str,
    rule: &str,
    params: &[(&str, &[&str])],
    check: fn(&FileCtx<'_>) -> Vec<Violation>,
) -> Vec<Violation> {
    let cfg = config_with_string_params(rule, params);
    let lines: Vec<&str> = source.lines().collect();
    let ctx = FileCtx {
        rel: "test.rs",
        path: Path::new("test.rs"),
        package_name: None,
        package_publishable: None,
        lines: &lines,
        contents: source,
        config: &cfg,
    };

    check(&ctx)
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
    let cfg = config_with_string_params(rule, params);
    let lines: Vec<&str> = source.lines().collect();
    let file_ctx = FileCtx {
        rel: "test.rs",
        path: Path::new("test.rs"),
        package_name: None,
        package_publishable: None,
        lines: &lines,
        contents: source,
        config: &cfg,
    };
    let ra_parse = SourceFile::parse(source, Edition::Edition2024);

    assert!(ra_parse.errors().is_empty(), "test source must parse");
    let root = ra_parse.tree();
    let line_index = LineIndex::new(source);
    let ast_ctx = AstCtx::new(&file_ctx, &root, &line_index, false);

    check(&ast_ctx)
}

fn config_with_string_params(rule: &str, params: &[(&str, &[&str])]) -> Config {
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

    cfg
}
