//! Where controller focus is, and the rules that keep it somewhere that exists.
//!
//! Plain values only: the window code hands in the visible game ids and where each focusable
//! widget was drawn last frame, and reads back what has focus.

use crate::nav::{self, Bounds, Control, Dir};

/// Something the controller can put focus on.
#[derive(Clone, Debug, PartialEq)]
pub enum Target {
    /// A game tile in the library grid, by game id.
    Game(String),
    /// A toolbar or details-page control.
    Control(Control),
}

/// A focusable widget and where it was drawn.
#[derive(Clone, Debug, PartialEq)]
pub struct Slot {
    pub target: Target,
    pub bounds: Bounds,
}

/// The page whose controls may have focus.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Page {
    Grid,
    Details,
    /// The Libraries window is open over the page.
    Libraries,
}

#[derive(Debug)]
pub struct Cursor {
    /// The game the cursor is on (by id, so it survives rescans and re-sorts).
    game: Option<String>,
    /// True until the user moves the cursor: it then tracks the first game as the list re-sorts.
    auto: bool,
    /// Set when focus is on a toolbar or details-page control instead of a game tile.
    control: Option<Control>,
    /// A control the controller pressed with A, consumed by that control next frame.
    pending_press: Option<Control>,
    /// Set when focus just moved, so the focused widget is scrolled into view.
    scroll: bool,
    /// The selected row of the open popup list.
    list_sel: usize,
    /// Where everything focusable was on the last frame; D-pad moves use it.
    slots: Vec<Slot>,
}

impl Default for Cursor {
    fn default() -> Self {
        Cursor {
            game: None,
            auto: true,
            control: None,
            pending_press: None,
            scroll: false,
            list_sel: 0,
            slots: Vec::new(),
        }
    }
}

impl Cursor {
    pub fn new() -> Self {
        Self::default()
    }

    /// The game the cursor is on.
    pub fn game(&self) -> Option<&str> {
        self.game.as_deref()
    }

    /// The control that has focus, if focus is on a control rather than a game tile.
    pub fn control(&self) -> Option<Control> {
        self.control
    }

    /// True when focus just moved and should be scrolled into view.
    pub fn scroll_requested(&self) -> bool {
        self.scroll
    }

    pub fn list_sel(&self) -> usize {
        self.list_sel
    }

    /// Keeps the cursor on a game that is on screen, so focus is never lost: it follows the
    /// first visible game until the user has moved it, and goes back there when its game is
    /// no longer visible.
    pub fn keep_on_game(&mut self, visible: &[String]) {
        let on_screen = self.game.as_ref().is_some_and(|id| visible.contains(id));
        if self.auto || !on_screen {
            self.game = visible.first().cloned();
        }
    }

    /// Keeps controller focus on something that exists on `page`.
    pub fn keep_on_page(&mut self, page: Page) {
        let drawn = |c: Control| self.slots.iter().any(|s| s.target == Target::Control(c));
        match (page, self.control) {
            // The slots are still the previous page's layout until the details page has been
            // drawn once (its Back button is the first thing it registers).
            (Page::Details, _) if !drawn(Control::Back) => {}
            // The Libraries window keeps focus on its own controls, once it has been drawn.
            (Page::Libraries, _) if !drawn(Control::LibraryClose) => {}
            (Page::Libraries, Some(c)) if c.in_library_window() && drawn(c) => {}
            (Page::Libraries, _) => self.control = Some(Control::LibraryPath),
            // The details page always has focus on one of its controls.
            (Page::Details, Some(c)) if drawn(c) => {}
            (Page::Details, _) => self.control = Some(Control::Back),
            // In the library, focus on a control that is gone goes back to the games.
            (Page::Grid, Some(c)) if !drawn(c) => self.control = None,
            _ => {}
        }
    }

    /// Moves focus to the nearest control or tile in `dir`, using where things were drawn.
    /// When what has focus is not on screen (nothing is focused, or its game is gone), the
    /// first move lands on the first thing drawn, so the controller can always reach the
    /// toolbar.
    pub fn move_toward(&mut self, dir: Dir) {
        let current = match (&self.control, &self.game) {
            (Some(c), _) => Some(Target::Control(*c)),
            (None, Some(id)) => Some(Target::Game(id.clone())),
            (None, None) => None,
        };
        let from = current.and_then(|c| self.slots.iter().position(|s| s.target == c));
        let to = match from {
            Some(from) => {
                let rects: Vec<Bounds> = self.slots.iter().map(|s| s.bounds).collect();
                nav::spatial_next(&rects, from, dir)
            }
            None if self.slots.is_empty() => return,
            None => 0,
        };
        match self.slots[to].target.clone() {
            Target::Game(id) => {
                self.game = Some(id);
                self.control = None;
                self.auto = false;
                self.scroll = true;
            }
            Target::Control(control) => {
                self.control = Some(control);
                self.scroll = true;
            }
        }
    }

    /// Puts focus on a control that is about to be drawn (the page changed under it).
    pub fn focus(&mut self, control: Control) {
        self.control = Some(control);
        self.scroll = true;
    }

    /// The user picked a game tile directly (a click).
    pub fn select_game(&mut self, id: String) {
        self.game = Some(id);
        self.control = None;
        self.auto = false;
    }

    /// Opens the details page: focus starts on its Launch button.
    pub fn open_details(&mut self) {
        self.control = Some(Control::Launch);
    }

    /// Back to the library: focus returns to the game that was selected.
    pub fn close_details(&mut self) {
        self.control = None;
        self.scroll = true;
    }

    /// The controller pressed A on `control`; it acts on it next frame.
    pub fn press(&mut self, control: Control) {
        self.pending_press = Some(control);
    }

    /// The press waiting for its control, taken so it acts only once.
    pub fn take_press(&mut self) -> Option<Control> {
        self.pending_press.take()
    }

    /// The frame is drawn: remember where everything was, and the scroll request is spent.
    pub fn end_frame(&mut self, slots: Vec<Slot>) {
        self.slots = slots;
        self.scroll = false;
    }

    /// A popup list closed or opened on its first row.
    pub fn reset_list(&mut self) {
        self.list_sel = 0;
    }

    /// A popup list opened with this row selected.
    pub fn set_list(&mut self, row: usize) {
        self.list_sel = row;
    }

    /// Moves the selected row by `delta` in a list of `len` rows, staying inside it.
    pub fn move_list(&mut self, delta: i32, len: usize) {
        let last = len.saturating_sub(1) as i64;
        self.list_sel = (self.list_sel as i64 + i64::from(delta)).clamp(0, last) as usize;
    }
}
