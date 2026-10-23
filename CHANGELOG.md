# Changelog

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
