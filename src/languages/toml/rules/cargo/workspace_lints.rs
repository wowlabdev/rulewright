#[cfg(test)]
use googletest::prelude::*;

use super::{
    CARGO_WORKSPACE_REL, cargo_document, is_cargo_member, key_line, nested_table, section_line,
};
use crate::{Example, RuleParam, TomlCtx, Violation, violation};

#[rustfmt::skip]
const EXAMPLES: &[Example] = &[
    Example {
        label: "workspace with full lint set",
        code: r#"[workspace.lints.rust]
ambiguous_negative_literals = "warn"
missing_debug_implementations = "warn"
redundant_imports = "warn"
redundant_lifetimes = "warn"
trivial_numeric_casts = "warn"
unsafe_op_in_unsafe_fn = "warn"
unused_lifetimes = "warn"

[workspace.lints.clippy]
cargo = { level = "warn", priority = -1 }
complexity = { level = "warn", priority = -1 }
correctness = { level = "deny", priority = -1 }
pedantic = { level = "warn", priority = -1 }
perf = { level = "warn", priority = -1 }
style = { level = "warn", priority = -1 }
suspicious = { level = "warn", priority = -1 }
allow_attributes_without_reason = "warn"
as_pointer_underscore = "warn"
assertions_on_result_states = "warn"
clone_on_ref_ptr = "warn"
deref_by_slicing = "warn"
disallowed_script_idents = "warn"
empty_drop = "warn"
empty_enum_variants_with_brackets = "warn"
empty_structs_with_brackets = "warn"
fn_to_numeric_cast_any = "warn"
if_then_some_else_none = "warn"
implicit_clone = "warn"
map_err_ignore = "warn"
redundant_type_annotations = "warn"
renamed_function_params = "warn"
semicolon_outside_block = "warn"
undocumented_unsafe_blocks = "warn"
unnecessary_safety_comment = "warn"
unnecessary_safety_doc = "warn"
unneeded_field_pattern = "warn"
unused_result_ok = "warn"
"#,
        pass: true,
    },
    Example { label: "workspace missing required lints", code: "[workspace]\nmembers = []\n", pass: false },
    Example { label: "workspace lint downgraded to allow", code: "[workspace.lints.rust]\nunsafe_op_in_unsafe_fn = \"allow\"\n", pass: false },
    Example { label: "member inherits workspace lints", code: "[package]\nname = \"foo\"\n\n[lints]\nworkspace = true\n", pass: true },
    Example { label: "member without lint inheritance", code: "[package]\nname = \"foo\"\n", pass: false },
];

crate::toml_rule!(
    toml_cargo_workspace_lints,
    "Require the workspace to enable the standard rust/clippy lint set and members to inherit it via `[lints] workspace = true`.",
    "Static verification only catches issues when every crate compiles under the same vetted workspace lint tables.",
    Medium,
    params {
        required_rust: [String] = [
            "ambiguous_negative_literals",
            "missing_debug_implementations",
            "redundant_imports",
            "redundant_lifetimes",
            "trivial_numeric_casts",
            "unsafe_op_in_unsafe_fn",
            "unused_lifetimes",
        ],
        required_clippy: [String] = [
            "cargo",
            "complexity",
            "correctness",
            "pedantic",
            "perf",
            "style",
            "suspicious",
            "allow_attributes_without_reason",
            "as_pointer_underscore",
            "assertions_on_result_states",
            "clone_on_ref_ptr",
            "deref_by_slicing",
            "disallowed_script_idents",
            "empty_drop",
            "empty_enum_variants_with_brackets",
            "empty_structs_with_brackets",
            "fn_to_numeric_cast_any",
            "if_then_some_else_none",
            "implicit_clone",
            "map_err_ignore",
            "redundant_type_annotations",
            "renamed_function_params",
            "semicolon_outside_block",
            "undocumented_unsafe_blocks",
            "unnecessary_safety_comment",
            "unnecessary_safety_doc",
            "unneeded_field_pattern",
            "unused_result_ok",
        ],
    },
);

fn check_toml_cargo_workspace_lints(ctx: &TomlCtx<'_>) -> Vec<Violation> {
    let Some(document) = cargo_document(ctx) else {
        return Vec::new();
    };

    if ctx.file.rel == CARGO_WORKSPACE_REL && document.contains_key("workspace") {
        let mut violations = Vec::new();

        required_group_violations(ctx, &document, "rust", &PARAMS[0], &mut violations);
        required_group_violations(ctx, &document, "clippy", &PARAMS[1], &mut violations);

        return violations;
    }

    if is_cargo_member(ctx.file.rel) && document.contains_key("package") {
        return member_violations(ctx, &document);
    }

    Vec::new()
}

fn required_group_violations(
    ctx: &TomlCtx<'_>,
    document: &toml::Table,
    group: &str,
    param: &RuleParam,
    violations: &mut Vec<Violation>,
) {
    let section = format!("workspace.lints.{group}");
    let table = nested_table(document, &["workspace", "lints", group]);

    for lint in ctx
        .file
        .config
        .get_str_array("toml_cargo_workspace_lints", param)
    {
        match table.and_then(|table| table.get(&lint)) {
            Some(value) if lint_level(value) == Some("allow") => violations.push(violation(
                ctx.file.rel,
                key_line(ctx.file.lines, &section, &lint),
                format!("[{section}] sets `{lint}` to allow; it is required at warn or deny"),
            )),
            Some(_) => {}
            None => violations.push(violation(
                ctx.file.rel,
                section_line(ctx.file.lines, &section),
                format!("[{section}] must enable `{lint}` at warn or deny"),
            )),
        }
    }
}

fn lint_level(value: &toml::Value) -> Option<&str> {
    match value {
        toml::Value::String(level) => Some(level),
        toml::Value::Table(table) => table.get("level").and_then(toml::Value::as_str),
        _ => None,
    }
}

fn member_violations(ctx: &TomlCtx<'_>, document: &toml::Table) -> Vec<Violation> {
    let inherits = nested_table(document, &["lints"])
        .and_then(|lints| lints.get("workspace"))
        .and_then(toml::Value::as_bool)
        == Some(true);

    if inherits {
        return Vec::new();
    }

    vec![violation(
        ctx.file.rel,
        section_line(ctx.file.lines, "lints"),
        "member manifest must inherit workspace lints via `[lints] workspace = true`",
    )]
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
                check_toml_cargo_workspace_lints,
            );

            verify_eq!(violations.is_empty(), example.pass)?;
        }

        Ok(())
    }

    #[gtest]
    fn missing_workspace_lints_flags_every_required_entry() -> Result<()> {
        let violations = crate::test_support::check_source_toml_at(
            "Cargo.toml",
            "[workspace]\nmembers = []\n",
            check_toml_cargo_workspace_lints,
        );

        verify_eq!(violations.len(), 35)?;

        Ok(())
    }

    #[gtest]
    fn non_cargo_toml_is_ignored() -> Result<()> {
        let violations = crate::test_support::check_source_toml_at(
            "crates/rulewright.toml",
            "[rules.x]\nenabled = true\n",
            check_toml_cargo_workspace_lints,
        );

        verify_true!(violations.is_empty())?;

        Ok(())
    }
}
