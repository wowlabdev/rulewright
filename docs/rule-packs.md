# Rule packs

Do not fork Rulewright just because your repository has rules nobody else needs. Put that policy in the consuming repository or its own crate, combine it with the generic built-ins, and ship a tiny wrapper binary. The result is ordinary statically linked Rust with no plugin loader involved.

This minimal line rule is buildable with the public API:

```rust
use rulewright::{
    FileCtx, RegistryError, Rule, RuleInfo, RulePack, RuleRegistry, Severity, Violation,
    violation,
};

static RULES: &[Rule] = &[Rule::rust_line(
    RuleInfo::new(
        "acme_no_placeholder",
        "Flag application placeholder markers.",
        "Committed placeholders hide unfinished application policy.",
        Severity::Medium,
        &[],
        &[],
    ),
    check,
    None,
)];

fn check(ctx: &FileCtx<'_>) -> Vec<Violation> {
    ctx.lines
        .iter()
        .enumerate()
        .filter(|(_, line)| line.contains("APP_PLACEHOLDER"))
        .map(|(index, _)| violation(ctx.rel, index + 1, "replace the placeholder"))
        .collect()
}

fn registry() -> Result<RuleRegistry, RegistryError> {
    let mut registry = RuleRegistry::with_builtins()?;
    registry.extend(RulePack {
        name: "acme-policy",
        version: env!("CARGO_PKG_VERSION"),
        implementation_fingerprint: "acme-policy:v1",
        rules: RULES,
    })?;
    Ok(registry)
}

fn main() -> std::process::ExitCode {
    match registry() {
        Ok(registry) => rulewright::run_with_registry(&registry),
        Err(error) => {
            eprintln!("policy: {error}");
            std::process::ExitCode::FAILURE
        }
    }
}
```

Constructors are also available for Rust AST rules, coordinated AST-tree fixes, Rust workspace rules, language-neutral workspace rules, and TOML rules. Context helpers provide source locations and typed configuration; workspace record types are publicly readable.

For larger packs, `pack_line_rule!`, `pack_ast_rule!`, and `pack_toml_rule!` declare the rule metadata and emit a `<RULE_NAME>_RULE` constant for the pack registry. Each macro expects an `EXAMPLES: &[rulewright::Example]` constant in the same module and accepts `default = false` plus typed `params { ... }` declarations. The direct `Rule` constructors remain useful when generated code or unusual registration logic is clearer than a declaration macro.

Enable Rulewright's `rule-pack-testing` feature in the pack's development dependency to use the exported `rulewright_test!`, `rulewright_ast_test!`, `rulewright_toml_test!`, `example_tests!`, and `fix_tests!` harnesses:

```toml
[dev-dependencies]
rulewright = { version = "0.1", features = ["rule-pack-testing"] }
```

The testing feature is not needed by the wrapper binary in normal builds.

`RuleInfo::new` enables a rule in generated configurations. Chain `.disabled_by_default()` for framework-specific or unusually opinionated pack rules that should be opt-in.

## Identity and cache safety

Rule IDs must be globally unique, stable lowercase identifiers. Pack names use lowercase letters, digits, `-`, or `_`; versions must be semantic versions; implementation fingerprints must be nonempty. Descriptions, justifications, examples, parameter declarations, and fix kinds are validated before any part of a pack is registered. Change the fingerprint whenever implementation changes could affect results without changing rule metadata. Wrapper binaries additionally contribute their executable content to cache identity.

`--init`, `--list`, `--detail`, `--parse-config`, and `--llm` all use the supplied registry. The runnable [custom-rule-pack fixture](../examples/custom-rule-pack) tests analysis, generated configuration, reporting, suppressions, cleanup, and cache invalidation through an external-style wrapper.

Application-specific TOML schemas or Rust conventions should remain in this separate pack. They can reuse generic built-ins while evolving independently, without name collisions or implying that their policy is universal.
