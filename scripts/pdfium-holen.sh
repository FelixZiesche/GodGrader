#!/usr/bin/env bash
# Lädt die PDFium-Bibliothek (bblanchon/pdfium-binaries) nach vendor/pdfium.
# Version muss zur Cargo-Feature „pdfium_8125“ von pdfium-render passen.
set -euo pipefail
VERSION="8125"
case "${1:-$(uname -s)}" in
  win|Windows*|MINGW*|MSYS*) ASSET="pdfium-win-x64.tgz" ;;
  linux|Linux*) ASSET="pdfium-linux-x64.tgz" ;;
  mac|Darwin*) ASSET="pdfium-mac-arm64.tgz" ;;
  *) echo "Unbekannte Plattform"; exit 1 ;;
esac
ZIEL="$(dirname "$0")/../vendor/pdfium"
rm -rf "$ZIEL" && mkdir -p "$ZIEL"
curl -fsSL "https://github.com/bblanchon/pdfium-binaries/releases/download/chromium%2F${VERSION}/${ASSET}" | tar xz -C "$ZIEL"
echo "PDFium ${VERSION} in ${ZIEL}"
