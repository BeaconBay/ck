#!/usr/bin/env bash
set -euo pipefail

# A successful binary build alone does not mean both registries were published.
# Keep this check in the executable path so a failed/skipped upstream job cannot
# accidentally turn a partially published release into the latest release.
for result_name in BUILD_RESULT CRATES_RESULT NPM_RESULT; do
  if [[ "${!result_name:-}" != success ]]; then
    echo "Refusing to finalize: $result_name=${!result_name:-missing}" >&2
    exit 1
  fi
done

gh release edit "${RELEASE_TAG:?missing release tag}" --draft=false \
  --repo="${GITHUB_REPOSITORY:?missing repository}"
