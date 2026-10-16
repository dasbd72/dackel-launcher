//! Controller/keyboard navigation as pure functions, so it is testable without a window.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dir {
    Up,
    Down,
    Left,
    Right,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Input {
    Up,
    Down,
    Left,
    Right,
    A,
    B,
    X,
    Y,
    Start,
    Select,
    /// Keyboard only (F11).
    ToggleFullscreen,
    /// Keyboard only (B).
    ToggleBlur,
}

/// What the input means depends on what is on screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Context {
    Grid,
    Details,
    Chooser,
    /// A window of controls over the page (the Libraries manager): the D-pad moves focus.
    Window,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
    /// Move focus to the nearest control or game in a direction.
    Move(Dir),
    /// Press the focused control, or launch / extract / pick an exe for the focused game.
    Primary,
    Reload,
    OpenDetails,
    Back,
    ToggleFavorite,
    Rescan,
    ToggleFavoritesOnly,
    ResetFilters,
    ChooserMove(i32),
    ChooserPick,
    ChooserCancel,
    ToggleFullscreen,
    ToggleBlur,
}

/// Every control the controller can focus besides the game tiles.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Control {
    // Toolbar.
    Rescan,
    Libraries,
    /// Opens the Settings window.
    Settings,
    /// Adds the launcher itself to Steam.
    AddLauncherToSteam,
    /// Points the launcher's Steam shortcut at where the launcher is now.
    UpdateLauncherInSteam,
    /// Takes the launcher's shortcut out of Steam.
    RemoveLauncherFromSteam,
    /// On the empty-state page, opens the Libraries window.
    AddLibrary,
    /// On the empty-state page, creates the missing library folder.
    CreateLibrary,
    Fullscreen,
    Blur,
    Sort,
    LibraryFilter,
    FavoritesOnly,
    Quit,
    // Details page.
    Back,
    /// One of the exe candidates listed on the details page.
    ExeChoice(usize),
    Launch,
    ReloadInfo,
    Extract,
    CancelExtract,
    Favorite,
    /// Adds the game to Steam, or takes it out.
    SteamEntry,
    /// Writes the game's cover into Steam again.
    SteamImages,
    /// Switches the game's Steam icon between the cover and the exe's icon.
    SteamIconSource,
    // Launch options on the details page.
    LaunchOptions,
    EnvBox,
    ArgsBox,
    SaveTuning,
    ProtonChoice,
    /// A common winetricks verb chip, by its slot in `winetricks::COMMON`.
    WinetricksVerb(usize),
    WinetricksBox,
    WinetricksInstall,
    ResetPrefix,
    DeletePrefix,
    KeepPrefix,
    // Libraries window.
    LibraryPath,
    LibraryAdd,
    LibraryBrowse,
    LibraryClose,
    /// The Remove button of the library in this slot.
    LibraryRemove(usize),
    // Settings window.
    /// A size-on-screen preset, by its slot in `SCALE_PRESETS`.
    SettingsSize(usize),
    /// A press-debounce preset, by its slot in `DEBOUNCE_PRESETS_MS`.
    SettingsDebounce(usize),
    /// A zip encoding, by its slot in `ENCODINGS`.
    SettingsEncoding(usize),
    SettingsClose,
}

impl Control {
    /// Whether the control lives in the Settings window, where the launcher's Steam buttons
    /// are too.
    pub fn in_settings_window(self) -> bool {
        matches!(
            self,
            Control::SettingsSize(_)
                | Control::SettingsDebounce(_)
                | Control::SettingsEncoding(_)
                | Control::SettingsClose
                | Control::AddLauncherToSteam
                | Control::UpdateLauncherInSteam
                | Control::RemoveLauncherFromSteam
        )
    }

    /// Whether the control lives in the Libraries window.
    pub fn in_library_window(self) -> bool {
        matches!(
            self,
            Control::LibraryPath
                | Control::LibraryAdd
                | Control::LibraryBrowse
                | Control::LibraryClose
                | Control::LibraryRemove(_)
        )
    }
}

