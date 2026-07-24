#!/usr/bin/env bash
set -Eeuo pipefail

tag="${1:-${GITHUB_REF_NAME:-}}"
if [[ ! "${tag}" =~ ^v[0-9]+\.[0-9]+\.[0-9]+([.-][0-9A-Za-z.-]+)?$ ]]; then
  echo "Release tag must look like v1.2.3 or v1.2.3-rc.1: ${tag}" >&2
  exit 1
fi

test -f package-lock.json
test -f src-tauri/Cargo.lock

package_version="$(node -p "require('./package.json').version")"
tauri_version="$(node -e "const fs=require('fs'); const c=JSON.parse(fs.readFileSync('src-tauri/tauri.conf.json','utf8')); process.stdout.write(c.version)")"
cargo_version="$(
  awk '
    /^\[package\]$/ { in_package = 1; next }
    /^\[/ { in_package = 0 }
    in_package && /^version[[:space:]]*=/ {
      value = $0
      sub(/^[^"]*"/, "", value)
      sub(/".*$/, "", value)
      print value
      exit
    }
  ' src-tauri/Cargo.toml
)"
tag_version="${tag#v}"

for candidate in "${package_version}" "${tauri_version}" "${cargo_version}"; do
  if [[ "${candidate}" != "${tag_version}" ]]; then
    echo "Version mismatch: tag=${tag_version}, package=${package_version}, tauri=${tauri_version}, cargo=${cargo_version}" >&2
    exit 1
  fi
done

echo "Validated release version ${tag_version}"
