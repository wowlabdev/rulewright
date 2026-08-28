#[cfg(test)]
use googletest::prelude::*;

use super::{
    CARGO_WORKSPACE_REL, cargo_document, inherits_workspace, is_cargo_member, key_line,
    nested_table, rust_version_supports_edition, section_line,
};
use crate::{Example, TomlCtx, Violation, violation};

const RESOLVER_REQUIRED_EDITION: i64 = 2024;
const CURRENT_RESOLVER: &str = "3";

#[rustfmt::skip]
const EXAMPLES: &[Example] = &[
    Example { label: "workspace on current edition", code: "[workspace]\nmembers = []\n\n[workspace.package]\nedition = \"2024\"\n", pass: false },
    Example { label: "workspace on old edition", code: "[workspace]\nresolver = \"2\"\nmembers = []\n\n[workspace.package]\nedition = \"2021\"\n", pass: false },
    Example { label: "redundant resolver on current edition", code: "[workspace]\nresolver = \"3\"\nmembers = []\n\n[workspace.package]\nedition = \"2024\"\n", pass: true },
    Example { label: "member inherits edition", code: "[package]\nname = \"foo\"\nedition.workspace = true\n", pass: true },
    Example { label: "member omits edition", code: "[package]\nname = \"foo\"\n", pass: false },
    Example { label: "member on old edition", code: "[package]\nname = \"foo\"\nedition = \"2021\"\n", pass: false },
];

crate::toml_rule!(
    toml_cargo_edition,
    "Require the workspace and non-inheriting members to target at least the configured Rust edition, with the matching virtual-workspace resolver.",
    "Virtual workspaces do not infer the resolver from workspace.package.edition, so resolver 3 must be explicit for edition 2024.",
    Medium,
    params {
        min_edition: i64 = 2024
    },
);

fn check_toml_cargo_edition(ctx: &TomlCtx<'_>) -> Vec<Violation> {
    let Some(document) = cargo_document(ctx) else {
        return Vec::new();
    };
    let min_edition = ctx
        .file
        .config
        .get_i64("toml_cargo_edition", &TOML_CARGO_EDITION_PARAMS[0]);

    if ctx.file.rel == CARGO_WORKSPACE_REL {
        let mut violations = Vec::new();

        if document.contains_key("workspace") {
            violations.extend(workspace_violations(ctx, &document, min_edition));
        }

        if document.contains_key("package") {
            violations.extend(member_violations(ctx, &document, min_edition));
        }

        return violations;
    }

    if is_cargo_member(ctx.file.rel) && document.contains_key("package") {
        return member_violations(ctx, &document, min_edition);
    }

    Vec::new()
}

fn workspace_violations(
    ctx: &TomlCtx<'_>,
    document: &toml::Table,
    min_edition: i64,
) -> Vec<Violation> {
    let mut violations = Vec::new();
    let Some(package) = nested_table(document, &["workspace", "package"]) else {
        return vec![violation(
            ctx.file.rel,
            section_line(ctx.file.lines, "workspace"),
            "declare `edition` in [workspace.package]",
        )];
    };
    let Some(edition) = package.get("edition").and_then(edition_value) else {
        return vec![violation(
            ctx.file.rel,
            section_line(ctx.file.lines, "workspace.package"),
            "declare a literal Rust edition in [workspace.package]",
        )];
    };

    if edition < min_edition {
        violations.push(violation(
            ctx.file.rel,
            key_line(ctx.file.lines, "workspace.package", "edition"),
            format!("workspace edition {edition} is below the minimum {min_edition}"),
        ));
    }

    if let Some(rust_version) = package.get("rust-version").and_then(toml::Value::as_str)
        && !rust_version_supports_edition(rust_version, edition)
    {
        violations.push(violation(
            ctx.file.rel,
            key_line(ctx.file.lines, "workspace.package", "rust-version"),
            format!("rust-version {rust_version} cannot compile Rust edition {edition}"),
        ));
    }

    if edition >= RESOLVER_REQUIRED_EDITION && !document.contains_key("package") {
        let resolver = nested_table(document, &["workspace"])
            .and_then(|workspace| workspace.get("resolver"))
            .and_then(toml::Value::as_str);

        if resolver != Some(CURRENT_RESOLVER) {
            violations.push(violation(
                ctx.file.rel,
                key_line(ctx.file.lines, "workspace", "resolver"),
                format!(
                    "virtual workspace on edition {edition} must set `resolver = \"{CURRENT_RESOLVER}\"`"
                ),
            ));
        }
    }

    violations
}