pub fn command(context: Context, input: Input) -> Option<Command> {
    use Command::*;
    use Input as I;
    match (context, input) {
        // Window controls work everywhere, including behind a modal.
        (_, I::ToggleFullscreen) => Some(ToggleFullscreen),
        (_, I::ToggleBlur) => Some(ToggleBlur),

        (Context::Chooser, I::Up) => Some(ChooserMove(-1)),
        (Context::Chooser, I::Down) => Some(ChooserMove(1)),
        (Context::Chooser, I::A) => Some(ChooserPick),
        (Context::Chooser, I::B) => Some(ChooserCancel),
        (Context::Chooser, _) => None,

        (Context::Window, I::Up) => Some(Move(Dir::Up)),
        (Context::Window, I::Down) => Some(Move(Dir::Down)),
        (Context::Window, I::Left) => Some(Move(Dir::Left)),
        (Context::Window, I::Right) => Some(Move(Dir::Right)),
        (Context::Window, I::A) => Some(Primary),
        (Context::Window, I::B) => Some(ChooserCancel),
        (Context::Window, _) => None,

        // The D-pad moves focus on both screens; A presses what is focused.
        (_, I::Up) => Some(Move(Dir::Up)),
        (_, I::Down) => Some(Move(Dir::Down)),
        (_, I::Left) => Some(Move(Dir::Left)),
        (_, I::Right) => Some(Move(Dir::Right)),

        (Context::Grid, I::Y) => Some(OpenDetails),
        (Context::Grid, I::B) => Some(ResetFilters),
        (Context::Grid, I::Select) => Some(ToggleFavoritesOnly),

        (Context::Details, I::B) => Some(Back),
        (Context::Details, I::Y) => Some(ToggleFavorite),

        (_, I::A) => Some(Primary),
        (_, I::X) => Some(Reload),
        (_, I::Start) => Some(Rescan),
        _ => None,
    }
}

const STICK_DEAD_ZONE: f32 = 0.5;
const REPEAT_DELAY_MS: u64 = 400;
const REPEAT_INTERVAL_MS: u64 = 120;

/// Turns a stick position (y positive = up, as gilrs reports it) into a direction.
pub fn stick_dir(x: f32, y: f32) -> Option<Dir> {
    if x.abs().max(y.abs()) < STICK_DEAD_ZONE {
        return None;
    }
    Some(if x.abs() > y.abs() {
        if x > 0.0 { Dir::Right } else { Dir::Left }
    } else if y > 0.0 {
        Dir::Up
    } else {
        Dir::Down
    })
}

/// How fast the stick asks a page to scroll, from -1.0 (down, full speed) to 1.0 (up, full
/// speed). Zero inside the dead zone and when the push is mostly sideways.
pub fn stick_scroll(x: f32, y: f32) -> f32 {
    if y.abs() < STICK_DEAD_ZONE || x.abs() > y.abs() {
        return 0.0;
    }
    y.signum() * ((y.abs() - STICK_DEAD_ZONE) / (1.0 - STICK_DEAD_ZONE)).min(1.0)
}

/// Key-repeat for a held direction: fires at once, then after a delay, then steadily.
#[derive(Debug, Default)]
pub struct Repeater {
    held: Option<Dir>,
    next_fire_ms: u64,
}

impl Repeater {
    pub fn update(&mut self, now_ms: u64, held: Option<Dir>) -> Option<Dir> {
        let Some(dir) = held else {
            self.held = None;
            return None;
        };
        if self.held != Some(dir) {
            self.held = Some(dir);
            self.next_fire_ms = now_ms + REPEAT_DELAY_MS;
            return Some(dir);
        }
        if now_ms >= self.next_fire_ms {
            self.next_fire_ms = now_ms + REPEAT_INTERVAL_MS;
            return Some(dir);
        }
        None
    }
}

/// Default for `Debounce`: presses of one button closer together than this are one press
/// seen twice. Held-key repeat is slower than this.
pub const DEFAULT_DEBOUNCE_MS: u64 = 25;
/// The windows offered in the UI; 0 turns it off.
pub const DEBOUNCE_PRESETS_MS: [u64; 5] = [0, 25, 50, 100, 150];

