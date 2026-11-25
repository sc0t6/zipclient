#!/usr/bin/env bash
set -euo pipefail
SRC=/mnt/c/Users/zippo/Documents/MyApps/ZipClient/
mkdir -p ~/zip ~/.tauri
cp /mnt/c/Users/zippo/.tauri/zip-updater.key ~/.tauri/zip-updater.key
chmod 600 ~/.tauri/zip-updater.key
rsync -a --delete --exclude node_modules --exclude src-tauri/target --exclude release --exclude dist --exclude .claude --exclude src-tauri/gen --exclude flatpak/build --exclude flatpak/repo "$SRC" ~/zip/
