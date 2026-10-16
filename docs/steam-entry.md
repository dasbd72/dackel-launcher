# Steam entries for games

Spec: GitHub issue #28.

The details page of a game has **Add to Steam** / **Remove from Steam** when Steam is found
(`~/.local/share/Steam`, `~/.steam/steam` or the Flatpak folder).

- **Entry:** a non-Steam shortcut in the newest Steam account's `shortcuts.vdf`, running
  `dackel-launcher --launch "<game folder>"`. Entries are recognised by those launch options,
  and nothing else in the file is changed. The first change keeps the original as
  `shortcuts.vdf.dackel-backup`.
- **`--launch <game folder>`:** starts the game with its saved Proton and launch options, with no
  window, and stays until the game ends so Steam and the play time see it. A game with several
  programs and none chosen must be chosen in the launcher first.
- **Cover:** copied into the account's `grid` folder as `<appid>.png` and `<appid>p.png`.
- **Steam running:** the file is still written, but Steam may overwrite it when it exits.
  Restart Steam to see the entry, and add entries with Steam closed to be safe.
- **Moving the launcher:** adding any entry rewrites the program path of all our entries.

- **The launcher itself:** the toolbar has **Add to Steam** when Steam has no shortcut for the launcher.
  Once there is one (including one made by hand), it shows **Remove from Steam**, and **Update in
  Steam** when the launcher has moved since (the shortcut keeps its app id and name). A shortcut is
  the launcher's when it has no launch options and its program is named `dackel-launcher`.

- **Launcher artwork:** adding or updating the launcher's shortcut writes a full image set (portrait and
  wide capsules, hero, logo, icon) into the account's `grid` folder and sets the shortcut's icon. The
  PNGs in `assets/steam/` are embedded in the program and drawn by `scripts/make-steam-art.sh`
  (rsvg-convert and ImageMagick); the colours and paw come from the app icon. With the shortcut in
  place the toolbar's update button reads **Update Steam images** until the launcher moves.
