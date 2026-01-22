#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)"
ROOT_DIR="$(cd -- "${SCRIPT_DIR}/../.." && pwd -P)"
TEMPL_DIR="$SCRIPT_DIR/templates"
CONF_JSON="$ROOT_DIR/src-tauri/tauri.conf.json"
LINUX_CONF_JSON="$ROOT_DIR/src-tauri/tauri.linux.conf.json"

# Get version
VERSION="$(jq -r '.version // empty' "$LINUX_CONF_JSON")"
if [[ -z "$VERSION" ]]; then
  echo "Please specify version for linux build in $LINUX_CONF_JSON" >&2
  exit 1
fi
export DMNOTE_VERSION="$VERSION"

# Get description
DESCRIPTION="$(jq -r --arg d 'Unofficial Linux Implementation for DM NOTE: A Fully Customizable Key Viewer Optimized for DJMAX RESPECT V, Ready for Any Games' '.bundle.longDescription // $d' "$CONF_JSON")"

echo "=== BUNDLING FOR ${VERSION} ==="
echo
OUTPUT_DIR="$SCRIPT_DIR/$VERSION"
rm -rf -- "$OUTPUT_DIR"
mkdir -p -- "$OUTPUT_DIR"

# Build and bundle for rpm and deb
echo "=== Start bundling for deb and rpm packages ==="
npm run tauri:build
DEB_SRC="$ROOT_DIR/src-tauri/target/release/bundle/deb/DM_NOTE_${VERSION}_amd64.deb"
if [[ ! -f "$DEB_SRC" ]]; then
	echo "deb package not found: ${DEB_SRC}" >&2
  exit 1
fi
RPM_SRC="$ROOT_DIR/src-tauri/target/release/bundle/rpm/DM_NOTE-${VERSION}-1.x86_64.rpm"
if [[ ! -f "$RPM_SRC" ]]; then
	echo "rpm package not found as ${RPM_SRC}" >&2
  exit 1
fi
cp -f -- "$DEB_SRC" "$OUTPUT_DIR/"
cp -f -- "$RPM_SRC" "$OUTPUT_DIR/"


# Bundle for pkg.tar.zst (pacman)
echo "=== Start bundling for pacman packages ==="
TMP_DIR="$(mktemp -d ./temp.XXXXXX)"
trap 'rm -rf "$TMP_DIR"' EXIT
pushd "${TMP_DIR}" > /dev/null

# Main release package
cp -r "$TEMPL_DIR/arch-deb" .
pushd arch-deb > /dev/null
cp "$DEB_SRC" .
sed -i "s|<version>|$VERSION|g" ./PKGBUILD
ESC_DESC="$(printf '%s' "$DESCRIPTION" | sed 's/["\\/&|]/\\&/g')"
sed -i "s|<description>|$ESC_DESC|g" ./PKGBUILD
makepkg -s --skipchecksums
PKG_SRC=( *.pkg.tar.zst )
cp -f -- "${PKG_SRC[@]}" "$OUTPUT_DIR/"
popd > /dev/null

popd > /dev/null
