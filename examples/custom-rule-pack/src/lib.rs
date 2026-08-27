//! A minimal statically linked Rulewright rule pack.

use rulewright::{FileCtx, Rule, RuleInfo, RulePack, RuleRegistry, Severity, Violation, violation};

const RULE_ID: &str = "custom_no_placeholder";

static RULES: &[Rule] = &[Rule::rust_line(
    RuleInfo::new(
        RULE_ID,
        "Flag application-specific placeholder markers.",
        "This neutral fixture demonstrates how a consuming repository owns policy outside Rulewright.",
        Severity::Medium,
        &[],
        &[],
    ),
    check_no_placeholder,
    None,
)];

fn check_no_placeholder(ctx: &FileCtx<'_>) -> Vec<Violation> {
    ctx.lines
        .iter()
        .enumerate()
        .filter(|(_, line)| line.contains("CUSTOM_PLACEHOLDER"))
        .map(|(index, _)| {
            violation(
                ctx.rel,
                index + 1,
                "replace the application-specific placeholder",
            )
        })
        .collect()
}

/// Combine Rulewright's generic built-ins with this fixture's policy.
pub fn registry() -> Result<RuleRegistry, rulewright::RegistryError> {
    let mut registry = RuleRegistry::with_builtins()?;

    registry.extend(RulePack {
        name: "custom-rule-pack",
        version: env!("CARGO_PKG_VERSION"),
        implementation_fingerprint: "custom-rule-pack-example:v1",
        rules: RULES,
    })?;

    Ok(registry)
}
