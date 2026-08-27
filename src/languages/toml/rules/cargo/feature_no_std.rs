use super::{assignment_line, is_cargo_manifest};
use crate::{Example, TomlCtx, Violation, violation};

#[rustfmt::skip]
const EXAMPLES: &[Example] = &[
    Example { label: "additive std feature", code: "[features]\ndefault = [\"std\"]\nstd = []\n", pass: true },
    Example { label: "no features table", code: "[package]\nname = \"foo\"\n", pass: true },
    Example { label: "no-std feature", code: "[features]\nno-std = []\n", pass: false },
    Example { label: "no_std feature", code: "[features]\nno_std = []\n", pass: false },
    Example { label: "nostd feature", code: "[features]\nnostd = []\n", pass: false },
];

crate::toml_rule!(
    toml_cargo_feature_no_std,
    "Ban subtractive no-std Cargo features; provide an additive std feature instead.",
    "Features must be additive so any combination compiles; a no-std feature that removes functionality breaks feature unification.",
    Medium,
);

fn check_toml_cargo_feature_no_std(ctx: &TomlCtx<'_>) -> Vec<Violation> {
    if !is_cargo_manifest(ctx.file.rel) || !ctx.parse.errors.is_empty() {
        return Vec::new();
    }

    let Ok(document) = toml::from_str::<toml::Table>(ctx.file.contents) else {
        return Vec::new();
    };
    let Some(features) = document.get("features").and_then(toml::Value::as_table) else {
        return Vec::new();
    };

    features
        .keys()
        .filter(|name| matches!(name.as_str(), "no-std" | "no_std" | "nostd"))
        .map(|name| {
            violation(
                ctx.file.rel,
                assignment_line(ctx.file.lines, name),
                format!(
                    "feature `{name}` is subtractive; invert it into an additive `std` feature"
                ),
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(source: &str) -> Vec<Violation> {
        crate::test_support::check_source_toml_at(
            "crates/foo/Cargo.toml",
            source,
            check_toml_cargo_feature_no_std,
        )
    }

    crate::example_tests!(EXAMPLES, check_toml_cargo_feature_no_std);
}
