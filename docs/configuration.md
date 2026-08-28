# Configuration

Rulewright loads `<workspace-root>/rulewright.toml` by default. `--config <PATH>` points at another file without changing the workspace being checked. Relative CLI and environment paths start from the directory where the process was launched.

The built-in catalog is intentionally strict and sometimes extremely pedantic. The complete generated configuration is a starting point, not a claim that every repository should copy our organizational style unchanged. Tune enablement, thresholds, ignores, and named glob sets until the file describes the codebase you actually want. That checked-in file then becomes the same contract for human and AI contributors.

Target-root precedence is:

1. `--workspace-root <PATH>`
2. `RULEWRIGHT_ROOT`
3. Cargo discovery from the current directory

The selected target must resolve through Cargo metadata to a root package or virtual workspace. Missing configuration is an error except for `--init`, which creates the selected destination and refuses to overwrite it.

## Source suppressions

Generated configurations allow source suppression directives by default. Repositories that keep every exception in shared configuration can reject them before rule dispatch:

```toml
allow_suppressions = false
```

With that policy, `#rw(...)` produces a `rust_rulewright_directives` finding and does not suppress its target. Layout markers such as `#rw:aligned` and `#rw:sorted(asc)` are unaffected because they opt into checks instead of hiding findings.

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

Ignore patterns are matched against slash-separated paths relative to the workspace root. `*` stays within one path component and `**` crosses directories, so `**/*_tests.rs` covers split test modules at the root and at any nesting depth, while `crates/example-tests/**` covers an entire workspace member. Invalid patterns are rejected while loading the configuration instead of silently matching nothing. Rulewright normalizes discovered Windows paths before matching them.

Cargo test, benchmark, and example targets are classified as test code from Cargo metadata. That classification follows their `mod`, `#[path = "..."]`, and `include!("...")` edges, as does any Rust file reached only through `#[cfg(test)] mod ...`; physical filenames are not used as a substitute for the module graph.

A workspace member built only as test infrastructure can declare that all of its Rust targets are test-only:

```toml
[package.metadata.rulewright]
test-only = true
```

Use this only for dedicated fixture or test-harness packages. A package containing production library or binary code should leave it unset.

Normal runs warn about missing and unknown rule entries; missing entries are backfilled in memory. `--strict` turns all configuration warnings into errors. Unknown parameters, invalid parameter types, duplicate list values, and values outside a parameter's declared choices are always errors. For example, `rust_padding.boundaries` accepts `functions`, `control-flow`, `let-runs`, `returns`, and `tail-expressions`. `--parse-config <PATH>` prints the resolved rule configuration as JSON.

## Workspace and package selection

`--filter <SELECTOR>` is repeatable. A selector is either an exact Cargo package name or its normalized workspace-relative root; `.` denotes a root package. Selectors are not comma-split. Absolute, unmatched, and ambiguous selectors are rejected.

Walking honors `.gitignore` and `.rulewrightignore`, always excludes `.git` and Cargo's resolved target directory, and prunes nested Cargo projects that are not metadata members. Ordinary Rust and TOML files elsewhere in the workspace tree remain eligible for non-package-aware rules.

## Existing workspaces

Do not mechanically rewrite an established codebase just to make the initial count reach zero. First tune rules and path scopes so findings represent decisions the project actually wants to enforce. If useful existing debt remains, write a count-aware baseline with `rulewright --write-baseline rulewright-baseline.json`, check it in, and run `rulewright check --baseline rulewright-baseline.json` in the gate. Moving a finding to another line does not break the baseline, while an additional finding with the same rule, path, and message still fails.
