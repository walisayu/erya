#!/usr/bin/env bash

set -Eeuo pipefail

ERYA_ROOT_DIR="$(CDPATH='' cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
readonly ERYA_ROOT_DIR
readonly ERYA_DIST_DIR="${ERYA_ROOT_DIR}/dist"
readonly ERYA_CARGO_CMD="${CARGO:-cargo}"
readonly ERYA_RUSTC_CMD="${RUSTC:-rustc}"
readonly ERYA_BUILD_TOOL="${ERYA_BUILD_TOOL:-cargo}"

usage() {
  cat <<'EOF'
發布爾雅 Erya 的 release 執行檔。

用法：
  ./publish.sh [Rust target triple]

正式發布 target：
  x86_64-unknown-linux-gnu          Linux x86_64
  aarch64-unknown-linux-gnu         Linux ARM64／Raspberry Pi 64-bit
  armv7-unknown-linux-gnueabihf     Raspberry Pi 32-bit
  x86_64-apple-darwin               macOS Intel
  aarch64-apple-darwin              macOS Apple Silicon
  x86_64-pc-windows-msvc            Windows x64
  aarch64-pc-windows-msvc           Windows ARM64

建置器：
  ERYA_BUILD_TOOL=cargo              原生 Cargo（預設）
  ERYA_BUILD_TOOL=zigbuild           cargo-zigbuild
  ERYA_BUILD_TOOL=xwin               cargo-xwin

範例：
  ./publish.sh
  ./publish.sh aarch64-apple-darwin
  ERYA_BUILD_TOOL=zigbuild \
    ERYA_BUILD_TARGET=aarch64-unknown-linux-gnu.2.17 \
    ./publish.sh aarch64-unknown-linux-gnu
  ERYA_BUILD_TOOL=xwin ./publish.sh aarch64-pc-windows-msvc

未指定 target 時，使用目前 Rust 工具鏈的 host target。
ERYA_BUILD_TARGET 可指定傳給建置器的 target，同時保留正式封裝名稱。
EOF
}

fail() {
  printf 'publish: %s\n' "$1" >&2
  exit 1
}

if [[ "${1:-}" == "-h" || "${1:-}" == "--help" ]]; then
  usage
  exit 0
fi

if (( $# > 1 )); then
  usage >&2
  exit 2
fi

command -v "${ERYA_CARGO_CMD}" >/dev/null 2>&1 || fail "找不到 cargo"
command -v "${ERYA_RUSTC_CMD}" >/dev/null 2>&1 || fail "找不到 rustc"
command -v tar >/dev/null 2>&1 || fail "找不到 tar"

ERYA_TARGET="${1:-}"
if [[ -z "${ERYA_TARGET}" ]]; then
  ERYA_TARGET="$(${ERYA_RUSTC_CMD} -vV | awk '/^host:/ { print $2 }')"
fi
[[ -n "${ERYA_TARGET}" ]] || fail "無法判斷 Rust host target"
[[ "${ERYA_TARGET}" != *[!A-Za-z0-9._-]* ]] || fail "target 格式不正確：${ERYA_TARGET}"

ERYA_BUILD_TARGET="${ERYA_BUILD_TARGET:-${ERYA_TARGET}}"
[[ "${ERYA_BUILD_TARGET}" != *[!A-Za-z0-9._-]* ]] \
  || fail "build target 格式不正確：${ERYA_BUILD_TARGET}"

case "${ERYA_BUILD_TOOL}" in
  cargo)
    ERYA_BUILD_COMMAND=("${ERYA_CARGO_CMD}" build)
    ;;
  zigbuild)
    command -v cargo-zigbuild >/dev/null 2>&1 || fail "找不到 cargo-zigbuild"
    ERYA_BUILD_COMMAND=("${ERYA_CARGO_CMD}" zigbuild)
    ;;
  xwin)
    command -v cargo-xwin >/dev/null 2>&1 || fail "找不到 cargo-xwin"
    ERYA_BUILD_COMMAND=("${ERYA_CARGO_CMD}" xwin build)
    ;;
  *)
    fail "不支援的建置器：${ERYA_BUILD_TOOL}"
    ;;
esac

ERYA_VERSION="$(awk -F '"' '/^version = "/ { print $2; exit }' "${ERYA_ROOT_DIR}/Cargo.toml")"
[[ -n "${ERYA_VERSION}" ]] || fail "無法從 Cargo.toml 讀取版本"

ERYA_BINARY_NAME="erya"
if [[ "${ERYA_TARGET}" == *windows* ]]; then
  ERYA_BINARY_NAME="erya.exe"
fi

