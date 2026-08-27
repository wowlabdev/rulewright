#!/usr/bin/env bash
set -euo pipefail

repository_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ci_workflow="${repository_root}/.github/workflows/ci.yml"
release_workflow="${repository_root}/.github/workflows/release.yml"

fail() {
    echo "release check failed: $1" >&2
    exit 1
}

if grep -Eq 'uses: (actions/checkout|actions/cache)@v[0-9]' "${ci_workflow}" "${release_workflow}"; then
    fail "GitHub Actions must be pinned to commit SHAs"
fi

grep -Fq 'id-token: write' "${release_workflow}" || fail "release workflow lacks OIDC permission"
grep -Fq 'rust-lang/crates-io-auth-action@c6f97d4' "${release_workflow}" || fail "crates.io authentication action is not pinned"
grep -Fq "CARGO_REGISTRY_TOKEN: \${{ steps.crates-io-auth.outputs.token }}" "${release_workflow}" || fail "trusted-publishing token is not scoped to the publish environment"

for workflow in "${ci_workflow}" "${release_workflow}"; do
    grep -Fq 'examples/custom-rule-pack/Cargo.toml --locked' "${workflow}" || fail "downstream fixture is not locked in ${workflow}"
    grep -Fq 'Test extracted package' "${workflow}" || fail "extracted package is not tested in ${workflow}"
done

grep -Fq '"/scripts/**"' "${repository_root}/Cargo.toml" || fail "release scripts are absent from the crate payload"
grep -Fq '"/skills/**"' "${repository_root}/Cargo.toml" || fail "setup skill is absent from the crate payload"

fixture="$(mktemp -d)"
trap 'rm -rf "${fixture}"' EXIT

git -C "${fixture}" init --initial-branch main >/dev/null
git -C "${fixture}" config user.name "Rulewright Test"
git -C "${fixture}" config user.email "rulewright@example.invalid"
git -C "${fixture}" commit --allow-empty --quiet -m "main commit"
git -C "${fixture}" tag v1.0.0
git -C "${fixture}" switch --quiet --create release-side
git -C "${fixture}" commit --allow-empty --quiet -m "off-main commit"
git -C "${fixture}" tag v2.0.0
git -C "${fixture}" switch --quiet main

if ! (
    cd "${fixture}"
    bash "${repository_root}/scripts/verify-release-commit.sh" . main v1.0.0
) >/dev/null 2>&1; then
    fail "main release tag was rejected"
fi

if (
    cd "${fixture}"
    bash "${repository_root}/scripts/verify-release-commit.sh" . main v2.0.0
) >/dev/null 2>&1; then
    fail "off-main release tag was accepted"
fi
