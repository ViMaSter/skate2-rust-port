//! Two-player input routing and controller detection.
//!
//! PC port: detects when player 2 presses START on the second pad to join an ongoing game,
//! and when a joined player 2 holds BACK to drop out. A joined player 2's START is the
//! pause menu's (`TwoPlayerSession::route_frontend_input`), not this router's.
//!
//! START is 0x0010 and BACK 0x0020 in `XboxPadSample::buttons`.

use crate::input_pad::XboxPadSample;
use std::fmt;

/// Button bit flag for START in XboxPadSample.buttons (u16).
/// From `input_pad.rs`: START button is 0x0010.
pub const START_BUTTON_BIT: u16 = 0x0010;

/// Button bit flag for BACK in `XboxPadSample::buttons`.
pub const BACK_BUTTON_BIT: u16 = 0x0020;

/// Frames (at the 60 Hz tick) a joined player 2 holds BACK to drop out: two seconds.
pub const LEAVE_HOLD_FRAMES: u32 = 120;

/// Tracks state of controller connection and player 2 join signal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Player2JoinState {
    /// Player 2 controller not detected or inactive.
    Disconnected,
    /// Player 2 controller detected, waiting for START press.
    Connected,
    /// Player 2 pressed START - join signal active.
    JoinRequested,
}

/// Result of checking player 2 input for join signal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Player2InputCheck {
    /// No player 2 input detected.
    NoInput,
    /// Player 2 controller detected and active.
    Connected,
    /// Player 2 pressed START button (join signal).
    StartPressed,
    /// A joined player 2 held BACK for [`LEAVE_HOLD_FRAMES`] to drop out.
    RemovalRequested,
}

/// Manages two-player input routing and join detection.
pub struct TwoPlayerInputRouter {
    /// Current state of player 2 join signal.
    state: Player2JoinState,
    /// Frame of the last accepted START press (for debouncing); `None` before the first.
    last_start_press_frame: Option<u32>,
    /// Current frame count.
    current_frame: u32,
    /// Last frame's START button state (for edge detection).
    last_start_pressed: bool,
    /// Whether player 2 is currently active in the game.
    player2_is_active: bool,
    /// Consecutive frames a joined player 2 has held BACK; it restarts after a release.
    back_held_frames: u32,
}

impl TwoPlayerInputRouter {
    /// Create a new two-player input router in disconnected state.
    pub fn new() -> Self {
        Self {
            state: Player2JoinState::Disconnected,
            last_start_press_frame: None,
            current_frame: 0,
            last_start_pressed: false,
            player2_is_active: false,
            back_held_frames: 0,
        }
    }

    /// Advance frame count and reset join-signal state (called each frame).
    pub fn advance_frame(&mut self) {
        self.current_frame = self.current_frame.wrapping_add(1);
    }

    /// Check player 2 input sample for connection and START press.
    /// `sample`: XboxPadSample from slot 1 controller (or zeroes if disconnected).
    /// Returns detected input state and updates internal join signal.
    pub fn check_player2_input(&mut self, sample: &XboxPadSample) -> Player2InputCheck {
        // Detect if controller is active (any button pressed or stick/trigger input).
        let start_pressed = (sample.buttons & START_BUTTON_BIT) != 0;
        let any_input = sample.buttons != 0
            || sample.left_trigger > 0
            || sample.right_trigger > 0
            || sample.left_x.abs() > 1000
            || sample.left_y.abs() > 1000
            || sample.right_x.abs() > 1000
            || sample.right_y.abs() > 1000;

        let back_pressed = (sample.buttons & BACK_BUTTON_BIT) != 0;
        self.back_held_frames = if self.player2_is_active && back_pressed {
            self.back_held_frames.saturating_add(1)
        } else {
            0
        };
        let result = if self.back_held_frames == LEAVE_HOLD_FRAMES {
            Player2InputCheck::RemovalRequested
        } else if self.player2_is_active {
            if any_input { Player2InputCheck::Connected } else { Player2InputCheck::NoInput }
        } else if start_pressed && !self.last_start_pressed {
            // START pressed (edge: was not pressed last frame) and debounce window passed.
            if self
                .last_start_press_frame
                .is_none_or(|frame| self.current_frame > frame.saturating_add(3))
            {
                self.state = Player2JoinState::JoinRequested;
                self.last_start_press_frame = Some(self.current_frame);
                Player2InputCheck::StartPressed
            } else {
                // In debounce window, treat as connected.
                self.state = Player2JoinState::Connected;
                Player2InputCheck::Connected
            }
        } else if any_input {
            // Any input detected → controller is connected/active.
            if !matches!(self.state, Player2JoinState::JoinRequested) {
                self.state = Player2JoinState::Connected;
            }
            Player2InputCheck::Connected
        } else {
            // No input detected.
            self.state = Player2JoinState::Disconnected;
            Player2InputCheck::NoInput
        };

        self.last_start_pressed = start_pressed;
        result
    }

