# Agent instructions

- Treat Rulewright as generic standalone Rust tooling; do not add organization- or application-specific policy to built-ins.
- Use `rg` for discovery and `apply_patch` for authored edits.
- Preserve contextual filesystem failures, deterministic ordering, cache correctness, atomic fixes, and strict configuration validation.
- Add focused tests for behavior changes and run the gates in `CONTRIBUTING.md`.
- Keep public rule-pack APIs documented and validate `examples/custom-rule-pack` after registry changes.
- Do not publish, tag, push, or alter remote state unless the user explicitly requests it.
