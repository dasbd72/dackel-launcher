//! Controller/keyboard navigation as pure functions, so it is testable without a window.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dir {
    Up,
    Down,
    Left,
    Right,
}

/// Moves focus through a grid of `len` items laid out `columns` wide.
/// Edges stop rather than wrap; moving down into a short last row lands on its last item.
pub fn move_focus(index: usize, len: usize, columns: usize, dir: Dir) -> usize {
    if len == 0 {
        return 0;
    }
    let columns = columns.max(1);
    let index = index.min(len - 1);
    match dir {
        Dir::Left if !index.is_multiple_of(columns) => index - 1,
        Dir::Right if !(index + 1).is_multiple_of(columns) && index + 1 < len => index + 1,
        Dir::Up if index >= columns => index - columns,
        Dir::Down if index + columns < len => index + columns,
        Dir::Down if index / columns < (len - 1) / columns => len - 1,
        _ => index,
    }
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
    LeftBumper,
    RightBumper,
    LeftTrigger,
    RightTrigger,
    Start,
    Select,
}

/// What the input means depends on what is on screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Context {
    Grid,
    Details,
    Chooser,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
    Move(Dir),
    /// Launch the game, or extract / pick an exe when it is not launchable yet.
    Primary,
    Reload,
    OpenDetails,
    Back,
    ToggleFavorite,
    ChooseExe,
    ReExtract,
    CycleSort(i32),
    CycleEncoding(i32),
    Rescan,
    ToggleFavoritesOnly,
    ResetFilters,
    ChooserMove(i32),
    ChooserPick,
    ChooserCancel,
}

pub fn command(context: Context, input: Input) -> Option<Command> {
    use Command::*;
    use Input as I;
    match (context, input) {
        (Context::Chooser, I::Up) => Some(ChooserMove(-1)),
        (Context::Chooser, I::Down) => Some(ChooserMove(1)),
        (Context::Chooser, I::A) => Some(ChooserPick),
        (Context::Chooser, I::B) => Some(ChooserCancel),
        (Context::Chooser, _) => None,

        (Context::Grid, I::Up) => Some(Move(Dir::Up)),
        (Context::Grid, I::Down) => Some(Move(Dir::Down)),
        (Context::Grid, I::Left) => Some(Move(Dir::Left)),
        (Context::Grid, I::Right) => Some(Move(Dir::Right)),
        (Context::Grid, I::Y) => Some(OpenDetails),
        (Context::Grid, I::B) => Some(ResetFilters),
        (Context::Grid, I::LeftBumper) => Some(CycleSort(-1)),
        (Context::Grid, I::RightBumper) => Some(CycleSort(1)),
        (Context::Grid, I::LeftTrigger) => Some(CycleEncoding(-1)),
        (Context::Grid, I::RightTrigger) => Some(CycleEncoding(1)),
        (Context::Grid, I::Select) => Some(ToggleFavoritesOnly),

        (Context::Details, I::B) => Some(Back),
        (Context::Details, I::Y) => Some(ToggleFavorite),
        (Context::Details, I::LeftBumper) => Some(ChooseExe),
        (Context::Details, I::RightBumper) => Some(ReExtract),

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
