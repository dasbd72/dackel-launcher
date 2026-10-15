# Release build and cross-machine check

## Build

```sh
scripts/build-release.sh
```

This builds inside a Debian 12 container (glibc 2.36) and writes `dist/dackel-launcher`.
Building on Arch directly would need Arch's newest glibc, which SteamOS does not have.
The script prints the highest glibc version the binary asks for; it should be 2.36 or less.

Copy the binary next to the library folder on the drive:

```
DLSite/
  dackel-launcher
  DLSiteLibrary/RJ.../
  umu-run
```

It looks for `DLSiteLibrary/` beside itself, so the drive can be mounted anywhere.
`--library /some/path` uses another folder for one run.

## What the Deck needs

- `umu-run`: SteamOS is read-only, so copy `dist/umu-run` (from `scripts/fetch-umu.sh`)
  beside `dackel-launcher`. The launcher looks at `$UMU_RUN`, beside itself, the `PATH`,
  then `~/.local/bin`. It needs `python3` (the Deck has it) and the internet on the first
  run, when umu downloads the Steam Linux Runtime.
- A GE-Proton under
  `~/.local/share/Steam/compatibilitytools.d/` (or `~/.config/dackel-launcher/proton-path`
  / `$PROTONPATH` pointing at one).
- A font with Japanese glyphs (SteamOS ships one).

## Steam Input

Steam picks the controller layout per program, and only for programs it started itself.
So always start the launcher from its Steam shortcut, never from a file manager or terminal.

1. In Steam, **Add a Non-Steam Game** and choose `dackel-launcher` on the drive. (The program
   used to be called `dlsite-launcher`: remove that old shortcut and add this one again, and
   pick the Gamepad layout for it again, since Steam ties both to the file name.)
2. Open the shortcut's **Controller Settings** (the controller icon on its page).
3. Under **Steam Input**, pick **Enable Steam Input** (not "Use default settings" if that
   is the desktop layout).
4. Choose the **Gamepad** template as its layout and apply it.
5. Start the launcher with **Play**. Its window now gets the gamepad layout while it is
   focused; the desktop layout returns when you switch to another window.

Games started from the launcher run inside that same layout, so they see a normal gamepad.

The desktop layout turns the sticks into arrow keys, so the launcher never sees a stick and
each press can arrive twice. If the launcher was started by Steam but only sees arrow keys,
it says so in the status line. If a D-pad press still moves twice, raise **Debounce** in the
toolbar.

## Checklist

Tick each on the **desktop** and on the **Deck**.

| Check | Desktop | Deck |
| --- | --- | --- |
| The same `dist/dackel-launcher` file starts (no `GLIBC_...` or missing-library error) | [ ] | [ ] |
| Games from `DLSiteLibrary/` appear, covers and Japanese titles render | [ ] | [ ] |
| Starts fullscreen on the Deck / windowed on the desktop (Fullscreen button and F11 toggle it) | [ ] | [ ] |
| D-pad moves focus over the toolbar, tiles and details buttons; A presses; B backs out | [ ] | [ ] |
| Left stick scrolls the details page; the page follows focus | [ ] | [ ] |
| The Sort, UI scale, Debounce and Zip encoding popups open and pick by controller | [ ] | [ ] |
| One D-pad press moves focus once (Steam Input) | [ ] | [ ] |
| Extract a zip: progress shows, Cancel works | [ ] | [ ] |
| Re-extract over existing files: the popup lists them, search filters, Cancel / Overwrite / Skip existing work | [ ] | [ ] |
| **A real game launches from the gamepad and plays**; play time is saved on exit | [ ] | [ ] |
| Launcher comes back after the game exits (window restore) | [ ] | [ ] |
| Mount the drive somewhere else (or copy the folder elsewhere): it still finds `DLSiteLibrary/` | [ ] | [ ] |
| The first launch pins the Proton version (details page > Launch options) | [ ] | [ ] |
| A game pinned on one machine shows the "Proton version missing" prompt on the other if its version is not installed | [ ] | [ ] |
| Libraries window: add a second folder; its games show with the library filter | [ ] | [ ] |
