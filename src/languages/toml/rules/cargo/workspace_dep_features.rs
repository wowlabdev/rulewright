#[cfg(test)]
use googletest::prelude::*;

use super::{CARGO_WORKSPACE_REL, cargo_document, key_line, nested_table};
use crate::{Example, TomlCtx, Violation, violation};

#[rustfmt::skip]
const EXAMPLES: &[Example] = &[
    Example { label: "allowlisted features", code: "[workspace.dependencies]\nserde = { version = \"1\", features = [\"derive\", \"std\"] }\n", pass: true },
    Example { label: "no features", code: "[workspace.dependencies]\nserde = \"1\"\nthiserror = { version = \"2\", default-features = false }\n", pass: true },
    Example { label: "feature outside allowlist", code: "[workspace.dependencies]\ntsify = { version = \"0.5\", features = [\"js\"] }\n", pass: false },
];

crate::toml_rule!(
    toml_cargo_workspace_dep_features,
    "Flag [workspace.dependencies] entries that enable features outside the allowlist.",
    "Features are additive and belong to the consuming crate; the workspace table should only pin versions so members do not inherit unwanted features.",
    Low,
    params {
        allowed_features: [String] = ["derive", "std"]
    },
);

fn check_toml_cargo_workspace_dep_features(ctx: &TomlCtx<'_>) -> Vec<Violation> {
    if ctx.file.rel != CARGO_WORKSPACE_REL {
        return Vec::new();
    }

    let Some(document) = cargo_document(ctx) else {
        return Vec::new();
    };
    let Some(dependencies) = nested_table(&document, &["workspace", "dependencies"]) else {
        return Vec::new();
    };
    let allowed = ctx
        .file
        .config
        .get_str_array("toml_cargo_workspace_dep_features", &PARAMS[0]);

    let mut violations = Vec::new();

    for (name, value) in dependencies {
        let Some(features) = value
            .as_table()
            .and_then(|spec| spec.get("features"))
            .and_then(toml::Value::as_array)
        else {
            continue;
        };
        let disallowed: Vec<&str> = features
            .iter()
            .filter_map(toml::Value::as_str)
            .filter(|feature| !allowed.iter().any(|allowed| allowed == feature))
            .collect();

        if disallowed.is_empty() {
            continue;
        }

        violations.push(violation(
            ctx.file.rel,
            key_line(ctx.file.lines, "workspace.dependencies", name),
            format!(
                "[workspace.dependencies] `{name}` enables features {disallowed:?}; \
                 consumers enable features, the workspace table only pins versions"
            ),
        ));
    }

    violations
}

#[cfg(test)]
mod tests {
    use super::*;

    #[gtest]
    fn examples() -> Result<()> {
        for example in EXAMPLES {
            let violations = crate::test_support::check_source_toml_at(
                "Cargo.toml",
                example.code,
                check_toml_cargo_workspace_dep_features,
            );

            verify_eq!(violations.is_empty(), example.pass)?;
        }

        Ok(())
    }

    #[gtest]
    fn member_manifests_are_not_checked() -> Result<()> {
        let violations = crate::test_support::check_source_toml_at(
            "nested/foo/Cargo.toml",
            "[workspace.dependencies]\ntsify = { version = \"0.5\", features = [\"js\"] }\n",
            check_toml_cargo_workspace_dep_features,
        );

        verify_true!(violations.is_empty())?;

        Ok(())
    }
}
