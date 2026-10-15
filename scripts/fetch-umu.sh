#!/usr/bin/env bash
# Puts umu-launcher's standalone build at dist/umu-run, checked against a pinned checksum.
# Copy it beside dlsite-launcher: SteamOS is read-only, so umu cannot be installed there.
# umu-run is GPL-3 and is shipped as a separate, unmodified file.
set -euo pipefail

version=1.4.4
sha256=eb590691841f7fad3fc3ad8fd5db4ccb87849fe7948e62b28ece7a4ee48cc851
url="https://github.com/Open-Wine-Components/umu-launcher/releases/download/$version/umu-launcher-$version-zipapp.tar"

cd "$(dirname "$0")/.."
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

curl -fsSL -o "$work/umu.tar" "$url"
echo "$sha256  $work/umu.tar" | sha256sum -c --quiet -
tar -xf "$work/umu.tar" -C "$work" umu/umu-run
mkdir -p dist
install -m 755 "$work/umu/umu-run" dist/umu-run
echo "fetched dist/umu-run (umu-launcher $version)"
