//! Split-screen two-player co-op session: synchronized objectives with independent cameras.
//!
//! `TwoPlayerSession` coordinates two skaters in the same world, sharing objective state
//! but with independent input, cameras, and viewports. Player 2 may join at any time by
//! pressing START on a second controller.
//!
//! **Objective Logic:**
//! - Standard pass: one player achieves the goal → challenge passes for both
//! - Fail conditions: only fail if both players satisfy the failure condition
//! - Timed activities: continue until timer expires or both players achieve objective

use std::fmt;

/// Two-player session state tracking.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TwoPlayerState {
    /// Waiting for player 2 to join (player 1 active only).
    Waiting,
    /// Both players active and playing.
    BothActive,
    /// Player 2 is being removed; transitioning to single-player.
    RemovingPlayer2,
}

/// Per-player objective tracking state.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlayerObjectiveState {
    /// Has this player individually completed the objective?
    pub player_completed: bool,
    /// Does this player satisfy a failure condition?
    /// (For fail conditions requiring both players to fail.)
    pub player_failed: bool,
    /// Player's cumulative score/points (if applicable to objective).
    pub player_score: i32,
}

impl Default for PlayerObjectiveState {
    fn default() -> Self {
        Self {
            player_completed: false,
            player_failed: false,
            player_score: 0,
        }
    }
}

/// Unified objective completion state shared by both players.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ObjectiveCompletionState {
    /// Challenge is still in progress.
    InProgress,
    /// Challenge passed: one or both players completed the objective.
    Passed,
    /// Challenge failed: both players hit a failure condition, or time ran out without pass.
    Failed,
}

/// Timed activity configuration.
#[derive(Clone, Copy, Debug)]
pub struct TimedActivityConfig {
    /// Total time allowed (in seconds).
    pub duration: f32,
    /// Behavior when time runs out:
    /// - If true: require both players to have completed objective to pass
    /// - If false: standard pass (one player completes = pass)
    pub require_both_on_timeout: bool,
}

/// Shared two-player objective state.
#[derive(Clone, Debug)]
pub struct TwoPlayerObjective {
    /// Player 1's individual state.
    pub player1: PlayerObjectiveState,
    /// Player 2's individual state.
    pub player2: PlayerObjectiveState,
    /// Cumulative completion state.
    pub completion: ObjectiveCompletionState,
    /// Time elapsed (if timed activity).
    pub elapsed_time: f32,
    /// Timed activity configuration (if applicable).
    pub timed_config: Option<TimedActivityConfig>,
}

impl TwoPlayerObjective {
    /// Create new objective state for a standard (non-timed) challenge.
    pub fn new_standard() -> Self {
        Self {
            player1: PlayerObjectiveState::default(),
            player2: PlayerObjectiveState::default(),
            completion: ObjectiveCompletionState::InProgress,
            elapsed_time: 0.0,
            timed_config: None,
        }
    }

    /// Create new objective state for a timed challenge.
    pub fn new_timed(duration: f32, require_both_on_timeout: bool) -> Self {
        Self {
            player1: PlayerObjectiveState::default(),
            player2: PlayerObjectiveState::default(),
            completion: ObjectiveCompletionState::InProgress,
            elapsed_time: 0.0,
            timed_config: Some(TimedActivityConfig {
                duration,
                require_both_on_timeout,
            }),
        }
    }

    /// Update objective state: mark player as completed.
    /// Returns new completion state.
    pub fn mark_player_completed(&mut self, player_index: u32) -> ObjectiveCompletionState {
        match player_index {
            0 => self.player1.player_completed = true,
            1 => self.player2.player_completed = true,
            _ => {}
        }
        self.update_completion()
    }

    /// Update objective state: mark player as failed.
    /// Returns new completion state.
    pub fn mark_player_failed(&mut self, player_index: u32) -> ObjectiveCompletionState {
        match player_index {
            0 => self.player1.player_failed = true,
            1 => self.player2.player_failed = true,
            _ => {}
        }
        self.update_completion()
    }

    /// Update player score.
    pub fn set_player_score(&mut self, player_index: u32, score: i32) {
        match player_index {
            0 => self.player1.player_score = score,
            1 => self.player2.player_score = score,
            _ => {}
        }
    }

    /// Advance time for timed activities.
    pub fn advance_time(&mut self, dt: f32) -> ObjectiveCompletionState {
        if let Some(config) = self.timed_config {
            self.elapsed_time += dt;
            if self.elapsed_time >= config.duration {
                self.elapsed_time = config.duration;
                // Time expired: check pass conditions
                if config.require_both_on_timeout {
                    // Both players must have completed
                    if self.player1.player_completed && self.player2.player_completed {
                        self.completion = ObjectiveCompletionState::Passed;
                    } else {
                        self.completion = ObjectiveCompletionState::Failed;
                    }
                } else {
                    // Standard: one player completes = pass
                    if self.player1.player_completed || self.player2.player_completed {
                        self.completion = ObjectiveCompletionState::Passed;
                    } else {
                        self.completion = ObjectiveCompletionState::Failed;
                    }
                }
            }
        }
        self.completion
    }

