# Dackel Launcher
## Release build

`scripts/build-release.sh` builds `dist/dackel-launcher` in a Debian 12 container so it runs
on both the Arch desktop and the Steam Deck.

Pushing a tag such as `v0.2.0` runs `.github/workflows/release.yml`, which does the same build
and attaches `dackel-launcher`, `umu-run`, `install.sh`, the menu entry, its icon and
`SHA256SUMS` to a GitHub release.

## Install

```sh
curl -fsSL https://github.com/dasbd72/dackel-launcher/releases/latest/download/install.sh | sh
```

Installs the launcher and `umu-run` for your user and adds a menu entry. Run it again to update.
See `docs/install-script.md`.

On Arch, the AUR has `dackel-launcher` (built from source) and `dackel-launcher-bin` (the
release binary); both are published by `.github/workflows/aur.yml` for every version tag.

## Set up a library

Make a folder for your games, one folder per game named by its RJ code, and put the game's zip
inside:

```
Library/
└── RJ123456/
    └── RJ123456.zip
```

Add `Library` with the folder button in the launcher (Libraries), then rescan.

## License

MIT, see `LICENSE`. The files that the AUR and Flatpak packages install (desktop entry, icon,
AppStream metadata) are in `packaging/`.

## Credits

Icons, including the app icon's paw print, are [Phosphor Icons](https://phosphoricons.com) (MIT); the license is in `assets/icons/LICENSE-phosphor.txt`.
