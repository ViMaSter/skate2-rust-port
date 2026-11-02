//! Two-player input routing and controller detection.
//!
//! Routes input from two controllers to their respective players and detects when
//! player 2 presses START to join an ongoing game.

use std::fmt;

/// Button index for START in the 24-element input array.
/// From `input_pad.rs`: buttons 0-9 are discrete buttons (A, B, X, Y, LB, RB, back, start, guide, pad_click)
/// START is button index 7.
pub const START_BUTTON_INDEX: usize = 7;

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
}

impl TwoPlayerInputRouter {
    /// Create a new two-player input router in disconnected state.
    pub fn new() -> Self {
        Self {
            state: Player2JoinState::Disconnected,
            last_start_press_frame: 0,
            current_frame: 0,
        }
    }

    /// Advance frame count and reset join-signal state (called each frame).
    pub fn advance_frame(&mut self) {
        self.current_frame = self.current_frame.wrapping_add(1);
        // JoinRequested is a single-frame signal; drop it next frame if not consumed
        if matches!(self.state, Player2JoinState::Connected) {
            self.state = Player2JoinState::Connected;
        }
    }

    /// Check player 2 input sample for connection and START press.
    /// `input_sample`: 24-element array from pad device (or zeroes if disconnected).
    /// Returns detected input state and updates internal join signal.
    pub fn check_player2_input(&mut self, input_sample: &[f32; 24]) -> Player2InputCheck {
        // Detect if controller is active (any non-zero input or specific button press).
        // We check for START press specifically.
        let start_pressed = input_sample.get(START_BUTTON_INDEX).copied().unwrap_or(0.0) > 0.5;

        if start_pressed && self.current_frame > self.last_start_press_frame.saturating_add(3) {
            // START pressed and sufficient frames since last press (debounce).
            self.state = Player2JoinState::JoinRequested;
            self.last_start_press_frame = self.current_frame;
            Player2InputCheck::StartPressed
        } else if any_button_pressed(input_sample) {
            // Any input detected → controller is connected/active.
            if !matches!(self.state, Player2JoinState::JoinRequested) {
                self.state = Player2JoinState::Connected;
            }
            Player2InputCheck::Connected
        } else {
            // No input detected.
            self.state = Player2JoinState::Disconnected;
            Player2InputCheck::NoInput
        }
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
    }
}

impl Default for TwoPlayerInputRouter {
    fn default() -> Self {
        Self::new()
    }
}

/// Check if any button is pressed in an input sample.
fn any_button_pressed(input_sample: &[f32; 24]) -> bool {
    // Buttons are indices 0-9 and 12-15
    input_sample[0..10].iter().any(|&v| v > 0.5) || 
    input_sample[12..16].iter().any(|&v| v > 0.5) ||
    // Also check triggers and sticks
    input_sample[10] > 0.1 || input_sample[11] > 0.1 || // triggers
    input_sample[16..].iter().any(|&v| v.abs() > 0.2) // sticks
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
        
        // Create input sample with START pressed (index 7).
        let mut sample = [0.0; 24];
        sample[7] = 1.0; // START button
        
        let result = router.check_player2_input(&sample);
        assert_eq!(result, Player2InputCheck::StartPressed);
        assert!(router.consume_join_request());
    }

    #[test]
    fn router_debounces_start_press() {
        let mut router = TwoPlayerInputRouter::new();
        
        let mut sample = [0.0; 24];
        sample[7] = 1.0; // START button
        
        router.check_player2_input(&sample);
        router.advance_frame();
        
        // Same button still pressed → debounced, not repeated
        let result = router.check_player2_input(&sample);
        assert_eq!(result, Player2InputCheck::Connected); // Not StartPressed
    }

    #[test]
    fn router_detects_any_input_as_connected() {
        let mut router = TwoPlayerInputRouter::new();
        
        let mut sample = [0.0; 24];
        sample[0] = 1.0; // A button
        
        let result = router.check_player2_input(&sample);
        assert_eq!(result, Player2InputCheck::Connected);
        assert_eq!(router.join_state(), Player2JoinState::Connected);
    }

    #[test]
    fn router_detects_disconnected_state() {
        let mut router = TwoPlayerInputRouter::new();
        router.state = Player2JoinState::Connected;
        
        let sample = [0.0; 24];
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
        
        let mut sample = [0.0; 24];
        sample[7] = 1.0; // START button
        
        router.check_player2_input(&sample);
        assert!(router.consume_join_request());
        
        // Next frame without consuming START again
        router.advance_frame();
        router.check_player2_input(&[0.0; 24]); // Release START
        assert!(!router.consume_join_request()); // No longer active
    }
}
