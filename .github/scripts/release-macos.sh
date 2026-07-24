#!/usr/bin/env bash
set -Eeuo pipefail

output_dir="${1:?output directory is required}"
target="${2:?Rust target is required}"
version="${3:?application version is required}"
config_path="${4:-.github/tauri.ci.conf.json}"

required_secrets=(
  APPLE_CERTIFICATE
  APPLE_CERTIFICATE_PASSWORD
  APPLE_SIGNING_IDENTITY
  APPLE_API_ISSUER
  APPLE_API_KEY
  APPLE_API_PRIVATE_KEY_BASE64
)

for variable_name in "${required_secrets[@]}"; do
  if [[ -z "${!variable_name:-}" ]]; then
    echo "Required macOS release secret is missing: ${variable_name}" >&2
    exit 1
  fi
done

if [[ "${APPLE_SIGNING_IDENTITY}" != "Developer ID Application:"* ]]; then
  echo "APPLE_SIGNING_IDENTITY must be a Developer ID Application identity" >&2
  exit 1
fi

umask 077
signing_dir="$(mktemp -d "${RUNNER_TEMP:?RUNNER_TEMP is required}/geotagger-signing.XXXXXX")"
certificate_path="${signing_dir}/certificate.p12"
api_key_path="${signing_dir}/AuthKey_${APPLE_API_KEY}.p8"
keychain_path="${signing_dir}/build.keychain-db"
keychain_password="$(openssl rand -hex 32)"

cleanup() {
  security delete-keychain "${keychain_path}" >/dev/null 2>&1 || true
  rm -rf "${signing_dir}"
}
trap cleanup EXIT

printf '%s' "${APPLE_CERTIFICATE}" | openssl base64 -d -A > "${certificate_path}"
printf '%s' "${APPLE_API_PRIVATE_KEY_BASE64}" | openssl base64 -d -A > "${api_key_path}"
chmod 0600 "${certificate_path}" "${api_key_path}"

security create-keychain -p "${keychain_password}" "${keychain_path}"
security default-keychain -s "${keychain_path}"
security unlock-keychain -p "${keychain_password}" "${keychain_path}"
security set-keychain-settings -lut 21600 "${keychain_path}"
security import "${certificate_path}" \
  -k "${keychain_path}" \
  -P "${APPLE_CERTIFICATE_PASSWORD}" \
  -T /usr/bin/codesign
security set-key-partition-list \
  -S apple-tool:,apple:,codesign: \
  -s \
  -k "${keychain_password}" \
  "${keychain_path}"

if ! security find-identity -v -p codesigning "${keychain_path}" |
  grep -F -- "${APPLE_SIGNING_IDENTITY}" >/dev/null; then
  echo "Configured signing identity was not imported into the temporary keychain" >&2
  exit 1
fi

export APPLE_API_KEY_PATH="${api_key_path}"
unset APPLE_CERTIFICATE APPLE_CERTIFICATE_PASSWORD APPLE_API_PRIVATE_KEY_BASE64

npm run tauri -- bundle \
  --target "${target}" \
  --bundles app,dmg \
  --config "${config_path}"

bash .github/scripts/collect-macos-artifacts.sh \
  "${output_dir}" \
  "${target}" \
  "${version}" \
  release
