#!/usr/bin/env bash
#
# Script: build-deb.sh
# Description: Build the Desk Tidy Sticky .deb package for the host architecture
#              and copy it to release/.
#
# Usage: ./scripts/linux/build-deb.sh [--install-deps] [--skip-build] [--output-dir DIR]
#
# Options:
#   --install-deps     Install missing apt build dependencies with sudo before building.
#   --skip-build       Reuse the .deb already in src-tauri/target/release/bundle/deb.
#   --output-dir DIR   Copy the package to DIR instead of release/.
#
# Examples:
#   ./scripts/linux/build-deb.sh
#   ./scripts/linux/build-deb.sh --install-deps
#   make package-deb
#

set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd -- "${SCRIPT_DIR}/../.." && pwd)"
OUTPUT_DIR="${PROJECT_ROOT}/release"
INSTALL_DEPS=0
SKIP_BUILD=0

# Build-time packages. libwebkit2gtk-4.1-dev pulls in GTK 3, libsoup 3 and JavaScriptCore.
# libgtk-layer-shell-dev provides the wlroots layer-shell binding used by sticky windows.
APT_BUILD_DEPS=(
  build-essential
  pkg-config
  file
  libwebkit2gtk-4.1-dev
  libgtk-layer-shell-dev
  libayatana-appindicator3-dev
  librsvg2-dev
)

log() {
  printf '[build-deb] %s\n' "$1"
}

fail() {
  printf '[build-deb] error: %s\n' "$1" >&2
  exit 1
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --install-deps) INSTALL_DEPS=1 ;;
    --skip-build) SKIP_BUILD=1 ;;
    --output-dir)
      [[ $# -ge 2 ]] || fail '--output-dir needs a directory'
      OUTPUT_DIR="$2"
      shift
      ;;
    -h|--help)
      sed -n '2,18p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'
      exit 0
      ;;
    *) fail "unknown option: $1" ;;
  esac
  shift
done

[[ "$(uname -s)" == "Linux" ]] || fail 'the .deb package can only be built on Linux'
command -v dpkg-deb >/dev/null 2>&1 || fail 'dpkg-deb not found; build on a Debian-based distribution'

missing_deps=()
for pkg in "${APT_BUILD_DEPS[@]}"; do
  if ! dpkg-query -W -f='${Status}' "${pkg}" 2>/dev/null | grep -q 'install ok installed'; then
    missing_deps+=("${pkg}")
  fi
done
if [[ ${#missing_deps[@]} -gt 0 ]]; then
  if [[ ${INSTALL_DEPS} -eq 1 ]]; then
    log "Installing build dependencies: ${missing_deps[*]}"
    sudo apt-get update
    sudo DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends "${missing_deps[@]}"
  else
    fail "missing build dependencies: ${missing_deps[*]}
Install them with:
  sudo apt install ${missing_deps[*]}
or rerun with --install-deps."
  fi
fi

# A Rust toolchain installed by rustup is often not on PATH in non-login shells.
if ! command -v cargo >/dev/null 2>&1 && [[ -x "${HOME}/.cargo/bin/cargo" ]]; then
  export PATH="${HOME}/.cargo/bin:${PATH}"
fi
command -v cargo >/dev/null 2>&1 || fail 'cargo not found; install Rust from https://rustup.rs'
command -v pnpm >/dev/null 2>&1 || fail 'pnpm not found; install Node.js and run "corepack enable"'

cd "${PROJECT_ROOT}"
VERSION="$(node -p "require('./src-tauri/tauri.conf.json').version")"
ARCH="$(dpkg --print-architecture)"
BUNDLE_DIR="${PROJECT_ROOT}/src-tauri/target/release/bundle/deb"

if [[ ${SKIP_BUILD} -eq 0 ]]; then
  log 'Installing Node dependencies'
  pnpm install --frozen-lockfile
  log "Building Desk Tidy Sticky ${VERSION} (${ARCH}) .deb"
  # Only the deb bundle: "targets": "all" would also try AppImage and rpm.
  pnpm tauri build --bundles deb
fi

shopt -s nullglob
debs=("${BUNDLE_DIR}"/*_"${VERSION}"_"${ARCH}".deb)
shopt -u nullglob
[[ ${#debs[@]} -eq 1 ]] || fail "expected one ${VERSION}/${ARCH} .deb in ${BUNDLE_DIR}, found ${#debs[@]}"

mkdir -p "${OUTPUT_DIR}"
OUTPUT_PATH="${OUTPUT_DIR}/DeskTidySticky-${VERSION}-${ARCH}.deb"
cp -f "${debs[0]}" "${OUTPUT_PATH}"

log "Package: ${OUTPUT_PATH}"
dpkg-deb --field "${OUTPUT_PATH}" Package Version Architecture Depends
log "Install with: sudo apt install ${OUTPUT_PATH}"
