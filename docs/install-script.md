# One-line installer

Spec: GitHub issue #17. Source: `scripts/install.sh`.

```sh
curl -fsSL https://github.com/dasbd72/dackel-launcher/releases/latest/download/install.sh | sh
```

- **Layout:** `~/.local/share/dackel-launcher/bin/dackel-launcher` and `bin/umu-run`. The launcher
  already looks for `umu-run` beside its own binary, so the app needs no change.
- **Downloads:** the two programs, the menu entry and its icon (the files in `packaging/`) come
  from the same release and are checked against its `SHA256SUMS` before anything is replaced. A
  failed check leaves the old install alone. The release workflow attaches all of them.
- **Menu entry:** written to `~/.local/share/applications` with an absolute `Exec=`; the icon is
  copied into `~/.local/share/icons/hicolor/scalable/apps`. The script adds no Steam shortcut.
  Steam applies the gamepad layout only to programs it started, so on the Deck add the installed
  `dackel-launcher` as a non-Steam game and pick the Gamepad layout, or the sticks act as arrow keys.
- **Library:** none is created; the first run shows the empty state (#14).
- **Update:** run the same command again. Games, profiles and config are never touched.
- **Version:** latest by default, or pass a tag: `… | sh -s -- v0.1.0`.
- **Uninstall:** `… | sh -s -- --uninstall` removes the install folder, entry and icon only.

## Why not Flathub

The launcher runs `umu-run` and Wine on the host and reads libraries from the SD card and other
folders. A sandboxed build would need a sandbox escape or broad filesystem access, and Flathub adds
review and upkeep. The local Flatpak (#16) stays an experiment; the AUR package (#15) is unaffected.

## Tests

`tests/install.rs` runs `install.sh` against a throwaway home and a fake release in a local
folder (`DACKEL_RELEASE_BASE` overrides the download base), so no network is used.
