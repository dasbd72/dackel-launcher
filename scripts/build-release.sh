#!/usr/bin/env bash
# Builds dist/dlsite-launcher inside a Debian 12 container, so the binary needs only an old
# glibc and runs on both the Arch desktop and the Steam Deck. Copy it next to DLSiteLibrary/.
set -euo pipefail

cd "$(dirname "$0")/.."
image=dlsite-launcher-build

docker build -q -t "$image" -f scripts/Dockerfile.release scripts >/dev/null

mkdir -p dist target-docker .cargo-docker
docker run --rm \
    --user "$(id -u):$(id -g)" \
    -e HOME=/tmp/home \
    -e CARGO_HOME=/work/.cargo-docker \
    -e CARGO_TARGET_DIR=/work/target-docker \
    -v "$PWD":/work -w /work \
    "$image" \
    cargo build --release --locked

cp target-docker/release/dlsite-launcher dist/dlsite-launcher

# Highest glibc version the binary asks for: must stay at or below what the Deck has.
need=$(objdump -T dist/dlsite-launcher | grep -o 'GLIBC_[0-9.]*' | sort -uV | tail -1)
echo "built dist/dlsite-launcher ($(du -h dist/dlsite-launcher | cut -f1)), needs up to $need"
