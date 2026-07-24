#!/usr/bin/env bash
set -Eeuo pipefail

artifact_dir="${1:?artifact directory is required}"
test -f "${artifact_dir}/SHA256SUMS"

(
  cd "${artifact_dir}"
  sha256sum -c SHA256SUMS
)
