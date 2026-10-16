# Folder browser for adding a library

Spec: GitHub issue #18. Status: built for mouse and keyboard; the gamepad cannot pick yet.

The Libraries window gets a **Browse...** button next to the typed path box. It opens a folder
browser inside the program, using the `egui-file-dialog` crate (pure egui, no GTK or desktop
portal), so the Steam Deck can add a library with the gamepad and no keyboard.

- **Starts at:** the home folder (the root folder if home is unknown).
- **Shows:** folders only, with the crate's drive and common-folder list. Hidden folders are
  off by default, with a toggle.
- **Adds:** the folder you select, with the dialog's Open button (the crate has no "add the folder you are in"). The picked folder goes
  through the same check as a typed path (`add_library`), so both give the same errors.
- **Typed path box:** stays as it is.
- **Gamepad:** the probe (reading the crate's source) found that the D-pad can move and open folders but cannot press Open, so the Deck keeps typing a path for now. A browser of our own is the way to fix it. The original plan: the first step is a probe that the D-pad can open folders, go up,
  press "Add this folder" and close the browser. If it cannot, stop and decide between our own
  browser and a mouse-and-keyboard-only dialog.
- **SD card:** relies on the crate's drive list; a pinned shortcut is added only if the Deck
  does not show it.

## Tests

The picked-folder check lives with the existing library tests (same errors as a typed path, the
start-folder choice, hidden folders off). The dialog itself is checked by hand on the desktop and
the Deck.
