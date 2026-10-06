//! Focus changes arrive after viewport commands, not in the frame issuing them.

use std::time::{Duration, Instant};

#[derive(Debug, Default)]
pub struct Visibility {
    pub visible: bool,
    focused_once: bool,
    hidden_at: Option<Instant>,
}

impl Visibility {
    pub fn show(&mut self) {
        // Even an already visible window may be reactivated by the tray menu.
        // Its previous focus observation must not dismiss the new request.
        self.focused_once = false;
        self.visible = true;
    }

    pub fn hide(&mut self, now: Instant) {
        self.visible = false;
        self.focused_once = false;
        self.hidden_at = Some(now);
    }

    pub fn can_reopen(&self, now: Instant) -> bool {
        !self
            .hidden_at
            .is_some_and(|at| now.duration_since(at) < Duration::from_millis(200))
    }

    /// Ignore the old hidden window's focus state until Windows acknowledges focus.
    pub fn lost_focus(&mut self, focused: Option<bool>) -> bool {
        if !self.visible {
            return false;
        }
        match focused {
            Some(true) => {
                self.focused_once = true;
                false
            }
            Some(false) => self.focused_once,
            None => false,
        }
    }
}
