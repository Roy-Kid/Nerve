//! Only a completed left click toggles the flyout; right click belongs to the menu.

use tray_icon::{MouseButton, MouseButtonState, TrayIconEvent};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PanelIntent {
    Toggle,
    Open,
}

/// Windows sends Down, Up, DoubleClick, Up. The final Up belongs to the
/// double click and must not undo its Open command.
#[derive(Debug, Default)]
pub struct ClickSequence {
    suppress_release: bool,
}

impl ClickSequence {
    pub fn accept(&mut self, event: &TrayIconEvent) -> Option<PanelIntent> {
        if opens_panel(event) {
            self.suppress_release = true;
            return Some(PanelIntent::Open);
        }
        if matches!(
            event,
            TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Down,
                ..
            }
        ) {
            self.suppress_release = false;
        }
        if toggles_panel(event) {
            if std::mem::take(&mut self.suppress_release) {
                return None;
            }
            return Some(PanelIntent::Toggle);
        }
        None
    }
}

pub fn toggles_panel(event: &TrayIconEvent) -> bool {
    matches!(
        event,
        TrayIconEvent::Click {
            button: MouseButton::Left,
            button_state: MouseButtonState::Up,
            ..
        }
    )
}

/// A double left click always opens, never toggles shut.
///
/// Without this the two constituent clicks read as toggle-open then
/// toggle-close, and a user double-clicking to be sure lands on closed.
pub fn opens_panel(event: &TrayIconEvent) -> bool {
    matches!(
        event,
        TrayIconEvent::DoubleClick {
            button: MouseButton::Left,
            ..
        }
    )
}
