#!/usr/bin/env bash

set -Eeuo pipefail

ERYA_ROOT_DIR="$(CDPATH='' cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
readonly ERYA_ROOT_DIR
readonly ERYA_BUILD_RUST="1.97.1"
readonly ERYA_MSRV="1.85.0"
readonly ERYA_ZIG_VERSION="0.15.2"
readonly ERYA_ZIGBUILD_VERSION="0.23.0"
readonly ERYA_XWIN_VERSION="0.23.1"
readonly ERYA_XWIN_CHANNEL="17"
readonly ERYA_GLIBC_VERSION="2.17"

fail() {
  printf 'build-release: %s\n' "$1" >&2
  exit 1
}

if (( $# != 0 )); then
  printf '用法：./build-release.sh\n' >&2
  exit 2
fi

for ERYA_COMMAND in cargo cargo-zigbuild cargo-xwin file rustc rustup shasum tar zig; do
  command -v "${ERYA_COMMAND}" >/dev/null 2>&1 || fail "找不到 ${ERYA_COMMAND}"
done

export RUSTUP_TOOLCHAIN="${ERYA_BUILD_RUST}"
ERYA_RUST_BIN_DIR="$(dirname -- "$(rustup which --toolchain "${ERYA_BUILD_RUST}" rustc)")"
export PATH="${ERYA_RUST_BIN_DIR}:${PATH}"
export CARGO_ZIGBUILD_ZIG_PATH="$(command -v zig)"
export XWIN_VERSION="${ERYA_XWIN_CHANNEL}"
export LC_ALL=C
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-${ERYA_ROOT_DIR}/target/release-build}"
export RUSTFLAGS="${RUSTFLAGS:-} --remap-path-prefix=${HOME}=build --remap-path-prefix=${ERYA_ROOT_DIR}=erya"

[[ "$(rustc --version)" == "rustc ${ERYA_BUILD_RUST} "* ]] \
  || fail "release toolchain 必須是 Rust ${ERYA_BUILD_RUST}"
[[ "$(rustup run "${ERYA_MSRV}" rustc --version)" == "rustc ${ERYA_MSRV} "* ]] \
  || fail "找不到 Rust ${ERYA_MSRV} MSRV toolchain"
[[ "$(zig version)" == "${ERYA_ZIG_VERSION}" ]] \
  || fail "Zig 必須是 ${ERYA_ZIG_VERSION}"

[[ "$(cargo-zigbuild --version)" == *"${ERYA_ZIGBUILD_VERSION}" ]] \
  || fail "cargo-zigbuild 必須是 ${ERYA_ZIGBUILD_VERSION}"
[[ "$(cargo-xwin --version)" == *"${ERYA_XWIN_VERSION}" ]] \
  || fail "cargo-xwin 必須是 ${ERYA_XWIN_VERSION}"

cd "${ERYA_ROOT_DIR}"
printf '執行本機品質關卡…\n'
cargo fmt --all -- --check
cargo clippy --all-targets --all-features --locked -- -D warnings
cargo test --all-targets --locked --no-fail-fast
rustup run "${ERYA_MSRV}" cargo check --all-targets --locked
cargo package --locked

printf '建置 macOS Apple Silicon…\n'
MACOSX_DEPLOYMENT_TARGET=11.0 ERYA_BUILD_TOOL=cargo \
  ./publish.sh aarch64-apple-darwin

printf '建置 macOS Intel…\n'
MACOSX_DEPLOYMENT_TARGET=10.15 ERYA_BUILD_TOOL=cargo \
  ./publish.sh x86_64-apple-darwin

for ERYA_TARGET in \
  x86_64-unknown-linux-gnu \
  aarch64-unknown-linux-gnu \
  armv7-unknown-linux-gnueabihf; do
  printf '以 Zig 建置 %s（glibc %s）…\n' \
    "${ERYA_TARGET}" "${ERYA_GLIBC_VERSION}"
  ERYA_BUILD_TOOL=zigbuild \
  ERYA_BUILD_TARGET="${ERYA_TARGET}.${ERYA_GLIBC_VERSION}" \
    ./publish.sh "${ERYA_TARGET}"
done

for ERYA_TARGET in \
  x86_64-pc-windows-msvc \
  aarch64-pc-windows-msvc; do
  printf '以 cargo-xwin 建置 %s…\n' "${ERYA_TARGET}"
  ERYA_BUILD_TOOL=xwin ./publish.sh "${ERYA_TARGET}"
done

ERYA_VERSION="$(awk -F '"' '/^version = "/ { print $2; exit }' Cargo.toml)"
[[ -n "${ERYA_VERSION}" ]] || fail "無法從 Cargo.toml 讀取版本"
readonly ERYA_VERSION

shopt -s nullglob
ERYA_ARCHIVES=("dist/erya-${ERYA_VERSION}-"*.tar.gz)
ERYA_CHECKSUMS=("dist/erya-${ERYA_VERSION}-"*.tar.gz.sha256)
(( ${#ERYA_ARCHIVES[@]} == 7 )) \
  || fail "預期 7 個 release archives，實際為 ${#ERYA_ARCHIVES[@]}"
(( ${#ERYA_CHECKSUMS[@]} == 7 )) \
  || fail "預期 7 個 checksum files，實際為 ${#ERYA_CHECKSUMS[@]}"

(
  cd dist
  shasum -a 256 "erya-${ERYA_VERSION}-"*.tar.gz | sort -k2 > SHA256SUMS
  for ERYA_CHECKSUM in "erya-${ERYA_VERSION}-"*.tar.gz.sha256; do
    shasum -a 256 -c "${ERYA_CHECKSUM}"
  done
  shasum -a 256 -c SHA256SUMS
)

ERYA_EXPECTED_VERSION="erya ${ERYA_VERSION}"
ERYA_ACTUAL_VERSION="$("${CARGO_TARGET_DIR}/aarch64-apple-darwin/release/erya" --version)"
[[ "${ERYA_ACTUAL_VERSION}" == "${ERYA_EXPECTED_VERSION}" ]] \
  || fail "Apple Silicon smoke test 預期 ${ERYA_EXPECTED_VERSION}，實際為 ${ERYA_ACTUAL_VERSION}"

for ERYA_TARGET in \
  aarch64-apple-darwin \
  x86_64-apple-darwin \
  x86_64-unknown-linux-gnu \
  aarch64-unknown-linux-gnu \
  armv7-unknown-linux-gnueabihf \
  x86_64-pc-windows-msvc \
  aarch64-pc-windows-msvc; do
  ERYA_BINARY="${CARGO_TARGET_DIR}/${ERYA_TARGET}/release/erya"
  [[ "${ERYA_TARGET}" == *windows* ]] && ERYA_BINARY="${ERYA_BINARY}.exe"
  [[ -f "${ERYA_BINARY}" ]] || fail "缺少建置產物：${ERYA_BINARY}"
  file "${ERYA_BINARY}"
done

printf '七平台本機建置完成：%s/dist\n' "${ERYA_ROOT_DIR}"
