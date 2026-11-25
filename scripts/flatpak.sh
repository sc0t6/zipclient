#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
test -x src-tauri/target/release/zip-launcher || { echo "Run npm run release on Linux first"; exit 1; }
flatpak remote-add --user --if-not-exists flathub https://dl.flathub.org/repo/flathub.flatpakrepo
flatpak-builder --user --install-deps-from=flathub --force-clean --repo=flatpak/repo flatpak/build flatpak/dev.ziplauncher.app.yml
mkdir -p release
flatpak build-bundle flatpak/repo release/Zip-Launcher-x86_64.flatpak dev.ziplauncher.app --runtime-repo=https://dl.flathub.org/repo/flathub.flatpakrepo
( cd release && ls | grep -vE ".sig$|^latest.json$|^SHA256SUMS.txt$" | sort | xargs sha256sum > SHA256SUMS.txt && cat SHA256SUMS.txt )
