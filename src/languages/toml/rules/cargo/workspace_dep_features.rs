#[cfg(test)]
use googletest::prelude::*;

use super::{CARGO_WORKSPACE_REL, cargo_document, key_line, nested_table};
use crate::{Example, TomlCtx, Violation, violation};

#[rustfmt::skip]
const EXAMPLES: &[Example] = &[
    Example { label: "allowlisted features", code: "[workspace.dependencies]\nserde = { version = \"1\", features = [\"derive\", \"std\"] }\n", pass: true },
    Example { label: "no features", code: "[workspace.dependencies]\nserde = \"1\"\nthiserror = { version = \"2\", default-features = false }\n", pass: true },
    Example { label: "feature outside allowlist", code: "[workspace.dependencies]\ntsify = { version = \"0.5\", features = [\"js\"] }\n", pass: false },
    Example { label: "dotted dependency table", code: "[workspace.dependencies.tsify]\nversion = \"0.5\"\nfeatures = [\"js\"]\n", pass: false },
];

crate::toml_rule!(
    toml_cargo_workspace_dep_features,
    "Flag [workspace.dependencies] entries that enable features outside the allowlist.",
    "Features are additive and belong to the consuming crate; the workspace table should only pin versions so members do not inherit unwanted features. `allowed_features` applies to every entry; each `crate_features` entry `name: feature, feature` additionally grants features to the dependency with that key.",
    Low,
    params {
        allowed_features: [String] = ["derive", "std"],
        crate_features: [String] = []
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
    let allowed = ctx.file.config.get_str_array(
        "toml_cargo_workspace_dep_features",
        &TOML_CARGO_WORKSPACE_DEP_FEATURES_PARAMS[0],
        ctx.file.rel,
    );

    let crate_features = ctx.file.config.get_str_array(
        "toml_cargo_workspace_dep_features",
        &TOML_CARGO_WORKSPACE_DEP_FEATURES_PARAMS[1],
        ctx.file.rel,
    );
    let mut violations = Vec::new();

    for (name, value) in dependencies {
        let granted = crate_features
            .iter()
            .filter_map(|entry| parse_crate_features(entry))
            .filter(|(dependency, _)| dependency == name)
            .flat_map(|(_, features)| features)
            .collect::<Vec<_>>();
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
            .filter(|feature| {
                !allowed.iter().any(|allowed| allowed == feature) && !granted.contains(feature)
            })
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

fn parse_crate_features(entry: &str) -> Option<(&str, Vec<&str>)> {
    let (name, features) = entry.split_once(':')?;
    let name = name.trim();
    let features = features.split(',').map(str::trim).collect::<Vec<_>>();

    (!name.is_empty() && features.iter().all(|feature| !feature.is_empty()))
        .then_some((name, features))
}

/// Validate `crate_features` entries of the form `name: feature, feature`.
pub(crate) fn validate_crate_features(entries: &[String]) -> Result<(), String> {
    match entries
        .iter()
        .find(|entry| parse_crate_features(entry).is_none())
    {
        Some(entry) => Err(format!(
            "invalid crate_features entry `{entry}`: expected `name: feature, feature`"
        )),

        None => Ok(()),
    }
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
    fn crate_features_grant_features_to_one_dependency() -> Result<()> {
        let source = "[workspace.dependencies]\ntoml = { version = \"1\", features = [\"std\", \"parse\", \"serde\"] }\nother = { version = \"1\", features = [\"parse\"] }\n";
        let mut config = crate::Config::generate_default(&[]);

        config.rules.insert(
            "toml_cargo_workspace_dep_features".to_owned(),
            crate::infra::config::RuleConfig {
                enabled: true,
                ignore: Vec::new(),
                only: Vec::new(),
                overrides: Vec::new(),
                params: std::collections::BTreeMap::from([(
                    "crate_features".to_owned(),
                    toml::Value::Array(vec![toml::Value::String("toml: parse, serde".to_owned())]),
                )]),
            },
        );
        let violations = crate::testing::check_source_toml_at_with_config(
            "Cargo.toml",
            source,
            &config,
            check_toml_cargo_workspace_dep_features,
        );

        verify_eq!(violations.len(), 1)?;
        verify_that!(violations[0].message, contains_substring("`other`"))?;
        verify_true!(validate_crate_features(&["toml: parse".to_owned()]).is_ok())?;

        verify_true!(validate_crate_features(&["toml".to_owned(), "x: ".to_owned()]).is_err())
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