fn member_violations(
    ctx: &TomlCtx<'_>,
    document: &toml::Table,
    min_edition: i64,
) -> Vec<Violation> {
    let Some(package) = nested_table(document, &["package"]) else {
        return Vec::new();
    };
    let Some(value) = package.get("edition") else {
        return vec![violation(
            ctx.file.rel,
            section_line(ctx.file.lines, "package"),
            "declare `edition` or inherit it with `edition.workspace = true`",
        )];
    };

    if inherits_workspace(value) {
        return Vec::new();
    }

    let Some(edition) = edition_value(value) else {
        return vec![violation(
            ctx.file.rel,
            key_line(ctx.file.lines, "package", "edition"),
            "edition must be a literal year or `edition.workspace = true`",
        )];
    };
    let mut violations = Vec::new();

    if edition < min_edition {
        violations.push(violation(
            ctx.file.rel,
            key_line(ctx.file.lines, "package", "edition"),
            format!("crate edition {edition} is below the minimum {min_edition}; prefer `edition.workspace = true`"),
        ));
    }

    if let Some(rust_version) = package.get("rust-version").and_then(toml::Value::as_str)
        && !rust_version_supports_edition(rust_version, edition)
    {
        violations.push(violation(
            ctx.file.rel,
            key_line(ctx.file.lines, "package", "rust-version"),
            format!("rust-version {rust_version} cannot compile Rust edition {edition}"),
        ));
    }

    violations
}

/// Literal edition as a number; `edition.workspace = true` yields `None`.
fn edition_value(value: &toml::Value) -> Option<i64> {
    match value {
        toml::Value::String(edition) => edition.parse().ok(),
        toml::Value::Integer(edition) => Some(*edition),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[gtest]
    fn examples() -> Result<()> {
        for example in EXAMPLES {
            let rel = if example.code.contains("[workspace") {
                "Cargo.toml"
            } else {
                "nested/foo/Cargo.toml"
            };
            let violations = crate::test_support::check_source_toml_at(
                rel,
                example.code,
                check_toml_cargo_edition,
            );

            verify_eq!(violations.is_empty(), example.pass)?;
        }

        Ok(())
    }

    #[gtest]
    fn workspace_resolver_must_match_the_current_edition() -> Result<()> {
        let current = crate::test_support::check_source_toml_at(
            "Cargo.toml",
            "[workspace]\nresolver = \"2\"\nmembers = []\n\n[workspace.package]\nedition = \"2024\"\nversion = \"0.1.0\"\n",
            check_toml_cargo_edition,
        );

        verify_eq!(current.len(), 1)?;
        verify_true!(current[0].message.contains("resolver"))?;
        verify_eq!(current[0].line, 2)?;

        let old = crate::test_support::check_source_toml_at(
            "Cargo.toml",
            "[workspace]\nresolver = \"2\"\nmembers = []\n\n[workspace.package]\nedition = \"2021\"\n",
            check_toml_cargo_edition,
        );

        verify_eq!(old.len(), 1)?;
        verify_true!(old[0].message.contains("edition 2021"))?;

        let matching = crate::test_support::check_source_toml_at(
            "Cargo.toml",
            "[workspace]\nresolver = \"3\"\nmembers = []\n\n[workspace.package]\nedition = \"2024\"\n",
            check_toml_cargo_edition,
        );

        verify_true!(matching.is_empty())?;

        let missing = crate::test_support::check_source_toml_at(
            "Cargo.toml",
            "[workspace]\nmembers = []\n\n[workspace.package]\nedition = \"2024\"\n",
            check_toml_cargo_edition,
        );

        verify_eq!(missing.len(), 1)?;

        Ok(())
    }

    #[gtest]
    fn root_package_uses_its_package_edition_without_a_virtual_resolver() -> Result<()> {
        let violations = crate::test_support::check_source_toml_at(
            "Cargo.toml",
            "[package]\nname = \"root\"\nedition = \"2024\"\n",
            check_toml_cargo_edition,
        );

        verify_true!(violations.is_empty())?;

        Ok(())
    }

    #[gtest]
    fn edition_requires_a_compatible_rust_version_when_both_are_literal() -> Result<()> {
        let violations = crate::test_support::check_source_toml_at(
            "Cargo.toml",
            "[package]\nname = \"root\"\nedition = \"2024\"\nrust-version = \"1.70\"\n",
            check_toml_cargo_edition,
        );

        verify_that!(
            violations,
            contains(field!(
                Violation.message,
                contains_substring("cannot compile")
            ))
        )
    }
}