    /// Check if player 2 has requested to join (START pressed).
    /// This is a single-frame signal that resets next frame if not consumed.
    pub fn consume_join_request(&mut self) -> bool {
        if matches!(self.state, Player2JoinState::JoinRequested) {
            self.state = Player2JoinState::Connected;
            true
        } else {
            false
        }
    }

    /// Get current player 2 join state.
    pub fn join_state(&self) -> Player2JoinState {
        self.state
    }

    /// Reset router state (e.g., when player 2 leaves).
    pub fn reset(&mut self) {
        self.state = Player2JoinState::Disconnected;
        self.last_start_press_frame = None;
        self.last_start_pressed = false;
        self.player2_is_active = false;
        self.back_held_frames = 0;
    }

    /// Mark player 2 as active (called after P2 spawns successfully).
    pub fn set_player2_active(&mut self, active: bool) {
        self.player2_is_active = active;
    }

    /// Check if player 2 is currently active in the router's state.
    pub fn is_player2_active(&self) -> bool {
        self.player2_is_active
    }
}

impl Default for TwoPlayerInputRouter {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for TwoPlayerInputRouter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TwoPlayerInputRouter")
            .field("state", &self.state)
            .field("current_frame", &self.current_frame)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn router_detects_start_press() {
        let mut router = TwoPlayerInputRouter::new();
        
        // Create input sample with START pressed.
        let sample = XboxPadSample {
            buttons: START_BUTTON_BIT,
            left_trigger: 0,
            right_trigger: 0,
            left_x: 0,
            left_y: 0,
            right_x: 0,
            right_y: 0,
        };
        
        let result = router.check_player2_input(&sample);
        assert_eq!(result, Player2InputCheck::StartPressed);
        assert!(router.consume_join_request());
    }

    #[test]
    fn router_debounces_start_press() {
        let mut router = TwoPlayerInputRouter::new();
        
        let sample = XboxPadSample {
            buttons: START_BUTTON_BIT,
            left_trigger: 0,
            right_trigger: 0,
            left_x: 0,
            left_y: 0,
            right_x: 0,
            right_y: 0,
        };
        
        router.check_player2_input(&sample);
        router.advance_frame();
        
        // Same button still pressed → debounced, not repeated
        let result = router.check_player2_input(&sample);
        assert_eq!(result, Player2InputCheck::Connected); // Not StartPressed
    }

    #[test]
    fn router_detects_any_input_as_connected() {
        let mut router = TwoPlayerInputRouter::new();
        
        let sample = XboxPadSample {
            buttons: 0x1000, // A button
            left_trigger: 0,
            right_trigger: 0,
            left_x: 0,
            left_y: 0,
            right_x: 0,
            right_y: 0,
        };
        
        let result = router.check_player2_input(&sample);
        assert_eq!(result, Player2InputCheck::Connected);
        assert_eq!(router.join_state(), Player2JoinState::Connected);
    }

    #[test]
    fn router_detects_disconnected_state() {
        let mut router = TwoPlayerInputRouter::new();
        router.state = Player2JoinState::Connected;
        
        let sample = XboxPadSample {
            buttons: 0,
            left_trigger: 0,
            right_trigger: 0,
            left_x: 0,
            left_y: 0,
            right_x: 0,
            right_y: 0,
        };
        let result = router.check_player2_input(&sample);
        assert_eq!(result, Player2InputCheck::NoInput);
        assert_eq!(router.join_state(), Player2JoinState::Disconnected);
    }

