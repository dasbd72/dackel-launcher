//! Controller focus: what can be focused, where it is on screen, and how it is drawn.
//!
//! Every frame, each focusable widget registers its rectangle here. The next D-pad press
//! uses those rectangles to find the nearest widget in that direction (see
//! `nav::spatial_next`), so the toolbar, the game tiles and the details-page buttons are
//! all reached the same way.

use dlsite_launcher::nav::{Bounds, Control};
use eframe::egui::{Color32, Rect, Response, Stroke, StrokeKind, Ui};

/// Something the controller can put focus on.
#[derive(Clone, Debug, PartialEq)]
pub enum Target {
    /// A game tile in the library grid, by game id.
    Game(String),
    Control(Control),
}

const RING: Color32 = Color32::from_rgb(90, 170, 255);

/// What one frame learned about focus.
pub struct FocusFrame {
    /// The control that has focus, if focus is on a control rather than a game tile.
    pub focused: Option<Control>,
    /// A control the controller pressed with A this frame.
    pub activate: Option<Control>,
    /// Every focusable widget drawn this frame.
    pub items: Vec<(Target, Rect)>,
}

impl FocusFrame {
    pub fn new(focused: Option<Control>, activate: Option<Control>) -> Self {
        FocusFrame {
            focused,
            activate,
            items: Vec::new(),
        }
    }

    /// Registers a control and draws the focus ring if it has focus.
    pub fn register(&mut self, ui: &Ui, id: Control, response: &Response) {
        self.items.push((Target::Control(id), response.rect));
        if self.focused == Some(id) {
            // Drawn inside the control's own rectangle, so it never reaches a neighbor and
            // does not change the layout.
            ui.painter().rect_stroke(
                response.rect,
                6.0,
                Stroke::new(3.0, RING),
                StrokeKind::Inside,
            );
        }
    }

    /// Registers a button-like control; true when it was clicked, or pressed with A while
    /// focused (and `enabled`).
    pub fn pressed(&mut self, ui: &Ui, id: Control, response: &Response, enabled: bool) -> bool {
        self.register(ui, id, response);
        response.clicked() || (enabled && self.activate == Some(id))
    }

    /// True when the controller pressed A on this control. For toggles and checkboxes whose
    /// widget has already handled mouse clicks itself.
    pub fn activated(&self, id: Control) -> bool {
        self.activate == Some(id)
    }
}

pub fn bounds(rect: Rect) -> Bounds {
    Bounds {
        left: rect.left(),
        top: rect.top(),
        right: rect.right(),
        bottom: rect.bottom(),
    }
}
