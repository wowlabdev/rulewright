# 0.2.0 compatibility check

Recorded 2026-10-23 on the local core-sidecar tree at commit
`c194ff1385698958f862b70888087f8268c3fa0c`. The configuration and tracked
sources were unchanged. Existing untracked entries were `drv-d.pid`,
`playwright`, and `supabase/`; neither run modified the source tree.

The SHA-256 of `rulewright.toml` was
`bb76275984bf97228d3b8ef8578796d9de85b64c3b4204c87eb6cbd3c2ec5011`.

Built the 0.1.2 binary from `git archive v0.1.2` in a temporary directory and
the 0.2.0 binary from the reviewed working tree. Ran both standalone binaries
against the same `/home/dev/pallet/core-sidecar` directory, with arguments:

```text
--workspace-root /home/dev/pallet/core-sidecar
--config /home/dev/pallet/core-sidecar/rulewright.toml
--format json
```

Both exited **0**, with byte-identical output:

```json
{
  "schema_version": 2,
  "findings": []
}
```

Both warned about the same three pack-only IDs: `rust_forbidden_deps`,
`rust_foreign_engine_reference`, and `rust_raw_rng`. This comparison covers
the standalone built-ins, not the sidecar wrapper's custom rules. Strict
mode on this config requires that custom registry in either version.

The CLI regression `strict_accepts_absent_default_off_rules_but_still_checks_their_params`
separately verifies strict-mode compatibility when optional rules are absent,
while continuing to reject invalid parameters on explicitly configured rules.
