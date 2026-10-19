# Dackel Launcher

A game library for Linux and the Steam Deck that you can carry on a drive. It lists your game folders, extracts zip downloads
and starts each game through [umu](https://github.com/Open-Wine-Components/umu-launcher) and
Proton. Folders can have any name; `RJ…` (DLsite) and `RA…` (072Project) names also get store
info and covers.

## Highlights

- **Portable.** Everything a game needs (its settings, Wine prefix, saves, cover and info) lives
  in its own folder, so the library moves between your desktop and the Deck with the games. Put
  the launcher and `umu-run` beside a `DLSiteLibrary` folder and run it from the drive.
  Per-machine settings such as UI size stay on each computer.
- **Made for the couch.** Works with a gamepad, a mouse or a keyboard, on the desktop and the
  Steam Deck.
- **One folder per game.** Scans libraries on several drives or SD cards at once, with search,
  sort and favorites. A folder can be named anything.
- **Store info and covers.** `RJ…` (DLsite) and `RA…` (072Project) folders get title, circle,
  genres, description and cover. One switch blurs all covers.
- **Add zip.** Moves downloaded archives into the right game folder and extracts them in the
  background.
- **Per-game settings.** Each game has its own Proton version, Wine prefix, environment variables
  and launch arguments, and the launcher finds the game's exe.
- **Winetricks in one click.** Install `dotnet48`, `vcrun2022` and other verbs into a game's
  prefix.
- **Steam integration.** Add a game, or the launcher itself, as a non-Steam entry with generated
  artwork.
- **Easy install and updates.** One-line installer and AUR packages, with an update check in
  Settings.

## Install

```sh
curl -fsSL https://github.com/dasbd72/dackel-launcher/releases/latest/download/install.sh | sh
```

Installs the launcher and `umu-run` for your user and adds a menu entry. Run it again to update.
See `docs/install-script.md`.

On Arch, the AUR has `dackel-launcher` (built from source) and `dackel-launcher-bin` (the
release binary); both are published by `.github/workflows/aur.yml` for every version tag.

## Set up a library

Make a folder for your games, one folder per game named after the game (an RJ or RA code also
gets store info), and put the game's zip inside:

```
Library/
└── RJ123456/
    └── RJ123456.zip
```

A folder named `DLSiteLibrary` beside the launcher is found automatically. For any other folder,
add it with the folder button in the launcher (Libraries), then rescan.

## Release build

`scripts/build-release.sh` builds `dist/dackel-launcher` in a Debian 12 container so it runs
on both the Arch desktop and the Steam Deck.

Pushing a tag such as `v0.2.0` runs `.github/workflows/release.yml`, which does the same build
and attaches `dackel-launcher`, `umu-run`, `install.sh`, the menu entry, its icon and
`SHA256SUMS` to a GitHub release.

## License

MIT, see `LICENSE`. The files that the AUR and Flatpak packages install (desktop entry, icon,
AppStream metadata) are in `packaging/`.

## Credits

Icons, including the app icon's paw print, are [Phosphor Icons](https://phosphoricons.com) (MIT); the license is in `assets/icons/LICENSE-phosphor.txt`.
