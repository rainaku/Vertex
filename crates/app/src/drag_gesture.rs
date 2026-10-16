//! Platform-independent gesture transitions, shared by hooks and regression tests.
/// Mouse hooks fire before Explorer finishes its OLE Drop call. A release over
/// the webview must wait for OLE Drop/Leave; closing it would destroy the target.
#[derive(Default)]
pub struct DropSession {
    active: bool,
    over_webview: bool,
    released: bool,
}

impl DropSession {
    pub const fn new() -> Self {
        Self {
            active: false,
            over_webview: false,
            released: false,
        }
    }
    pub fn begin(&mut self) {
        *self = Self {
            active: true,
            ..Self::new()
        };
    }
    pub fn enter(&mut self) {
        if self.active {
            self.over_webview = true;
        }
    }
    /// Return true only when the gesture ended outside the OLE drop target.
    pub fn release(&mut self) -> bool {
        if !self.active {
            return false;
        }
        self.released = true;
        if self.over_webview {
            false
        } else {
            self.finish();
            true
        }
    }
    pub fn leave(&mut self) -> bool {
        self.over_webview = false;
        if self.active && self.released {
            self.finish();
            true
        } else {
            false
        }
    }
    pub fn finish(&mut self) {
        *self = Self::new();
    }
}

#[derive(Default)]
pub struct DragGesture {
    state: State,
}

#[derive(Default)]
enum State {
    #[default]
    Idle,
    Armed {
        x: i32,
        y: i32,
    },
    Dragging,
    Suppressed,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Action {
    None,
    Show { right: bool },
    End,
    Cancel,
}

impl DragGesture {
    pub fn press(&mut self, x: i32, y: i32, allowed: bool) {
        if !matches!(self.state, State::Idle) {
            return;
        }
        self.state = if allowed {
            State::Armed { x, y }
        } else {
            State::Suppressed
        };
    }

    pub fn motion(&mut self, x: i32, y: i32, allowed: bool) -> Action {
        let State::Armed {
            x: start_x,
            y: start_y,
        } = self.state
        else {
            return Action::None;
        };
        if !allowed {
            self.state = State::Suppressed;
            return Action::None;
        }
        let dx = i64::from(x) - i64::from(start_x);
        let dy = i64::from(y) - i64::from(start_y);
        if dx * dx + dy * dy < 64 {
            return Action::None;
        }
        self.state = State::Dragging;
        Action::Show { right: dx >= 0 }
    }

    pub fn release(&mut self) -> Action {
        let action = if matches!(self.state, State::Dragging) {
            Action::End
        } else {
            Action::None
        };
        self.state = State::Idle;
        action
    }

    pub fn escape(&mut self) -> Action {
        if matches!(self.state, State::Idle) {
            return Action::None;
        }
        let action = if matches!(self.state, State::Dragging) {
            Action::Cancel
        } else {
            Action::None
        };
        self.state = State::Suppressed;
        action
    }

    pub fn needs_motion(&self) -> bool {
        matches!(self.state, State::Armed { .. })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn release_over_webview_waits_for_ole_drop_in_either_event_order() {
        let mut session = DropSession::new();
        session.begin();
        session.enter();
        assert!(!session.release());
        session.finish(); // OLE Drop
        assert!(!session.release());
        assert!(!session.leave());
        session.begin();
        session.enter();
        session.finish(); // OLE Drop before hook release is delivered
        assert!(!session.release());
    }
    #[test]
    fn release_outside_closes_once_and_leave_after_release_closes_once() {
        let mut session = DropSession::new();
        session.begin();
        session.enter();
        assert!(!session.leave());
        assert!(session.release());
        assert!(!session.release());
        session.begin();
        session.enter();
        assert!(!session.release());
        assert!(session.leave());
        assert!(!session.leave());
    }
    #[test]
    fn escape_suppresses_reopening_until_the_primary_button_is_released() {
        let mut gesture = DragGesture::default();
        gesture.press(100, 100, true);
        assert_eq!(gesture.motion(104, 100, true), Action::None);
        assert_eq!(gesture.motion(108, 100, true), Action::Show { right: true });
        assert_eq!(gesture.escape(), Action::Cancel);
        gesture.press(200, 200, true);
        assert_eq!(gesture.motion(300, 300, true), Action::None);
        assert_eq!(gesture.release(), Action::None);
        gesture.press(200, 200, true);
        assert_eq!(
            gesture.motion(190, 200, true),
            Action::Show { right: false }
        );
        assert_eq!(gesture.release(), Action::End);
    }

    #[test]
    fn busy_settings_or_fullscreen_suppresses_the_entire_gesture() {
        let mut gesture = DragGesture::default();
        gesture.press(10, 10, false);
        assert_eq!(gesture.motion(200, 200, true), Action::None);
        gesture.release();
        gesture.press(10, 10, true);
        assert_eq!(gesture.motion(200, 200, false), Action::None);
        assert_eq!(gesture.motion(300, 300, true), Action::None);
    }
}
