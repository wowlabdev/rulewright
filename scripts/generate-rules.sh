#!/usr/bin/env bash
set -euo pipefail

repository_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
temporary_dir="$(mktemp -d)"
llm_report="${temporary_dir}/rulewright-report.md"
generated_catalog="${temporary_dir}/RULES.md"

trap 'rm -rf "${temporary_dir}"' EXIT

(
    cd "${repository_root}"
    cargo run --quiet --locked -- --llm >"${llm_report}"
)

if ! grep -Fxq '## Current violations' "${llm_report}"; then
    echo "rulewright report is missing the Current violations section" >&2
    exit 1
fi

awk '
    /^## Current violations$/ { exit }
    /^## How to use findings$/ { skip = 1; next }
    skip && /^## / { skip = 0 }
    !skip { print }
' "${llm_report}" >"${generated_catalog}"
npx --yes prettier@3.6.2 --write "${generated_catalog}" --log-level silent
mv "${generated_catalog}" "${repository_root}/RULES.md"

echo "generated RULES.md"
