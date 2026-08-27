#[cfg(test)]
use googletest::prelude::*;
use std::collections::HashSet;

use crate::{Example, Violation, languages::workspace::WorkspaceCtx, matches_ignore, violation};

#[rustfmt::skip]
const EXAMPLES: &[Example] = &[
    Example { label: "unused dependency", code: "fn main() {}", pass: false },
    Example { label: "path-referenced dependency", code: "fn encode(value: serde::Value) {}", pass: true },
    Example { label: "derive-only dependency", code: "#[derive(serde::Serialize)] struct Value;", pass: true },
];

crate::language_workspace_rule!(
    toml_cargo_unused_deps,
    "Flag workspace-member dependencies that are never referenced by any Rust compile target.",
    "Unused dependencies increase build time and supply-chain surface. Full-workspace runs are authoritative because references are aggregated across src, tests, benches, examples, and build.rs.",
    Medium,
    params {
        always_used: [String] = [],
    },
);

fn check_toml_cargo_unused_deps(ctx: &WorkspaceCtx<'_>) -> Vec<Violation> {
    let always_used: HashSet<String> = ctx
        .config
        .get_str_array("toml_cargo_unused_deps", &PARAMS[0])
        .into_iter()
        .map(|name| name.replace('-', "_"))
        .collect();
    let mut violations = Vec::new();

    for manifest in ctx.manifests {
        if matches_ignore(
            &manifest.rel,
            ctx.config.ignore_patterns("toml_cargo_unused_deps"),
        ) {
            continue;
        }

        let referenced: HashSet<&str> = ctx
            .files
            .iter()
            .filter(|file| belongs_to_manifest(&file.rel, &manifest.rel, ctx.manifests))
            .flat_map(|file| file.crate_roots.iter().map(String::as_str))
            .collect();

        for dependency in &manifest.dependencies {
            if always_used.contains(&dependency.root)
                || referenced.contains(dependency.root.as_str())
            {
                continue;
            }

            violations.push(violation(
                &manifest.rel,
                dependency.line,
                format!(
                    "dependency `{}` is never referenced by any Rust target in this workspace member",
                    dependency.name
                ),
            ));
        }
    }

    violations
}

fn belongs_to_manifest(
    source_rel: &str,
    manifest_rel: &str,
    manifests: &[crate::WorkspaceManifest],
) -> bool {
    let Some((member_dir, _)) = manifest_rel.rsplit_once('/') else {
        return manifests.iter().all(|other| {
            other.rel == manifest_rel
                || other
                    .rel
                    .rsplit_once('/')
                    .is_none_or(|(other_dir, _)| !source_rel.starts_with(&format!("{other_dir}/")))
        });
    };

    source_rel.starts_with(&format!("{member_dir}/"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(source: &str, dependency_name: &str) -> Vec<Violation> {
        crate::test_support::check_workspace_member(
            "crates/example",
            source,
            &[(dependency_name, dependency_name)],
            check_toml_cargo_unused_deps,
        )
    }

    #[gtest]
    fn examples() -> Result<()> {
        for example in EXAMPLES {
            let violations = run(example.code, "serde");

            verify_eq!(violations.is_empty(), example.pass)?;
        }

        Ok(())
    }

    #[gtest]
    fn renamed_dependency_uses_the_manifest_key_as_its_crate_root() -> Result<()> {
        let violations = run("fn decode(value: json::Value) {}", "json");

        verify_true!(violations.is_empty())?;

        Ok(())
    }

    #[gtest]
    fn qualified_path_inside_macro_tokens_counts_as_used() -> Result<()> {
        let violations = run(
            "macro_rules! encode { () => { serde::Serialize }; }",
            "serde",
        );

        verify_true!(violations.is_empty())?;

        Ok(())
    }
}
