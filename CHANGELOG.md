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