/// Drops a press that repeats the same button within a short window.
///
/// Under Steam Input the physical pad and Steam's virtual copy of it both exist, and each
/// reports every press, so one D-pad tap would otherwise move focus twice. A person cannot
/// tap the same button twice that fast.
#[derive(Debug)]
pub struct Debounce {
    window_ms: u64,
    last: Vec<(Input, u64)>,
}

impl Default for Debounce {
    fn default() -> Self {
        Debounce::new(DEFAULT_DEBOUNCE_MS)
    }
}

impl Debounce {
    pub fn new(window_ms: u64) -> Self {
        Debounce {
            window_ms,
            last: Vec::new(),
        }
    }

    pub fn window(&self) -> u64 {
        self.window_ms
    }

    pub fn set_window(&mut self, window_ms: u64) {
        self.window_ms = window_ms;
    }

    /// True when the press should be acted on.
    pub fn accept(&mut self, now_ms: u64, input: Input) -> bool {
        match self.last.iter_mut().find(|(seen, _)| *seen == input) {
            Some((_, at)) => {
                let fresh = now_ms.saturating_sub(*at) >= self.window_ms;
                if fresh {
                    *at = now_ms;
                }
                fresh
            }
            None => {
                self.last.push((input, now_ms));
                true
            }
        }
    }
}

/// A rectangle on screen, in any consistent units (y grows downward).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Bounds {
    pub left: f32,
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
}

impl Bounds {
    fn center(&self) -> (f32, f32) {
        (
            (self.left + self.right) / 2.0,
            (self.top + self.bottom) / 2.0,
        )
    }
}

/// The item to move focus to from `items[current]` in direction `dir`, or `current` when
/// nothing lies that way.
///
/// Left and right only consider items that share the current item's row, so focus never
/// jumps to a different row sideways. Up and down prefer items that overlap horizontally
/// (the column above or below), and fall back to the nearest item in that direction when
/// none do, which is how focus climbs from a row of tiles to the toolbar. Among the
/// candidates, one in line beats a nearer one that is off to the side.
pub fn spatial_next(items: &[Bounds], current: usize, dir: Dir) -> usize {
    /// How much a sideways offset counts against a candidate, per unit, compared with
    /// distance along the direction of travel.
    const SIDEWAYS_WEIGHT: f32 = 3.0;
    /// The same, for an item that overlaps the current one along the other axis.
    const IN_LINE_WEIGHT: f32 = 0.2;

    let Some(from) = items.get(current) else {
        return current;
    };
    let (cx, cy) = from.center();
    let horizontal = matches!(dir, Dir::Left | Dir::Right);

    let pick = |require_overlap: bool| -> Option<usize> {
        let mut best: Option<(f32, usize)> = None;
        for (i, item) in items.iter().enumerate() {
            if i == current {
                continue;
            }
            let (x, y) = item.center();
            let (along, sideways) = match dir {
                Dir::Right => (x - cx, (y - cy).abs()),
                Dir::Left => (cx - x, (y - cy).abs()),
                Dir::Down => (y - cy, (x - cx).abs()),
                Dir::Up => (cy - y, (x - cx).abs()),
            };
            if along <= 0.0 {
                continue;
            }
            let overlaps = if horizontal {
                item.top < from.bottom && item.bottom > from.top
            } else {
                item.left < from.right && item.right > from.left
            };
            if require_overlap && !overlaps {
                continue;
            }
            // Among items in line, the nearest row wins; being off-centre only breaks ties.
            let weight = if overlaps {
                IN_LINE_WEIGHT
            } else {
                SIDEWAYS_WEIGHT
            };
            let score = along + weight * sideways;
            if best.is_none_or(|(s, _)| score < s) {
                best = Some((score, i));
            }
        }
        best.map(|(_, i)| i)
    };

    // Sideways movement never leaves the row; vertical movement may fall back.
    pick(true)
        .or_else(|| if horizontal { None } else { pick(false) })
        .unwrap_or(current)
}
