# Rulewright

Rulewright turns repository engineering standards into executable rules for Rust workspaces. It runs alongside rustfmt and Clippy and covers the project-level decisions they cannot know about: API shape, module structure, documentation, Cargo policy, cross-file patterns, suppressions, and verified autofixes.

The built-in catalog covers generic Rust and Cargo/TOML policy. Browse the generated [rule catalog](RULES.md), the [Rust rules](src/languages/rust/rules), or the [Cargo/TOML rules](src/languages/toml/rules). Rulewright is an independent project and is not affiliated with the Rust Project, Cargo, Clippy, or rust-analyzer.

## Why it is strict

Rulewright is opinionated on purpose. It can be extremely pedantic, but the goal is coherent, internally consistent code rather than the smallest possible number of warnings. This is especially useful in repositories where AI agents write or revise a meaningful amount of code: conventions in a prompt are easy to forget, while executable rules give humans and agents the same feedback.

Almost none of the ideas are new. Most start with guidance from [The Rust Programming Language](https://doc.rust-lang.org/book/), standard library conventions, and lessons already documented across the Rust ecosystem. Rulewright combines that guidance with the coding style used in our own organization and turns it into something a repository can enforce.

That does not make code passing Rulewright universally better, and our preferences are not right for every team. Disable rules you disagree with, change thresholds, add ignores, and keep the parts that make sense for your project. If you want to see the result of the default setup, look through this repository; Rulewright runs against itself.

## Getting started

Install Rulewright from crates.io:

```console
cargo install rulewright --locked
```

From a source checkout, use `cargo install --path . --locked` instead.

Then run these commands anywhere inside a Cargo root package or workspace:

```console
rulewright --init
rulewright --strict
```

`--init` creates a complete `rulewright.toml` without replacing an existing file. Rulewright finds the workspace through Cargo metadata, honors `.gitignore` and `.rulewrightignore`, and works with root packages, virtual workspaces, and nested members. The [configuration guide](docs/configuration.md) covers explicit roots, custom configuration paths, package filters, and ignore behavior.

## Everyday use

```console
rulewright --list
rulewright --detail rust_panic
rulewright --rule rust_panic --dirty
rulewright --filter my-package --strict
rulewright --fix --dry-run
rulewright --fix
rulewright --suppressions
rulewright clean --dry-run
rulewright --llm > rulewright-report.md
rulewright --ci --strict
```

`--dirty` limits source analysis to Git changes while retaining the workspace context needed by cross-file rules. `--fix --dry-run` previews safe fixes. `--ci` runs Rulewright, rustfmt, and Clippy as one local gate. Run `rulewright --help` for the complete CLI.

## Configuration and directives

Every registered rule has an explicit entry in `rulewright.toml`. Entries control enablement, path ignores, and typed parameters, while `--strict` rejects missing or unknown entries. The generated file is a starting point, not a demand that every repository use our exact policy.

Intentional findings can be suppressed, but the reason is part of the directive:

```rust
// #rw(rust_panic) process boundary converts this panic into an exit status
panic!("unreachable state");
```

The [directives guide](docs/directives.md) covers file, block, function, multi-rule, and wildcard scopes as well as the `#rw:aligned` and `#rw:sorted` layout markers. `rulewright clean --dry-run` previews stale suppression targets before changing anything.

## AI agents

`rulewright --llm` emits a Markdown reference for the resolved configuration, including rule metadata, examples, suppression syntax, alignment guidance, and current findings. Use repeated `--rule` options when an agent only needs the rules relevant to its task.

The repository also ships a [`rulewright-setup` skill](skills/rulewright-setup) that installs Rulewright into an existing Rust workspace, creates its configuration, and can wire it into CI. It works with Codex, Claude Code, and other Agent Skills-compatible tools.

Rules make decisions visible and enforceable; they do not decide whether those decisions were good in the first place. Humans and agents still need to understand the code they are changing.

## Autofix

Most rules deliberately have no automatic fix. They explain what was found, why it matters, and show useful examples so a person or AI agent can make the right change with the surrounding code in mind. Autofixes are reserved for mechanical rewrites that Rulewright can verify without guessing.

Use `rulewright --fix --dry-run` to preview changes. When you apply them, Rulewright makes the safe edits, reruns the rules until nothing else changes, and checks the complete workspace again before reporting success. The [architecture guide](docs/architecture.md) explains how that works internally.

## Custom rule packs

Projects can combine the generic built-ins with application-specific line, AST, TOML, or workspace rules through the public `RuleRegistry` API. There is no dynamic plugin ABI. See the [rule-pack guide](docs/rule-packs.md) and the runnable [`custom-rule-pack` example](examples/custom-rule-pack).

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) to get started and [SECURITY.md](SECURITY.md) for vulnerability reports.

## Built on

Rulewright relies on a lot of excellent open-source work. [rust-analyzer's syntax crates](https://github.com/rust-lang/rust-analyzer) provide the Rust syntax tree, [Taplo](https://github.com/tamasfe/taplo) parses TOML, [Cargo Metadata](https://github.com/oli-obk/cargo_metadata) describes Cargo workspaces, and [ignore](https://github.com/BurntSushi/ripgrep/tree/master/crates/ignore) handles file discovery and ignore rules. [Rayon](https://github.com/rayon-rs/rayon) runs checks in parallel, [inventory](https://github.com/dtolnay/inventory) collects registered rules, and [Clap](https://github.com/clap-rs/clap) powers the CLI. These projects do a lot of the heavy lifting and deserve the credit.

## License

[MIT](LICENSE)
