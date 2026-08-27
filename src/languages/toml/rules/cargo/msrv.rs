#[cfg(test)]
use googletest::prelude::*;

use super::{
    CARGO_WORKSPACE_REL, cargo_document, inherits_workspace, is_cargo_member, key_line,
    nested_table, section_line,
};
use crate::{Example, TomlCtx, Violation, violation};

#[rustfmt::skip]
const EXAMPLES: &[Example] = &[
    Example { label: "workspace declares MSRV", code: "[workspace]\nmembers = []\n\n[workspace.package]\nedition = \"2024\"\nrust-version = \"1.85\"\n", pass: true },
    Example { label: "workspace without MSRV", code: "[workspace]\nmembers = []\n\n[workspace.package]\nedition = \"2024\"\n", pass: false },
    Example { label: "member inherits MSRV", code: "[package]\nname = \"foo\"\nrust-version.workspace = true\n", pass: true },
    Example { label: "member omits MSRV inheritance", code: "[package]\nname = \"foo\"\n", pass: false },
    Example { label: "member overrides MSRV", code: "[package]\nname = \"foo\"\nrust-version = \"1.85\"\n", pass: false },
];

crate::toml_rule!(
    toml_cargo_msrv,
    "Require the workspace to declare rust-version and members to inherit it instead of overriding it.",
    "A declared MSRV makes the supported-compiler contract explicit, and per-crate overrides silently fragment it.",
    Low,
);

fn check_toml_cargo_msrv(ctx: &TomlCtx<'_>) -> Vec<Violation> {
    let Some(document) = cargo_document(ctx) else {
        return Vec::new();
    };

    if ctx.file.rel == CARGO_WORKSPACE_REL {
        let has_workspace = document.contains_key("workspace");
        let (section, package) = if has_workspace {
            (
                "workspace.package",
                nested_table(&document, &["workspace", "package"]),
            )
        } else {
            ("package", nested_table(&document, &["package"]))
        };
        let declares_msrv = package
            .and_then(|package| package.get("rust-version"))
            .and_then(toml::Value::as_str)
            .is_some();

        let mut violations = Vec::new();

        if !declares_msrv {
            violations.push(violation(
                ctx.file.rel,
                section_line(ctx.file.lines, section),
                format!("declare literal `rust-version` (MSRV) in [{section}]"),
            ));
        }

        if has_workspace
            && let Some(root_package) = nested_table(&document, &["package"])
            && !root_package
                .get("rust-version")
                .is_some_and(inherits_workspace)
        {
            violations.push(violation(
                ctx.file.rel,
                key_line(ctx.file.lines, "package", "rust-version"),
                "root workspace package must use `rust-version.workspace = true`",
            ));
        }

        return violations;
    }

    if is_cargo_member(ctx.file.rel) && document.contains_key("package") {
        let inherits_msrv = nested_table(&document, &["package"])
            .and_then(|package| package.get("rust-version"))
            .is_some_and(inherits_workspace);

        if !inherits_msrv {
            return vec![violation(
                ctx.file.rel,
                key_line(ctx.file.lines, "package", "rust-version"),
                "member must inherit the workspace MSRV with `rust-version.workspace = true`",
            )];
        }
    }

    Vec::new()
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
            let violations =
                crate::test_support::check_source_toml_at(rel, example.code, check_toml_cargo_msrv);

            verify_eq!(violations.is_empty(), example.pass)?;
        }

        Ok(())
    }

    #[gtest]
    fn member_without_msrv_is_rejected() -> Result<()> {
        let violations = crate::test_support::check_source_toml_at(
            "nested/foo/Cargo.toml",
            "[package]\nname = \"foo\"\nedition.workspace = true\n",
            check_toml_cargo_msrv,
        );

        verify_eq!(violations.len(), 1)?;
        verify_true!(violations[0].message.contains("must inherit"))?;

        Ok(())
    }

    #[gtest]
    fn root_package_declares_its_own_msrv() -> Result<()> {
        let violations = crate::test_support::check_source_toml_at(
            "Cargo.toml",
            "[package]\nname = \"root\"\nrust-version = \"1.85\"\n",
            check_toml_cargo_msrv,
        );

        verify_true!(violations.is_empty())?;

        Ok(())
    }

    #[gtest]
    fn combined_root_package_inherits_workspace_msrv() -> Result<()> {
        let valid = crate::test_support::check_source_toml_at(
            "Cargo.toml",
            "[workspace]\nmembers = []\n\n[workspace.package]\nrust-version = \"1.85\"\n\n[package]\nname = \"root\"\nrust-version.workspace = true\n",
            check_toml_cargo_msrv,
        );
        let invalid = crate::test_support::check_source_toml_at(
            "Cargo.toml",
            "[workspace]\nmembers = []\n\n[workspace.package]\nrust-version = \"1.85\"\n\n[package]\nname = \"root\"\nrust-version = \"1.85\"\n",
            check_toml_cargo_msrv,
        );

        verify_true!(valid.is_empty())?;

        verify_that!(
            invalid,
            contains(field!(
                Violation.message,
                contains_substring("root workspace package")
            ))
        )
    }
}
