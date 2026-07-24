#!/usr/bin/env bash
set -Eeuo pipefail

readonly EXPECTED_VERSION="13.59"
readonly EXPECTED_LATITUDE="31.2304"
readonly EXPECTED_LONGITUDE="121.4737"
readonly EXPECTED_ALTITUDE="12.3"

workspace="${GITHUB_WORKSPACE:-$(pwd)}"
exiftool_dir="${1:-${workspace}/src-tauri/resources/exiftool}"
fixture_path="${2:-${RUNNER_TEMP:-/tmp}/geotagger-exiftool-fixture.jpg}"
exiftool="${exiftool_dir}/exiftool"

test -x "${exiftool}"
test -f "${exiftool_dir}/lib/Image/ExifTool.pm"
test -f "${exiftool_dir}/README"
test -f "${fixture_path}"

actual_version="$("${exiftool}" -ver)"
if [[ "${actual_version}" != "${EXPECTED_VERSION}" ]]; then
  echo "Expected ExifTool ${EXPECTED_VERSION}, got ${actual_version}" >&2
  exit 1
fi

work_dir="$(mktemp -d "${RUNNER_TEMP:-/tmp}/geotagger-exiftool-smoke.XXXXXX")"
trap 'rm -rf "${work_dir}"' EXIT

source_photo="${work_dir}/源 照片.jpg"
output_photo="${work_dir}/输出 照片.jpg"
cp "${fixture_path}" "${source_photo}"
cp "${source_photo}" "${output_photo}"

source_hash_before="$(shasum -a 256 "${source_photo}" | awk '{print $1}')"

"${exiftool}" \
  -overwrite_original \
  -n \
  "-GPSLatitude=${EXPECTED_LATITUDE}" \
  -GPSLatitudeRef=N \
  "-GPSLongitude=${EXPECTED_LONGITUDE}" \
  -GPSLongitudeRef=E \
  "-GPSAltitude=${EXPECTED_ALTITUDE}" \
  "${output_photo}"

values_file="${work_dir}/gps-values.txt"
"${exiftool}" \
  -n \
  -s3 \
  -GPSLatitude \
  -GPSLongitude \
  -GPSAltitude \
  "${output_photo}" > "${values_file}"

latitude="$(sed -n '1p' "${values_file}")"
longitude="$(sed -n '2p' "${values_file}")"
altitude="$(sed -n '3p' "${values_file}")"

assert_close() {
  local actual="$1"
  local expected="$2"
  local tolerance="$3"
  awk -v actual="${actual}" -v expected="${expected}" -v tolerance="${tolerance}" '
    BEGIN {
      delta = actual - expected
      if (delta < 0) {
        delta = -delta
      }
      exit(delta <= tolerance ? 0 : 1)
    }
  '
}

assert_close "${latitude}" "${EXPECTED_LATITUDE}" "0.000001"
assert_close "${longitude}" "${EXPECTED_LONGITUDE}" "0.000001"
assert_close "${altitude}" "${EXPECTED_ALTITUDE}" "0.01"

source_hash_after="$(shasum -a 256 "${source_photo}" | awk '{print $1}')"
if [[ "${source_hash_before}" != "${source_hash_after}" ]]; then
  echo "ExifTool smoke test modified the source fixture" >&2
  exit 1
fi

echo "ExifTool ${actual_version} write/read smoke test passed"
