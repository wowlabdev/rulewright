# Changelog

## 0.3.0

- Apply size, shape, signature, duplication, ordering, layout, naming, and safety
  rules to test code: test files, inline test modules, and test-only packages.
  Public API, error-type, documentation, panic and `unwrap`, assertion-message,
  output, ambient I/O, performance, magic-number, cast, float, indexing,
  wildcard-import, and URL rules still skip it.
- Check documentation and `#[non_exhaustive]` in `publish = false` packages, and
  require `//!` docs on every module file, not only `lib.rs`.
- Add `rust_similar_fns.uncompared` for path groups that are not compared with
  each other.
- Add allow edges (`manifest-glob => glob,...`) and per-edge dependency kinds
  (`(normal, build, dev)`) to `toml_cargo_layers`.
- Add `toml_cargo_workspace_dep_features.crate_features` for per-crate feature
  grants, and the default-off `toml_cargo_exact_versions` rule.
- Add top-level `cache_inputs` for files that rules read themselves.
- `rust_ambient_syscall` follows `use` imports and aliases and reports thread,
  process, stdin, and further `std::env` calls.
- `rust_pub_api_foreign_types` follows `use` imports and aliases.
- Fix `rust_inline_test_module_size` reporting `cfg(not(test))` modules; it now
  classifies test modules like the rest of Rulewright.
- Fix `rust_large_stack_array` reporting `static` arrays.
- Report `core::fmt::Display` and `core::error::Error` in `rust_manual_error_impl`.
- Let `to_` methods consume same-file `Copy` types in
  `rust_conversion_self_convention`.
- Generate `RULES.md` from `--init` defaults instead of this repository's policy.
- Add `testing::check_source_toml_at_with_config`.

### Breaking

- Rules listed above report findings in test code, unpublished packages, and
  module files that 0.2 skipped.
- Line rules receive test items; workspace records include test functions,
  structs, and strings. Rules that should skip test code filter with
  `AstCtx::is_in_test` or `FileCtx::test_only`.
- New required parameters: `rust_similar_fns.uncompared` and
  `toml_cargo_workspace_dep_features.crate_features`.
- `DependencyRecord` has a new `version` field.
- `rust_inline_test_module_size` no longer counts `cfg(any(test, ...))` modules.

## 0.2.0

- Add per-rule `only` globs and ordered, typed per-path parameter overrides.
- Expose test-only file classification, Cargo dependency section/source/package,
  manifest package and patch declarations, and a complete walked-path inventory.
- Add generic `toml_cargo_layers` and `toml_cargo_path_deps` rules.
- Add `pack_workspace_rule!` and public multi-file workspace fixtures.
- Add `--baseline-forbid` to reject baseline entries for designated rules.
- Invalidate cached analysis and widen dirty scans when policy inputs change.
- Parameter getters now require the workspace-relative path; downstream packs
  must update context construction and getter calls for the expanded public API.

### Breaking for rule packs

- Workspace rules now receive virtual Cargo manifests as well as package manifests.
  Check `WorkspaceManifest::package_name` before assuming a package exists.
- Public context literals need new fields: `WorkspaceCtx::paths`,
  `FileCtx::test_only`, and `WorkspaceManifest::{package_name, patch_tables,
  workspace_dependencies}`. Dependency records also expose section, source, and
  renamed package identity. Prefer the public test fixtures for context creation.
- `ParamType::Bool` and `ParamDefault::Bool` extend the parameter enums. Both enums
  are now `#[non_exhaustive]`; downstream matches need a fallback arm.
- Parameter getters require a workspace-relative path so overrides can resolve.
  Pass the current file or manifest's `rel`.
- `RuleInfo` adds `needs_full_manifest_graph`. Prefer `RuleInfo::new` over literals;
  use `.with_full_manifest_graph()` for graph rules that need out-of-scope targets.
  Such rules must check `config.applies_to` before reporting source manifests.
  Context structs remain constructible for packs; making them non-exhaustive would
  prevent existing external fixture construction.
