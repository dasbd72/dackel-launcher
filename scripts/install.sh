#!/bin/sh
# Installs dackel-launcher and umu-run for the current user.
#
#   curl -fsSL https://github.com/dasbd72/dackel-launcher/releases/latest/download/install.sh | sh
#
# Run it again to update. Pass a release tag to install that version instead of the latest,
# or --uninstall to remove the program, its menu entry and its icon (never games or settings).
# See docs/install-script.md.
set -eu

base=${DACKEL_RELEASE_BASE:-https://github.com/dasbd72/dackel-launcher/releases}
data=${XDG_DATA_HOME:-$HOME/.local/share}
dir=$data/dackel-launcher
bin=$dir/bin
app_id=io.github.dasbd72.DackelLauncher
entry=$data/applications/$app_id.desktop
icon=$data/icons/hicolor/scalable/apps/$app_id.svg

die() {
    echo "install.sh: $*" >&2
    exit 1
}

command -v curl >/dev/null 2>&1 || die "curl is required"
command -v sha256sum >/dev/null 2>&1 || die "sha256sum is required"

case ${1:-} in
--uninstall)
    rm -rf "$dir"
    rm -f "$entry" "$icon"
    echo "Removed dackel-launcher from $dir"
    exit 0
    ;;
"") url=$base/latest/download ;;
-*) die "unknown option $1 (use a release tag such as v0.1.0, or --uninstall)" ;;
*) url=$base/download/$1 ;;
esac

mkdir -p "$data"
stage=$(mktemp -d "$data/.dackel-install.XXXXXX")
trap 'rm -rf "$stage"' EXIT

fetch() {
    curl -fsSL -o "$stage/$1" "$url/$1" || die "could not download $url/$1"
}

files="dackel-launcher umu-run $app_id.desktop $app_id.svg"

fetch SHA256SUMS
for file in $files; do
    fetch "$file"
done

# Every file must match the release's checksums before anything is replaced.
for file in $files; do
    line=$(grep "  $file\$" "$stage/SHA256SUMS") || die "$file is not in SHA256SUMS"
    (cd "$stage" && echo "$line" | sha256sum -c - >/dev/null 2>&1) || die "$file does not match SHA256SUMS"
done

# The release's menu entry, started by the installed program's absolute path.
exec_path=$bin/dackel-launcher
case $exec_path in
*[[:space:]]*) exec_path="\"$exec_path\"" ;;
esac
sed "s|^Exec=.*|Exec=$exec_path|" "$stage/$app_id.desktop" >"$stage/entry"

chmod 755 "$stage/dackel-launcher" "$stage/umu-run"
mkdir -p "$bin" "$(dirname "$entry")" "$(dirname "$icon")"
mv -f "$stage/dackel-launcher" "$stage/umu-run" "$bin/"
mv -f "$stage/$app_id.svg" "$icon"
mv -f "$stage/entry" "$entry"
# Tells the program it was installed by this script, so it may update itself.
: >"$bin/.installed-by-script"
echo "Installed dackel-launcher and umu-run in $bin"