readonly ERYA_TARGET ERYA_BUILD_TARGET ERYA_VERSION ERYA_BINARY_NAME
readonly ERYA_PACKAGE_NAME="erya-${ERYA_VERSION}-${ERYA_TARGET}"
readonly ERYA_ARCHIVE_NAME="${ERYA_PACKAGE_NAME}.tar.gz"
readonly ERYA_ARCHIVE_PATH="${ERYA_DIST_DIR}/${ERYA_ARCHIVE_NAME}"
readonly ERYA_BUILD_BINARY="${CARGO_TARGET_DIR:-${ERYA_ROOT_DIR}/target}/${ERYA_TARGET}/release/${ERYA_BINARY_NAME}"

for ERYA_REQUIRED_FILE in README.md README.zh-TW.md README.ja.md LICENSE CHANGELOG.md docs/BUILD_ENVIRONMENT.md; do
  [[ -f "${ERYA_ROOT_DIR}/${ERYA_REQUIRED_FILE}" ]] \
    || fail "缺少發布檔案：${ERYA_REQUIRED_FILE}"
done

printf '建置爾雅 Erya %s（%s，%s）\n' \
  "${ERYA_VERSION}" "${ERYA_TARGET}" "${ERYA_BUILD_TOOL}"
"${ERYA_BUILD_COMMAND[@]}" \
  --manifest-path "${ERYA_ROOT_DIR}/Cargo.toml" \
  --locked \
  --release \
  --target "${ERYA_BUILD_TARGET}"

[[ -f "${ERYA_BUILD_BINARY}" ]] || fail "找不到建置產物：${ERYA_BUILD_BINARY}"

mkdir -p "${ERYA_DIST_DIR}"
ERYA_STAGING_DIR="$(mktemp -d "${ERYA_DIST_DIR}/.erya-publish.XXXXXX")"
readonly ERYA_STAGING_DIR
readonly ERYA_PACKAGE_DIR="${ERYA_STAGING_DIR}/${ERYA_PACKAGE_NAME}"

cleanup() {
  rm -rf -- "${ERYA_STAGING_DIR}"
}
trap cleanup EXIT

mkdir -p "${ERYA_PACKAGE_DIR}/docs"
cp "${ERYA_BUILD_BINARY}" "${ERYA_PACKAGE_DIR}/${ERYA_BINARY_NAME}"
cp "${ERYA_ROOT_DIR}/README.md" "${ERYA_PACKAGE_DIR}/README.md"
cp "${ERYA_ROOT_DIR}/README.zh-TW.md" "${ERYA_PACKAGE_DIR}/README.zh-TW.md"
cp "${ERYA_ROOT_DIR}/README.ja.md" "${ERYA_PACKAGE_DIR}/README.ja.md"
cp "${ERYA_ROOT_DIR}/CHANGELOG.md" "${ERYA_PACKAGE_DIR}/CHANGELOG.md"
cp "${ERYA_ROOT_DIR}/CHANGELOG.md" "${ERYA_PACKAGE_DIR}/RELEASE_NOTES.md"
cp "${ERYA_ROOT_DIR}/LICENSE" "${ERYA_PACKAGE_DIR}/LICENSE"
cp "${ERYA_ROOT_DIR}/docs/BUILD_ENVIRONMENT.md" \
  "${ERYA_PACKAGE_DIR}/docs/BUILD_ENVIRONMENT.md"
cp -R "${ERYA_ROOT_DIR}/locales" "${ERYA_PACKAGE_DIR}/locales"
cp -R "${ERYA_ROOT_DIR}/keymap" "${ERYA_PACKAGE_DIR}/keymap"
if [[ "${ERYA_TARGET}" != *windows* ]]; then
  chmod 755 "${ERYA_PACKAGE_DIR}/${ERYA_BINARY_NAME}"
fi

COPYFILE_DISABLE=1 tar -C "${ERYA_STAGING_DIR}" -czf "${ERYA_STAGING_DIR}/${ERYA_ARCHIVE_NAME}" "${ERYA_PACKAGE_NAME}"
mv -f "${ERYA_STAGING_DIR}/${ERYA_ARCHIVE_NAME}" "${ERYA_ARCHIVE_PATH}"

if command -v sha256sum >/dev/null 2>&1; then
  (
    cd "${ERYA_DIST_DIR}"
    sha256sum "${ERYA_ARCHIVE_NAME}" > "${ERYA_ARCHIVE_NAME}.sha256"
  )
elif command -v shasum >/dev/null 2>&1; then
  (
    cd "${ERYA_DIST_DIR}"
    shasum -a 256 "${ERYA_ARCHIVE_NAME}" > "${ERYA_ARCHIVE_NAME}.sha256"
  )
else
  printf '警告：找不到 sha256sum 或 shasum，略過 checksum\n' >&2
fi

printf '發布完成：\n'
printf '  壓縮檔：%s\n' "${ERYA_ARCHIVE_PATH}"
