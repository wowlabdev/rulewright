# Configuration

Rulewright loads `<workspace-root>/rulewright.toml` by default. `--config <PATH>` points at another file without changing the workspace being checked. Relative CLI and environment paths start from the directory where the process was launched.

The built-in catalog is intentionally strict and sometimes extremely pedantic. The complete generated configuration is a starting point, not a claim that every repository should copy our organizational style unchanged. Tune enablement, thresholds, ignores, and named glob sets until the file describes the codebase you actually want. That checked-in file then becomes the same contract for human and AI contributors.

Target-root precedence is:

1. `--workspace-root <PATH>`
2. `RULEWRIGHT_ROOT`
3. Cargo discovery from the current directory

The selected target must resolve through Cargo metadata to a root package or virtual workspace. Missing configuration is an error except for `--init`, which creates the selected destination and refuses to overwrite it.

## Rule entries

`rulewright --init` emits every rule from the active registry, including downstream packs:

```toml
[rules.rust_max_fn_lines]
enabled = true
ignore = ["tests/fixtures/**"]
threshold = 80
```

- `enabled` selects the rule.
- `ignore` accepts workspace-relative glob patterns or names from `[glob_sets]`. Prefix a set with `@` when you want a missing set name to be an error.
- Remaining keys are typed rule parameters declared by registry metadata.

Named sets reduce repeated path policy:

```toml
[glob_sets]
generated = ["src/generated/**", "tests/snapshots/**"]

[rules.rust_pub_api_docs]
enabled = true
ignore = ["@generated"]
```

Normal runs warn about missing and unknown rule entries; missing entries are backfilled in memory. `--strict` turns all configuration warnings into errors. Unknown parameters, invalid parameter types, duplicate list values, and values outside a parameter's declared choices are always errors. For example, `rust_padding.boundaries` accepts `functions`, `control-flow`, `let-runs`, `returns`, and `tail-expressions`. `--parse-config <PATH>` prints the resolved rule configuration as JSON.

## Workspace and package selection

`--filter <SELECTOR>` is repeatable. A selector is either an exact Cargo package name or its normalized workspace-relative root; `.` denotes a root package. Selectors are not comma-split. Absolute, unmatched, and ambiguous selectors are rejected.

Walking honors `.gitignore` and `.rulewrightignore`, always excludes `.git` and Cargo's resolved target directory, and prunes nested Cargo projects that are not metadata members. Ordinary Rust and TOML files elsewhere in the workspace tree remain eligible for non-package-aware rules.
