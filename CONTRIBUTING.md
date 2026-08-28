# Contributing

Whether you are fixing a false positive, improving an error message, or arguing that one of the rules is a terrible idea, all of it helps. Rulewright is deliberately opinionated, but the implementation should still be fair, predictable, and useful outside the repository it came from.

You do not need to understand the whole rule catalog to contribute. Most changes live in one rule module and its tests. Start with the part you care about, run the focused test while you work, and run the complete gates before opening a pull request.

Rulewright supports Rust 1.95 and newer. Local development uses the current stable toolchain so rust-analyzer, rustfmt, and Clippy stay current; CI separately checks that the code still builds on 1.95.

## On AI

AI agents are a first-class use case here, but they are not an excuse for vague rules or generated-looking code. A good rule explains a real engineering decision, reports exactly what happened, and gives the same useful feedback to a person and an agent. Keep the reasoning in the rule metadata and the behavior in tests so neither has to guess.

## Before you submit

Before submitting a change, run:

```console
cargo fmt --all -- --check
cargo check --workspace --all-targets --all-features --locked
cargo test --workspace --all-targets --all-features --locked
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --all-features --locked
cargo run --locked -- --strict
cargo test --manifest-path examples/custom-rule-pack/Cargo.toml
```

## Authoring rules

- Choose a globally unique, stable ID with a language prefix such as `rust_` or `toml_`.
- State the detected condition separately from the engineering justification.
- Include focused pass and fail examples and direct unit tests for edge cases.
- Prefer syntax-tree analysis when text matching cannot distinguish code from comments or literals.
- Make repository policy configurable with neutral defaults, or keep it in a downstream rule pack.
- Return every applicable violation deterministically and preserve contextual I/O errors.
- For fixes, test exact output, overlaps, dry runs, and fixpoint behavior.

Built-ins register through the internal rule macros. Repository-specific policies belong in a separate pack using the public API demonstrated in [rule packs](docs/rule-packs.md).

When registry metadata changes, update the documentation, regenerate `RULES.md` with `bash scripts/generate-rules.sh`, and regenerate `rulewright.toml` with `rulewright --init`. The checked-in configuration is Rulewright's own policy, so restore its intentional enablement choices after generation instead of accepting every default blindly.