    /// Recompute completion state based on current player states.
    fn update_completion(&mut self) -> ObjectiveCompletionState {
        // Check if time-based pass condition already determined outcome
        if matches!(self.completion, ObjectiveCompletionState::Passed | ObjectiveCompletionState::Failed) {
            return self.completion;
        }

        // Standard pass: one player completes → pass
        if self.player1.player_completed || self.player2.player_completed {
            self.completion = ObjectiveCompletionState::Passed;
        }
        // Both players failed → fail
        else if self.player1.player_failed && self.player2.player_failed {
            self.completion = ObjectiveCompletionState::Failed;
        }

        self.completion
    }

    /// Check if challenge is complete (pass or fail).
    pub fn is_complete(&self) -> bool {
        !matches!(self.completion, ObjectiveCompletionState::InProgress)
    }

    /// Remaining time (in seconds) for timed activities.
    pub fn remaining_time(&self) -> Option<f32> {
        self.timed_config.map(|config| (config.duration - self.elapsed_time).max(0.0))
    }
}

/// Complete two-player session state.
pub struct TwoPlayerSession {
    /// Current session state (waiting, both active, removing player 2).
    pub state: TwoPlayerState,
    /// Shared objective tracking.
    pub objective: TwoPlayerObjective,
    /// Player 2 character ID (if loaded).
    pub player2_character_id: Option<u64>,
}

impl TwoPlayerSession {
    /// Create a new two-player session in waiting state (player 2 not yet joined).
    pub fn new_waiting() -> Self {
        Self {
            state: TwoPlayerState::Waiting,
            objective: TwoPlayerObjective::new_standard(),
            player2_character_id: None,
        }
    }

    /// Activate player 2 (they pressed START to join).
    pub fn activate_player2(&mut self, character_id: u64) {
        self.state = TwoPlayerState::BothActive;
        self.player2_character_id = Some(character_id);
    }

    /// Begin removal of player 2 (from either player's pause menu).
    pub fn begin_remove_player2(&mut self) {
        self.state = TwoPlayerState::RemovingPlayer2;
    }

    /// Complete removal of player 2 (transition back to single-player).
    pub fn complete_remove_player2(&mut self) {
        self.state = TwoPlayerState::Waiting;
        self.player2_character_id = None;
    }

    /// Check if player 2 is currently active.
    pub fn is_player2_active(&self) -> bool {
        matches!(self.state, TwoPlayerState::BothActive)
    }
}

impl fmt::Debug for TwoPlayerSession {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TwoPlayerSession")
            .field("state", &self.state)
            .field("objective", &self.objective)
            .field("player2_character_id", &self.player2_character_id)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn standard_objective_one_player_pass() {
        let mut obj = TwoPlayerObjective::new_standard();
        let completion = obj.mark_player_completed(0);
        assert_eq!(completion, ObjectiveCompletionState::Passed);
    }

    #[test]
    fn standard_objective_both_players_fail_to_fail() {
        let mut obj = TwoPlayerObjective::new_standard();
        obj.mark_player_failed(0);
        assert_eq!(obj.completion, ObjectiveCompletionState::InProgress);
        let completion = obj.mark_player_failed(1);
        assert_eq!(completion, ObjectiveCompletionState::Failed);
    }

    #[test]
    fn timed_objective_timeout_standard_logic() {
        let mut obj = TwoPlayerObjective::new_timed(5.0, false);
        obj.advance_time(3.0);
        assert_eq!(obj.completion, ObjectiveCompletionState::InProgress);
        obj.advance_time(3.0); // total 6.0, over limit
        assert_eq!(obj.completion, ObjectiveCompletionState::Failed);
    }

    #[test]
    fn timed_objective_timeout_require_both() {
        let mut obj = TwoPlayerObjective::new_timed(5.0, true);
        obj.mark_player_completed(0);
        obj.advance_time(6.0); // time expired
        assert_eq!(obj.completion, ObjectiveCompletionState::Failed); // only one completed
    }

    #[test]
    fn timed_objective_both_pass_before_timeout() {
        let mut obj = TwoPlayerObjective::new_timed(5.0, true);
        obj.mark_player_completed(0);
        obj.mark_player_completed(1);
        assert_eq!(obj.completion, ObjectiveCompletionState::Passed);
        obj.advance_time(1.0); // doesn't matter now
        assert_eq!(obj.completion, ObjectiveCompletionState::Passed);
    }

    #[test]
    fn two_player_session_lifecycle() {
        let mut session = TwoPlayerSession::new_waiting();
        assert!(!session.is_player2_active());

        session.activate_player2(0x123);
        assert!(session.is_player2_active());
        assert_eq!(session.player2_character_id, Some(0x123));

        session.begin_remove_player2();
        assert!(matches!(session.state, TwoPlayerState::RemovingPlayer2));

        session.complete_remove_player2();
        assert!(!session.is_player2_active());
        assert_eq!(session.player2_character_id, None);
    }
}
