//! The gamepad: its buttons and left stick turned into `Input`s, the key-repeat for a held
//! direction, the stick's scroll request, the press debounce, and the Steam Input watch.

use crate::nav::{self, Debounce, Dir, Input, Repeater};
use crate::steam_input::Watch;
use gilrs::{Axis, Button, EventType, Gilrs};

pub struct Pad {
    /// `None` when gamepad support could not start.
    gilrs: Option<Gilrs>,
    steam_watch: Watch,
    stick: Repeater,
    /// Drops a press that a second device (Steam's virtual pad) reports again.
    debounce: Debounce,
    /// What the left stick asks a page to do this frame (-1.0 down .. 1.0 up).
    scroll: f32,
}

impl Pad {
    pub fn new(debounce_ms: u64) -> Self {
        Pad {
            gilrs: Gilrs::new()
                .map_err(|err| eprintln!("gamepad support unavailable: {err}"))
                .ok(),
            ..Self::without_gamepad(debounce_ms, Watch::from_env())
        }
    }

    /// A pad that never reads a device; the stick and debounce can still be driven by hand.
    pub fn without_gamepad(debounce_ms: u64, steam_watch: Watch) -> Self {
        Pad {
            gilrs: None,
            steam_watch,
            stick: Repeater::default(),
            debounce: Debounce::new(debounce_ms),
            scroll: 0.0,
        }
    }

    /// Button presses and stick moves since the last call, or `None` when gamepad support
    /// is not running. A pad that is not focused (a game has the screen) gives no inputs.
    /// `scrolls` turns an up or down push into a scroll request instead of focus moves.
    pub fn poll(&mut self, now_ms: u64, focused: bool, scrolls: bool) -> Option<Vec<Input>> {
        let gilrs = self.gilrs.as_mut()?;
        let mut inputs = Vec::new();
        while let Some(event) = gilrs.next_event() {
            self.steam_watch.pad_seen();
            if let (true, EventType::ButtonPressed(button, _)) = (focused, event.event)
                && let Some(input) = pad_input(button)
            {
                inputs.push(input);
            }
        }
        let stick = gilrs
            .gamepads()
            .map(|(_, pad)| (pad.value(Axis::LeftStickX), pad.value(Axis::LeftStickY)))
            .find(|&(x, y)| nav::stick_dir(x, y).is_some())
            .unwrap_or((0.0, 0.0));
        inputs.extend(self.stick_inputs(now_ms, stick, focused, scrolls));
        Some(inputs)
    }

    /// The inputs a left-stick position makes at `now_ms`: a direction when it is pushed
    /// (repeating while held), or a scroll request when `scrolls` and it is pushed up or down.
    pub fn stick_inputs(
        &mut self,
        now_ms: u64,
        stick: (f32, f32),
        focused: bool,
        scrolls: bool,
    ) -> Vec<Input> {
        let scroll = if scrolls && focused {
            nav::stick_scroll(stick.0, stick.1)
        } else {
            0.0
        };
        self.scroll = scroll;
        let held = nav::stick_dir(stick.0, stick.1).filter(|_| scroll == 0.0);
        let fired = self.stick.update(now_ms, held.filter(|_| focused));
        fired.map(dir_input).into_iter().collect()
    }

    /// What the stick asked a page to do at the last poll (-1.0 down .. 1.0 up).
    pub fn scroll(&self) -> f32 {
        self.scroll
    }

    /// An arrow key was pressed; returns the Steam Input hint the first time it looks like
    /// the desktop layout is on.
    pub fn arrow_pressed(&mut self) -> Option<&'static str> {
        self.steam_watch.arrow_pressed()
    }

    pub fn debounce_window(&self) -> u64 {
        self.debounce.window()
    }

    pub fn set_debounce_window(&mut self, ms: u64) {
        self.debounce.set_window(ms);
    }

    /// Keeps the presses to act on, dropping one that repeats the same button too soon.
    pub fn debounced(&mut self, now_ms: u64, inputs: Vec<Input>) -> Vec<Input> {
        inputs
            .into_iter()
            .filter(|input| self.debounce.accept(now_ms, *input))
            .collect()
    }
}

pub fn pad_input(button: Button) -> Option<Input> {
    Some(match button {
        Button::DPadUp => Input::Up,
        Button::DPadDown => Input::Down,
        Button::DPadLeft => Input::Left,
        Button::DPadRight => Input::Right,
        Button::South => Input::A,
        Button::East => Input::B,
        Button::West => Input::X,
        Button::North => Input::Y,
        Button::Start => Input::Start,
        Button::Select => Input::Select,
        _ => return None,
    })
}

pub fn dir_input(dir: Dir) -> Input {
    match dir {
        Dir::Up => Input::Up,
        Dir::Down => Input::Down,
        Dir::Left => Input::Left,
        Dir::Right => Input::Right,
    }
}
