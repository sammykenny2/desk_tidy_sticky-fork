#!/usr/bin/env bash
#
# Script: test-deb-smoke.sh
# Description: Validate a built Desk Tidy Sticky .deb without installing it.
#
# Usage: ./scripts/linux/test-deb-smoke.sh [path/to/package.deb]
#
# Defaults to release/DeskTidySticky-<version>-<arch>.deb.
#

set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd -- "${SCRIPT_DIR}/../.." && pwd)"

cd "${PROJECT_ROOT}"
VERSION="$(node -p "require('./src-tauri/tauri.conf.json').version")"
ARCH="$(dpkg --print-architecture)"
DEB_PATH="${1:-release/DeskTidySticky-${VERSION}-${ARCH}.deb}"

pass() {
  printf '[pass] %s\n' "$1"
}

fail() {
  printf '[fail] %s\n' "$1" >&2
  exit 1
}

[[ -f "${DEB_PATH}" ]] || fail "package not found: ${DEB_PATH}"
printf '[smoke] Validating %s\n' "${DEB_PATH}"

field() {
  dpkg-deb --field "${DEB_PATH}" "$1"
}

[[ "$(field Version)" == "${VERSION}" ]] || fail "Version is $(field Version), expected ${VERSION}"
pass "version ${VERSION}"
[[ "$(field Architecture)" == "${ARCH}" ]] || fail "Architecture is $(field Architecture), expected ${ARCH}"
pass "architecture ${ARCH}"

depends="$(field Depends)"
for dep in libwebkit2gtk-4.1-0 libgtk-3-0 libgtk-layer-shell0 libayatana-appindicator3-1; do
  grep -q "${dep}" <<<"${depends}" || fail "Depends is missing ${dep}: ${depends}"
  pass "depends on ${dep}"
done

STAGE="$(mktemp -d)"
trap 'rm -rf "${STAGE}"' EXIT
dpkg-deb -x "${DEB_PATH}" "${STAGE}"

BIN="${STAGE}/usr/bin/desk_tidy_sticky"
[[ -x "${BIN}" ]] || fail 'missing /usr/bin/desk_tidy_sticky'
pass 'executable /usr/bin/desk_tidy_sticky'

DESKTOP_FILE="$(find "${STAGE}/usr/share/applications" -name '*.desktop' | head -n 1)"
[[ -n "${DESKTOP_FILE}" ]] || fail 'missing .desktop entry'
grep -q '^Exec=desk_tidy_sticky' "${DESKTOP_FILE}" || fail "unexpected Exec line in ${DESKTOP_FILE}"
grep -q '^Categories=.\+' "${DESKTOP_FILE}" || fail "empty Categories in ${DESKTOP_FILE}"
pass "desktop entry $(basename "${DESKTOP_FILE}")"

icon_count="$(find "${STAGE}/usr/share/icons/hicolor" -name 'desk_tidy_sticky.png' | wc -l)"
[[ "${icon_count}" -gt 0 ]] || fail 'missing hicolor icons'
pass "${icon_count} hicolor icons"

missing_libs="$(ldd "${BIN}" | grep 'not found' || true)"
[[ -z "${missing_libs}" ]] || fail "unresolved shared libraries:
${missing_libs}"
pass 'all shared libraries resolve on this host'

printf '[smoke] OK\n'
