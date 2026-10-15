# Dackel Launcher
## Release build

`scripts/build-release.sh` builds `dist/dackel-launcher` in a Debian 12 container so it runs
on both the Arch desktop and the Steam Deck. See `docs/release-checklist.md` for how to
copy it onto the drive and what to check on each machine.

Pushing a tag such as `v0.2.0` runs `.github/workflows/release.yml`, which does the same build
and attaches `dackel-launcher`, `umu-run` and `SHA256SUMS` to a GitHub release.

## License

MIT, see `LICENSE`. The files that the AUR and Flatpak packages install (desktop entry, icon,
AppStream metadata) are in `packaging/`.

## Credits

Icons, including the app icon's paw print, are [Phosphor Icons](https://phosphoricons.com) (MIT); the license is in `assets/icons/LICENSE-phosphor.txt`.
