#!/usr/bin/env bash
set -euo pipefail

repository_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
temporary_dir="$(mktemp -d)"
llm_report="${temporary_dir}/rulewright-report.md"
default_config="${temporary_dir}/rulewright.toml"
generated_catalog="${temporary_dir}/RULES.md"

trap 'rm -rf "${temporary_dir}"' EXIT

(
    cd "${repository_root}"
    cargo run --quiet --locked -- --init --quiet --config "${default_config}"
    cargo run --quiet --locked -- --llm --config "${default_config}" >"${llm_report}"
)

if ! grep -Fxq '## Current violations' "${llm_report}"; then
    echo "rulewright report is missing the Current violations section" >&2
    exit 1
fi

awk '
    /^Configured rules for the target Rust workspace\./ {
        print "Built-in rules with the defaults written by `rulewright --init`. Runs beyond what clippy and rustfmt cover."
        next
    }
    /^## Current violations$/ { exit }
    /^## How to use findings$/ { skip = 1; next }
    skip && /^## / { skip = 0 }
    !skip { print }
' "${llm_report}" >"${generated_catalog}"
npx --yes prettier@3.6.2 --write "${generated_catalog}" --log-level silent
mv "${generated_catalog}" "${repository_root}/RULES.md"

echo "generated RULES.md"
