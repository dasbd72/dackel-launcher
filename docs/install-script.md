# One-line installer

Spec: GitHub issue #17. Status: not built yet.

```sh
curl -fsSL https://github.com/dasbd72/dackel-launcher/releases/latest/download/install.sh | sh
```

- **Layout:** `~/.local/share/dackel-launcher/bin/dackel-launcher` and `bin/umu-run`. The launcher
  already looks for `umu-run` beside its own binary, so the app needs no change.
- **Downloads:** both binaries come from the same release and are checked against its
  `SHA256SUMS` before anything is replaced. A failed check leaves the old install alone.
- **Menu entry:** written to `~/.local/share/applications` with an absolute `Exec=`; the icon is
  copied into `~/.local/share/icons`. No Steam shortcut.
- **Library:** none is created; the first run shows the empty state (#14).
- **Update:** run the same command again. Games, profiles and config are never touched.
- **Version:** latest by default, or pass a tag: `… | sh -s -- v0.1.0`.
- **Uninstall:** `… | sh -s -- --uninstall` removes the install folder, entry and icon only.

## Why not Flathub

The launcher runs `umu-run` and Wine on the host and reads libraries from the SD card and other
folders. A sandboxed build would need a sandbox escape or broad filesystem access, and Flathub adds
review and upkeep. The local Flatpak (#16) stays an experiment; the AUR package (#15) is unaffected.

## Tests

One integration test runs `install.sh` against a throwaway home and a fake release in a local
folder (the download base is overridable), so no network is used. See the spec for the cases.
