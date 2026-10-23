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

For larger packs, `pack_line_rule!`, `pack_ast_rule!`, `pack_toml_rule!`, and `pack_workspace_rule!` declare the rule metadata and emit a `<RULE_NAME>_RULE` constant for the pack registry. Each macro expects an `EXAMPLES: &[rulewright::Example]` constant in the same module and accepts `default = false` plus typed `params { ... }` declarations. Workspace and TOML macros use the exact supplied ID; Rust line and AST macros prefix it with `rust_`. The direct `Rule` constructors remain useful when generated code or unusual registration logic is clearer than a declaration macro.

Enable Rulewright's `rule-pack-testing` feature in the pack's development dependency to use the exported `rulewright_test!`, `rulewright_ast_test!`, `rulewright_toml_test!`, `example_tests!`, and `fix_tests!` harnesses:

```toml
[dev-dependencies]
rulewright = { version = "0.2", features = ["rule-pack-testing"] }
```

The testing feature is not needed by the wrapper binary in normal builds.

`rulewright_workspace_test!` provides `run(source)` for one Rust file and `run_files(&[("Cargo.toml", manifest), ("src/lib.rs", source), ("script.py", script)])` for a workspace fixture. The public `testing::check_workspace_sources_with_config` helper accepts an explicit configuration. Workspace fixtures parse Cargo records and expose all supplied paths, including non-Rust/TOML files.

## Context and parameter migration in 0.2

Pass a normalized workspace-relative path to every parameter lookup:
`ctx.config.get_usize("my_rule", &MY_RULE_PARAMS[0], ctx.rel)` for line rules,
or `ctx.file.rel` for AST/TOML rules. Workspace rules should use the relevant
record or finding path. Boolean parameters use `get_bool`.

`FileCtx::test_only` uses the same Cargo target and module-graph classification
as AST rules. Ordinary line rules still receive the production-only line view;
use `Rule::rust_line_full` when a rule needs the complete test source.

`WorkspaceCtx::paths` is sorted, deduplicated, and includes every file admitted
by the walker, including foreign-language files. Ignore files, hidden-file
policy, workspace boundaries, and Cargo output exclusions still apply.
`WorkspaceManifest` contains `package_name: Option<String>`, ordinary
`dependencies`, separate `workspace_dependencies`, and `patch_tables` keyed
by patch source. Virtual workspace manifests have no package name.
`DependencyRecord` preserves the dependency key/import root and source line,
adds `DependencySection::{Normal, Dev, Build}`, and exposes
`DependencySource::{Registry, Path(String), Git, Workspace}` plus an optional
explicit renamed `package`. Sources are declarations, not Cargo resolution;
an inherited record retains `Workspace`, and consumers can resolve it using
the root's shared declarations. Target-specific entries are retained for every
target condition, with their own source locations.

`RuleInfo::new` enables a rule in generated configurations. Chain `.disabled_by_default()` for framework-specific or unusually opinionated pack rules that should be opt-in.

## Identity and cache safety

Rule IDs must be globally unique, stable lowercase identifiers. Pack names use lowercase letters, digits, `-`, or `_`; versions must be semantic versions; implementation fingerprints must be nonempty. Descriptions, justifications, examples, parameter declarations, and fix kinds are validated before any part of a pack is registered. Change the fingerprint whenever implementation changes could affect results without changing rule metadata. Wrapper binaries additionally contribute their executable content to cache identity.

`--init`, `--list`, `--detail`, `--parse-config`, and `--llm` all use the supplied registry. The runnable [custom-rule-pack fixture](../examples/custom-rule-pack) tests analysis, generated configuration, reporting, suppressions, cleanup, and cache invalidation through an external-style wrapper.

Application-specific TOML schemas or Rust conventions should remain in this separate pack. They can reuse generic built-ins while evolving independently, without name collisions or implying that their policy is universal.
