#[cfg(test)]
use googletest::prelude::*;

use super::{assignment_line, is_cargo_manifest};
use crate::{Example, TomlCtx, Violation, violation};

#[rustfmt::skip]
const EXAMPLES: &[Example] = &[
    Example { label: "capability-named features", code: "[features]\ndefault = []\nstd = []\nserde = [\"dep:serde\"]\n", pass: true },
    Example { label: "no features table", code: "[package]\nname = \"foo\"\n", pass: true },
    Example { label: "use- prefix", code: "[features]\nuse-serde = [\"dep:serde\"]\n", pass: false },
    Example { label: "with_ prefix", code: "[features]\nwith_tokio = [\"dep:tokio\"]\n", pass: false },
    Example { label: "-support suffix", code: "[features]\nserde-support = [\"dep:serde\"]\n", pass: false },
    Example { label: "_support suffix", code: "[features]\nserde_support = [\"dep:serde\"]\n", pass: false },
];

crate::toml_rule!(
    toml_cargo_feature_names,
    "Flag Cargo feature names with use-/with- prefixes or -support suffixes.",
    "Feature names should describe the capability itself; placeholder affixes add noise without meaning (C-FEATURE).",
    Low,
    params {
        allowed: [String] = [],
    },
);

fn check_toml_cargo_feature_names(ctx: &TomlCtx<'_>) -> Vec<Violation> {
    if !is_cargo_manifest(ctx.file.rel) || !ctx.parse.errors.is_empty() {
        return Vec::new();
    }

    let Ok(document) = toml::from_str::<toml::Table>(ctx.file.contents) else {
        return Vec::new();
    };
    let Some(features) = document.get("features").and_then(toml::Value::as_table) else {
        return Vec::new();
    };
    let allowed = ctx
        .file
        .config
        .get_str_array("toml_cargo_feature_names", &PARAMS[0]);

    features
        .keys()
        .filter(|name| has_placeholder_affix(name) && !allowed.contains(name))
        .map(|name| {
            violation(
                ctx.file.rel,
                assignment_line(ctx.file.lines, name),
                format!("feature `{name}` uses a placeholder affix; name the capability itself (e.g. `serde` instead of `use-serde` or `serde-support`)"),
            )
        })
        .collect()
}

fn has_placeholder_affix(name: &str) -> bool {
    const PREFIXES: &[&str] = &["use-", "use_", "with-", "with_"];
    const SUFFIXES: &[&str] = &["-support", "_support"];

    PREFIXES.iter().any(|prefix| name.starts_with(prefix))
        || SUFFIXES.iter().any(|suffix| name.ends_with(suffix))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(source: &str) -> Vec<Violation> {
        crate::test_support::check_source_toml_at(
            "crates/foo/Cargo.toml",
            source,
            check_toml_cargo_feature_names,
        )
    }

    crate::example_tests!(EXAMPLES, check_toml_cargo_feature_names);

    #[gtest]
    fn ignores_non_cargo_toml() -> Result<()> {
        let violations = crate::test_support::check_source_toml_at(
            "crates/foo/other.toml",
            "[features]\nuse-serde = []\n",
            check_toml_cargo_feature_names,
        );

        verify_true!(violations.is_empty())?;

        Ok(())
    }
}
