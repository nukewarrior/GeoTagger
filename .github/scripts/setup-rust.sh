#!/usr/bin/env bash
set -Eeuo pipefail

toolchain="${RUST_TOOLCHAIN_VERSION:-1.97.1}"

rustup set profile minimal
rustup toolchain install "${toolchain}" --component clippy --component rustfmt
rustup default "${toolchain}"

if [[ "$#" -gt 0 ]]; then
  rustup target add --toolchain "${toolchain}" "$@"
fi

rustc --version
cargo --version
rustfmt --version
cargo clippy --version
