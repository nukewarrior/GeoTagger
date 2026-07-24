#!/usr/bin/env bash
set -Eeuo pipefail

output_dir="${1:?output directory is required}"
target="${2:?Rust target is required}"
version="${3:?application version is required}"
mode="${4:-smoke}"

bundle_root="src-tauri/target/${target}/release/bundle"
app_path="$(find "${bundle_root}/macos" -maxdepth 1 -type d -name '*.app' -print -quit)"
dmg_path="$(find "${bundle_root}/dmg" -maxdepth 1 -type f -name '*.dmg' -print -quit)"

test -n "${app_path}"
test -n "${dmg_path}"
test -d "${app_path}"
test -f "${dmg_path}"

bundled_exiftool="${app_path}/Contents/Resources/exiftool/exiftool"
test -x "${bundled_exiftool}"
test -f "${app_path}/Contents/Resources/exiftool/lib/Image/ExifTool.pm"
test -f "${app_path}/Contents/Resources/exiftool/README"

actual_version="$("${bundled_exiftool}" -ver)"
if [[ "${actual_version}" != "13.59" ]]; then
  echo "Bundled ExifTool version mismatch: ${actual_version}" >&2
  exit 1
fi

codesign --verify --deep --strict --verbose=2 "${app_path}"

if [[ "${mode}" == "release" ]]; then
  spctl --assess --type execute --verbose=4 "${app_path}"
  xcrun stapler validate "${app_path}"
  xcrun stapler validate "${dmg_path}"
elif [[ "${mode}" != "smoke" ]]; then
  echo "Unknown artifact collection mode: ${mode}" >&2
  exit 1
fi

mkdir -p "${output_dir}"
output_abs="$(cd "${output_dir}" && pwd)"
dmg_name="GeoTagger-${version}-macos-aarch64.dmg"
app_archive_name="GeoTagger-${version}-macos-aarch64.app.tar.gz"

cp "${dmg_path}" "${output_abs}/${dmg_name}"
COPYFILE_DISABLE=1 tar \
  -C "$(dirname "${app_path}")" \
  -czf "${output_abs}/${app_archive_name}" \
  "$(basename "${app_path}")"

(
  cd "${output_abs}"
  : > SHA256SUMS
  for file in *.dmg *.tar.gz; do
    if [[ -f "${file}" ]]; then
      shasum -a 256 "${file}" >> SHA256SUMS
    fi
  done
)

echo "Collected macOS artifacts in ${output_abs}"