    #[test]
    fn router_resets_state() {
        let mut router = TwoPlayerInputRouter::new();
        router.state = Player2JoinState::Connected;
        
        router.reset();
        assert_eq!(router.join_state(), Player2JoinState::Disconnected);
    }

    #[test]
    fn join_request_is_single_frame_signal() {
        let mut router = TwoPlayerInputRouter::new();
        
        let sample = XboxPadSample {
            buttons: START_BUTTON_BIT,
            left_trigger: 0,
            right_trigger: 0,
            left_x: 0,
            left_y: 0,
            right_x: 0,
            right_y: 0,
        };
        
        router.check_player2_input(&sample);
        assert!(router.consume_join_request());
        
        // Next frame without pressing START again
        router.advance_frame();
        let released = XboxPadSample {
            buttons: 0,
            left_trigger: 0,
            right_trigger: 0,
            left_x: 0,
            left_y: 0,
            right_x: 0,
            right_y: 0,
        };
        router.check_player2_input(&released);
        assert!(!router.consume_join_request()); // No longer active
    }

    fn pad(buttons: u16) -> XboxPadSample {
        XboxPadSample { buttons, left_trigger: 0, right_trigger: 0, left_x: 0, left_y: 0, right_x: 0, right_y: 0 }
    }

    #[test]
    fn start_while_player2_active_is_left_to_the_pause_menu() {
        let mut router = TwoPlayerInputRouter::new();
        router.set_player2_active(true);
        assert_eq!(router.check_player2_input(&pad(START_BUTTON_BIT)), Player2InputCheck::Connected);
        assert!(!router.consume_join_request());
    }

    #[test]
    fn removal_request_after_holding_back() {
        let mut router = TwoPlayerInputRouter::new();
        router.set_player2_active(true);
        for _ in 1..LEAVE_HOLD_FRAMES {
            router.advance_frame();
            assert_eq!(router.check_player2_input(&pad(BACK_BUTTON_BIT)), Player2InputCheck::Connected);
        }
        router.advance_frame();
        assert_eq!(router.check_player2_input(&pad(BACK_BUTTON_BIT)), Player2InputCheck::RemovalRequested);
        // Holding on does not repeat the request.
        router.advance_frame();
        assert_eq!(router.check_player2_input(&pad(BACK_BUTTON_BIT)), Player2InputCheck::Connected);
    }

    #[test]
    fn releasing_back_restarts_the_hold() {
        let mut router = TwoPlayerInputRouter::new();
        router.set_player2_active(true);
        for _ in 1..LEAVE_HOLD_FRAMES {
            router.check_player2_input(&pad(BACK_BUTTON_BIT));
        }
        router.check_player2_input(&pad(0));
        for _ in 1..LEAVE_HOLD_FRAMES {
            assert_ne!(router.check_player2_input(&pad(BACK_BUTTON_BIT)), Player2InputCheck::RemovalRequested);
        }
    }

    #[test]
    fn back_before_joining_does_not_remove() {
        let mut router = TwoPlayerInputRouter::new();
        for _ in 0..=LEAVE_HOLD_FRAMES {
            assert_ne!(router.check_player2_input(&pad(BACK_BUTTON_BIT)), Player2InputCheck::RemovalRequested);
        }
    }

    #[test]
    fn join_request_when_player2_not_active() {
        let mut router = TwoPlayerInputRouter::new();
        router.set_player2_active(false); // P2 not yet active
        
        let sample = XboxPadSample {
            buttons: START_BUTTON_BIT,
            left_trigger: 0,
            right_trigger: 0,
            left_x: 0,
            left_y: 0,
            right_x: 0,
            right_y: 0,
        };
        
        // START press while P2 not active → StartPressed (join)
        let result = router.check_player2_input(&sample);
        assert_eq!(result, Player2InputCheck::StartPressed);
    }
}

