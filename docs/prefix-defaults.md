# Prefix and launch defaults

What the launcher changes for every game without being asked. Each one is a candidate for a
per-game or global toggle later; none has one yet.

## Prefix registry (once per prefix)

Imported by `prefix::prepare` with `umu-run regedit /S` before the first launch. The marker
`dackel-defaults-<n>` inside the prefix records it; Reset prefix brings it back. Bump `VERSION`
in `src/prefix.rs` to re-apply after changing the list.

| Setting | Why | Turn off if |
| --- | --- | --- |
| `DllOverrides` `winemenubuilder.exe=""` | no desktop or menu entries from games | never needed |
| `WineDbg` `ShowCrashDialog=0` | no Wine crash dialog | debugging a crash |

## Launch environment (every launch)

Set in `launch::plan_with`; the game's own Env wins.

| Variable | Why | Turn off if |
| --- | --- | --- |
| `WINEDLLOVERRIDES=winewayland.drv=d` | Wine's Wayland driver cannot give OpenGL games a context: Bakin's `kmyOGL.dll` breaks (`0x80000003`) | a game needs native Wayland (HDR, fractional scale) |
| `LANG` / `LC_ALL=ja_JP.UTF-8` | Japanese games | a game wants another locale |
| `GAMEID=umu-<RJ code>` | one protonfixes id per game | never |

## Tried, not adopted

- `PROTON_ENABLE_WAYLAND=0`: no effect on GE-Proton11.
- `PROTON_USE_XALIA=0`: not the cause of the .NET message; left alone.
- Lutris' desktop folder links to the home directory: would move saves out of the prefix, so
  Reset prefix would stop clearing them.
- Lutris' `steamuser` link: Proton already provides `steamuser`.

## Known, per game

- Bakin games refuse to start on Wine Mono alone ("needs .NET 4.5 or later": Mono never writes
  the registry marker); install `dotnet48` into the prefix with winetricks. They also show
  "failed to build battlescript, using default battlescript": Roslyn needs
  `Microsoft.DiaSymReader.Native.amd64.dll` (comes with Windows), which Wine Mono lacks. The
  game runs on the default script; real .NET may supply the DLL (untested).
- RJ392812: `Game.exe` has a base-relocation directory pointing into `.rsrc`, so Wine (which
  moves the image off its preferred base) crashes in `ntdll` on the TLS index. Needs the header
  patched (directory entry RVA `0x256000`, size `0x227C`); no Proton version helps.
