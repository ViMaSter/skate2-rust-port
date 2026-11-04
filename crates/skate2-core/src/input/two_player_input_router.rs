//! Two-player input routing and controller detection.
//!
//! Routes input from two controllers to their respective players and detects when
//! player 2 presses START to join an ongoing game.
//!
//! START button is bit 4 (0x0010) in the XboxPadSample.buttons field.

use crate::input_pad::XboxPadSample;
use std::fmt;

/// Button bit flag for START in XboxPadSample.buttons (u16).
/// From `input_pad.rs`: START button is 0x0010.
pub const START_BUTTON_BIT: u16 = 0x0010;

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
}

/// Manages two-player input routing and join detection.
pub struct TwoPlayerInputRouter {
    /// Current state of player 2 join signal.
    state: Player2JoinState,
    /// Frame count of last START press (for debouncing).
    last_start_press_frame: u32,
    /// Current frame count.
    current_frame: u32,
    /// Last frame's START button state (for edge detection).
    last_start_pressed: bool,
}

impl TwoPlayerInputRouter {
    /// Create a new two-player input router in disconnected state.
    pub fn new() -> Self {
        Self {
            state: Player2JoinState::Disconnected,
            last_start_press_frame: 0,
            current_frame: 0,
            last_start_pressed: false,
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

        let result = if start_pressed && !self.last_start_pressed {
            // START pressed (edge: was not pressed last frame) and debounce window passed.
            if self.current_frame > self.last_start_press_frame.saturating_add(3) {
                self.state = Player2JoinState::JoinRequested;
                self.last_start_press_frame = self.current_frame;
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
        self.last_start_press_frame = 0;
        self.last_start_pressed = false;
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
}
