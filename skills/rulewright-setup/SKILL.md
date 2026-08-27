---
name: rulewright-setup
description: Install and configure Rulewright in an existing Rust project or Cargo workspace. Use when adding the Rulewright CLI, generating or validating rulewright.toml, integrating Rulewright into CI, or repairing an incomplete setup. Do not use for developing Rulewright itself or authoring custom rule packs.
license: MIT
---

# Rulewright Setup

Set up Rulewright without overwriting existing policy or disabling rules just to get a green run.

## What Rulewright is

Rulewright is a strict, configurable linter for Rust workspaces. It complements rustfmt and Clippy with checks for API design, module structure, documentation, Cargo policy, cross-file patterns, suppressions, and autofixes.

It is intentionally pedantic and mainly built to keep human- and AI-written code coherent across a repository. Its defaults reflect one organization's coding style, not a claim that every Rust project should look the same. The consuming project decides which rules and thresholds become policy.

## Inspect the project

- Read the repository's agent instructions and existing contribution guidance first.
- Run `cargo metadata --no-deps --format-version 1` and use its `workspace_root`; do not infer the workspace from directory names.
- Inspect `rust-toolchain.toml`, `rulewright.toml`, `.rulewrightignore`, existing rustfmt and Clippy configuration, CI configuration, and the Git working tree before editing.
- Preserve existing configuration and unrelated changes. Never overwrite an existing `rulewright.toml` with generated output.

## Install the CLI

Check `rulewright --version` first. If the command is unavailable, install the published crate with:

```console
cargo install rulewright --locked
```

Rulewright needs Rust 1.95 or newer. If the project is pinned to an older compiler, use a separate supported toolchain such as `cargo +stable install rulewright --locked`; this installs the CLI without changing the project's compiler. Do not add Rulewright to the target project's runtime dependencies. When the user requests a particular version, install it with `--version '=X.Y.Z'`.

Run `rulewright --help` before changing the project. Use the installed CLI's command surface when it differs from this skill.

## Establish configuration

- If `rulewright.toml` is absent, run `rulewright --workspace-root <ROOT> --init` from the workspace root. `--init` refuses to overwrite an existing configuration.
- If it already exists, validate it rather than regenerating it.
- Add `/.rulewright.lock` to the workspace-root `.gitignore`, creating the file if necessary.
- Do not change rule enablement or thresholds unless the user asked to tune the policy. In particular, do not disable rules merely to make the first run green.
- Rulewright already honors `.gitignore` and Cargo's target directory. Create or extend `.rulewrightignore` only for generated, vendored, or external paths that the repository genuinely does not want analyzed.

Once `rulewright.toml` exists, use `rulewright --workspace-root <ROOT> --llm` as the agent-facing reference for the installed version and resolved configuration. It emits Markdown covering severity levels, suppression syntax, findings, and configured rule metadata; a complete catalog can be large, so redirect it to a temporary file and inspect the overview plus the rules relevant to the current findings. Repeating `--rule <RULE>` before `--llm` produces a focused reference when the complete catalog is unnecessary.

Run `rulewright --workspace-root <ROOT> --strict` and report the initial findings. `rulewright --fix --dry-run` may be used to preview safe fixes, but do not apply fixes or broad configuration changes unless the user's request includes that work.

## Integrate CI

Add Rulewright to the existing CI configuration only when requested. Pin the exact installed Rulewright release. If CI already runs rustfmt and Clippy, add only Rulewright:

```console
cargo install rulewright --version '=X.Y.Z' --locked
rulewright --strict
```

Use `rulewright --ci --strict` instead when Rulewright should also own the rustfmt and Clippy checks. Preserve the project's toolchain and cache setup. If its compiler is older than Rust 1.95, install Rulewright with a separate supported toolchain that is available in CI. Run the chosen command locally and report existing findings without weakening the configuration.

## Verify

- Run `rulewright --version`.
- Run Rulewright in strict mode and report its findings.
- Check edited CI configuration with the repository's normal workflow linter.
- Review `git diff --check` and the final diff.
