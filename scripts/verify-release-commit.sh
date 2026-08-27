#!/usr/bin/env bash
set -euo pipefail

remote="${1:-origin}"
branch="${2:-main}"
tag="${3:-${GITHUB_REF_NAME:-}}"

if [[ -z "${tag}" ]]; then
    echo "release tag is required" >&2
    exit 1
fi

git fetch --no-tags "${remote}" "${branch}"
tag_commit="$(git rev-parse "${tag}^{commit}")"

if ! git merge-base --is-ancestor "${tag_commit}" FETCH_HEAD; then
    echo "tag ${tag} does not point to a commit on ${remote}/${branch}" >&2
    exit 1
fi
