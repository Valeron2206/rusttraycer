#!/usr/bin/env bash
# Pack Linux amd64 AppImage + .deb from already-built binaries.
# Usage: pack-linux.sh [--dry-run] <version> <dest-dir> <bin-dir>
#        PACK_DRY_RUN=1 pack-linux.sh <version> <dest-dir> <bin-dir>
# --dry-run / PACK_DRY_RUN=1: parse args, check binaries via need(), print
# what would be packed, exit 0 without mkdir dest, dpkg-deb, downloads, or artifacts.
set -eu
DRY_RUN=0
if [ "${1:-}" = "--dry-run" ]; then
  DRY_RUN=1
  shift
fi
if [ "${PACK_DRY_RUN:-}" = "1" ]; then
  DRY_RUN=1
fi
VERSION="${1:?version}"
DEST="${2:?dest dir}"
BIN="${3:?bin dir}"
VERSION="${VERSION#v}"
TAG="v${VERSION}"
NAME="rusttraycer-${TAG}-linux-x86_64"
DEB_NAME="rusttraycer_${VERSION}_amd64.deb"
APPIMAGE_NAME="rusttraycer-${TAG}-linux-x86_64.AppImage"

need() {
  local p="${BIN}/$1"
  if [ ! -f "$p" ] || [ ! -x "$p" ]; then
    echo "missing executable: $p" >&2
    exit 1
  fi
}
need rt-host
need rt-cli
need rt-gui

if [ "${DRY_RUN}" = "1" ]; then
  echo "dry-run: would pack ${DEB_NAME}"
  echo "dry-run: would pack ${APPIMAGE_NAME}"
  echo "dry-run: dest=${DEST}"
  echo "dry-run: bin=${BIN}"
  exit 0
fi

mkdir -p "${DEST}"
DEST="$(cd "${DEST}" && pwd)"
BIN="$(cd "${BIN}" && pwd)"
WORKDIR="$(mktemp -d)"
trap 'rm -rf "${WORKDIR}"' EXIT

DEB_ROOT="${WORKDIR}/deb"
mkdir -p "${DEB_ROOT}/DEBIAN" "${DEB_ROOT}/usr/bin" "${DEB_ROOT}/usr/share/applications"
install -m 0755 "${BIN}/rt-host" "${BIN}/rt-cli" "${BIN}/rt-gui" "${DEB_ROOT}/usr/bin/"
cat > "${DEB_ROOT}/usr/share/applications/rusttraycer.desktop" << DESK
[Desktop Entry]
Type=Application
Name=RustTraycer
Exec=rt-gui
Terminal=false
Categories=Development;
DESK
cat > "${DEB_ROOT}/DEBIAN/control" << CTL
Package: rusttraycer
Version: ${VERSION}
Section: devel
Priority: optional
Architecture: amd64
Maintainer: RustTraycer <noreply@localhost>
Description: Local-first RustTraycer desktop (host, CLI, GUI)
CTL
dpkg-deb --build --root-owner-group "${DEB_ROOT}" "${DEST}/${DEB_NAME}" >/dev/null

APPDIR="${WORKDIR}/RustTraycer.AppDir"
mkdir -p "${APPDIR}/usr/bin"
install -m 0755 "${BIN}/rt-host" "${BIN}/rt-cli" "${BIN}/rt-gui" "${APPDIR}/usr/bin/"
cat > "${APPDIR}/rusttraycer.desktop" << DESK
[Desktop Entry]
Type=Application
Name=RustTraycer
Exec=rt-gui
Icon=rusttraycer
Categories=Development;
DESK
printf '%s' 'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==' \
  | base64 -d > "${APPDIR}/rusttraycer.png"
cat > "${APPDIR}/AppRun" << 'RUN'
#!/bin/sh
HERE="$(dirname "$(readlink -f "$0")")"
export PATH="${HERE}/usr/bin:${PATH}"
exec "${HERE}/usr/bin/rt-gui" "$@"
RUN
chmod 0755 "${APPDIR}/AppRun"

APPIMAGE="${DEST}/${APPIMAGE_NAME}"
if [ "${PACK_SKIP_APPIMAGE:-}" = "1" ]; then
  echo "skip AppImage (PACK_SKIP_APPIMAGE=1)"
else
  TOOL="${WORKDIR}/appimagetool"
  if [ -n "${APPIMAGETOOL:-}" ] && [ -x "${APPIMAGETOOL}" ]; then
    TOOL="${APPIMAGETOOL}"
  else
    url="https://github.com/AppImage/appimagetool/releases/download/continuous/appimagetool-x86_64.AppImage"
    if command -v curl >/dev/null; then
      curl -fsSL -o "${TOOL}" "${url}"
    else
      wget -q -O "${TOOL}" "${url}"
    fi
    chmod 0755 "${TOOL}"
  fi
  ARCH=x86_64 APPIMAGE_EXTRACT_AND_RUN=1 "${TOOL}" "${APPDIR}" "${APPIMAGE}"
fi

(
  cd "${DEST}"
  : > SHA256SUMS.new
  if [ -f SHA256SUMS ]; then
    grep -E "${NAME}\\.tar\\.gz$" SHA256SUMS >> SHA256SUMS.new || true
  fi
  sha256sum "${DEB_NAME}" >> SHA256SUMS.new
  if [ -f "$(basename "${APPIMAGE}")" ]; then
    sha256sum "$(basename "${APPIMAGE}")" >> SHA256SUMS.new
  fi
  mv SHA256SUMS.new SHA256SUMS
)

echo "packed ${DEB_NAME}"
if [ -f "${APPIMAGE}" ]; then
  echo "packed $(basename "${APPIMAGE}")"
fi
