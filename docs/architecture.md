# Architecture

Rulewright is intentionally one publishable crate with both a library and CLI binary. There is no internal crate graph to publish in the right order and no separate runner to keep in sync.

1. The CLI resolves the target through Cargo metadata, validates filters, and loads the active registry and configuration.
2. The walker builds an ignore-aware workspace view, pruning `.git`, Cargo output, and independent nested Cargo projects.
3. Rust files are analyzed as source lines and rust-analyzer syntax trees; TOML files use Taplo's lossless parse tree and a semantic TOML document.
4. Adapters extract shared records for Rust-only and language-neutral workspace rules.
5. Rayon executes file analysis in parallel; results are sorted before reporting.
6. The cache keys executable content, Rulewright version, configuration, rule metadata, pack identity, source checksums, and workspace context.
7. Fixes are conflict-checked and the complete edit batch is syntax-checked before writing. Each changed file is then replaced atomically under `.rulewright.lock`, the selected rules run to a bounded fixpoint, and the complete workspace is reanalyzed.

## Registry and packs

`RuleRegistry::with_builtins()` creates a deterministic ID-sorted registry. A `RulePack` contributes a stable name, semantic version, implementation fingerprint, and static rules. Duplicate rule or pack IDs fail before partial registration. The stock binary uses built-ins only; wrappers call `run_with_registry` with their combined registry.

Rule packs are ordinary, statically linked Rust code. There is no dynamic-library ABI and no code downloaded at runtime. A wrapper knows exactly which rules it ships because it builds them into the binary.

## Analysis kinds

- Rust line rules receive `FileCtx`, including the owning Cargo package name when applicable.
- Rust AST and coordinated tree-fix rules receive `AstCtx` backed by `ra_ap_syntax`.
- TOML rules receive `TomlCtx` backed by Taplo.
- Rust workspace and language-neutral workspace rules receive `WorkspaceCtx` records.

Downstream rules participate in configuration generation and validation, catalog/detail/LLM rendering, ignores, suppressions, caching, dry runs, and fixes through the same registry dispatch as built-ins.
