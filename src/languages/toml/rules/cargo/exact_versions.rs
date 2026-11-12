#[cfg(test)]
use googletest::prelude::*;

use crate::{DependencyRecord, Example, TomlCtx, Violation, violation};

#[rustfmt::skip]
const EXAMPLES: &[Example] = &[
    Example { label: "exact registry version", code: "[dependencies]\nserde = \"=1.0.228\"\n", pass: true },
    Example { label: "exact workspace table version", code: "[workspace.dependencies]\nserde = { version = \"=1.0.228\", default-features = false }\n", pass: true },
    Example { label: "inherited and path dependencies", code: "[dependencies]\nserde.workspace = true\nlocal = { path = \"../local\" }\n", pass: true },
    Example { label: "caret requirement", code: "[dependencies]\nserde = \"1\"\n", pass: false },
    Example { label: "partial exact requirement", code: "[dev-dependencies]\nserde = { version = \"=1.0\" }\n", pass: false },
    Example { label: "range requirement", code: "[workspace.dependencies]\nserde = \">=1, <2\"\n", pass: false },
];

crate::toml_rule!(
    toml_cargo_exact_versions,
    "Require every declared dependency version to be an exact `=major.minor.patch` requirement.",
    "Exact requirements make the agreed version the only one a manifest accepts, so an upgrade is a reviewed edit of the manifest rather than a side effect of `cargo update`. Inherited (`workspace = true`) and version-less path or Git dependencies are not checked.",
    Medium,
    default = false,
);

fn check_toml_cargo_exact_versions(ctx: &TomlCtx<'_>) -> Vec<Violation> {
    let Some(manifest) = crate::languages::toml::workspace_manifest(ctx) else {
        return Vec::new();
    };

    manifest
        .workspace_dependencies
        .iter()
        .chain(&manifest.dependencies)
        .filter_map(|dependency| {
            let version = dependency.version.as_deref()?;

            (!is_exact(version)).then(|| inexact(ctx, dependency, version))
        })
        .collect()
}

fn inexact(ctx: &TomlCtx<'_>, dependency: &DependencyRecord, version: &str) -> Violation {
    violation(
        ctx.file.rel,
        dependency.line,
        format!(
            "dependency `{}` requires `{version}`; pin an exact `=major.minor.patch` version",
            dependency.name
        ),
    )
}

fn is_exact(version: &str) -> bool {
    semver::VersionReq::parse(version).is_ok_and(|requirement| {
        matches!(
            requirement.comparators.as_slice(),
            [comparator] if comparator.op == semver::Op::Exact
                && comparator.minor.is_some()
                && comparator.patch.is_some()
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[gtest]
    fn examples() -> Result<()> {
        for example in EXAMPLES {
            let violations = crate::test_support::check_source_toml_at(
                "member/Cargo.toml",
                example.code,
                check_toml_cargo_exact_versions,
            );

            verify_eq!(violations.is_empty(), example.pass)?;
        }

        Ok(())
    }

    #[gtest]
    fn target_tables_are_checked_at_their_lines() -> Result<()> {
        let source =
            "[package]\nname = \"m\"\n\n[target.'cfg(unix)'.dependencies]\nlibc = \"0.2\"\n";
        let violations = crate::test_support::check_source_toml_at(
            "member/Cargo.toml",
            source,
            check_toml_cargo_exact_versions,
        );

        verify_eq!(violations.len(), 1)?;

        verify_eq!(violations[0].line, 5)
    }
}
