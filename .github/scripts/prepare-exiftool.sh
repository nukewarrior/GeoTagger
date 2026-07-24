#!/usr/bin/env bash
set -Eeuo pipefail

readonly EXIFTOOL_VERSION="13.59"
readonly EXIFTOOL_SHA256="668ea3acececb7235fbd0f4900e72d5f12c9b07e5c778fd36cb1e9b5828fd65a"
readonly EXIFTOOL_URL="https://sourceforge.net/projects/exiftool/files/Image-ExifTool-${EXIFTOOL_VERSION}.tar.gz/download"

workspace="${GITHUB_WORKSPACE:-$(pwd)}"
destination="${1:-${workspace}/src-tauri/resources/exiftool}"
fixture_path="${2:-${RUNNER_TEMP:-/tmp}/geotagger-exiftool-fixture.jpg}"

mkdir -p "$(dirname "${destination}")" "$(dirname "${fixture_path}")"
destination_parent="$(cd "$(dirname "${destination}")" && pwd)"
destination_abs="${destination_parent}/$(basename "${destination}")"
expected_abs="${workspace}/src-tauri/resources/exiftool"

if [[ "${destination_abs}" != "${expected_abs}" ]]; then
  echo "Refusing to replace unexpected ExifTool destination: ${destination_abs}" >&2
  exit 1
fi

work_dir="$(mktemp -d "${RUNNER_TEMP:-/tmp}/geotagger-exiftool.XXXXXX")"
trap 'rm -rf "${work_dir}"' EXIT

archive="${work_dir}/Image-ExifTool-${EXIFTOOL_VERSION}.tar.gz"
curl \
  --proto '=https' \
  --tlsv1.2 \
  --fail \
  --location \
  --retry 5 \
  --retry-all-errors \
  --output "${archive}" \
  "${EXIFTOOL_URL}"

printf '%s  %s\n' "${EXIFTOOL_SHA256}" "${archive}" | shasum -a 256 -c -
tar -xzf "${archive}" -C "${work_dir}"

source_dir="${work_dir}/Image-ExifTool-${EXIFTOOL_VERSION}"
test -f "${source_dir}/exiftool"
test -d "${source_dir}/lib"
test -f "${source_dir}/README"
test -f "${source_dir}/t/images/ExifTool.jpg"

rm -rf "${destination_abs}"
mkdir -p "${destination_abs}"
cp "${source_dir}/exiftool" "${destination_abs}/exiftool"
cp -R "${source_dir}/lib" "${destination_abs}/lib"
cp "${source_dir}/README" "${destination_abs}/README"
chmod 0755 "${destination_abs}/exiftool"
cp "${source_dir}/t/images/ExifTool.jpg" "${fixture_path}"

actual_version="$("${destination_abs}/exiftool" -ver)"
if [[ "${actual_version}" != "${EXIFTOOL_VERSION}" ]]; then
  echo "Expected ExifTool ${EXIFTOOL_VERSION}, got ${actual_version}" >&2
  exit 1
fi

echo "Prepared ExifTool ${actual_version} at ${destination_abs}"
