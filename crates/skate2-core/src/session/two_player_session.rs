//! PC port: split-screen two-player co-op session state.
//!
//! The reference build has no local split-screen free skate; this whole module is a documented
//! PC extension (`docs/SPLIT_SCREEN_MULTIPLAYER.md`). `TwoPlayerSession` tracks whether the
//! co-op player (pad 1, player index 1) joined, whose pad drives the pause menu, and the shared
//! co-op scoreboard. The HUD, save and career profile stay the first player's.
//!
//! Career challenges share one progression per team through `challenge::coop`
//! (`CoopObjectives`). `TwoPlayerObjective` below is an earlier standalone model of the same
//! rules (one player's pass passes both, both must fail to fail), and the challenge runtime
//! does not use it.

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

/// The player whose pad drives the in-game pause menu (`StartHub`).
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum PauseOwner {
    /// The first player, as in the single-player reference build.
    #[default]
    Player1,
    /// The co-op player, who pressed START while the pause menu was closed.
    Player2,
}

/// The pad index the co-op player plays on.
pub const PLAYER2_CONTROLLER: usize = 1;

/// The players' index of the co-op skater (`PlayerLoader` member and `GameInputPlayer`).
pub const PLAYER2_INDEX: u32 = 1;

/// PC port: how far (metres, on the ground plane) the co-op skater may get from the first
/// player before it is brought back beside them. The world streams collision around the first
/// player's skater only (`docs/evidence/stream-focus.md`, 195 m), so a co-op skater further
/// out would leave the streamed ground.
pub const REGROUP_DISTANCE: f32 = 120.0;

/// PC port: completed ticks the tether waits after a regroup. The co-op skater's published
/// presentation root, which the tether reads, trails its new transform by a few frames.
pub const REGROUP_HOLD_TICKS: u32 = 30;

/// PC port: where a regrouped co-op skater goes. `spawn`'s axes (column-major, columns 0-2)
/// turned about the world's Y axis to the first player's root heading (column 2 on the ground
/// plane), placed at the first player's root 1.5 m along their right axis and 0.5 m up.
#[must_use]
pub fn regroup_transform(spawn: [f32; 16], player1_root: [f32; 16]) -> [f32; 16] {
    let delta = player1_root[8].atan2(player1_root[10]) - spawn[8].atan2(spawn[10]);
    let (sin, cos) = delta.sin_cos();
    let mut out = spawn;
    for column in 0..3 {
        let (x, z) = (spawn[column * 4], spawn[column * 4 + 2]);
        out[column * 4] = cos * x + sin * z;
        out[column * 4 + 2] = -sin * x + cos * z;
    }
    let length = player1_root[0].hypot(player1_root[2]);
    let (right_x, right_z) =
        if length > 1e-4 { (player1_root[0] / length, player1_root[2] / length) } else { (1.0, 0.0) };
    out[12] = player1_root[12] + 1.5 * right_x;
    out[13] = player1_root[13] + 0.5;
    out[14] = player1_root[14] + 1.5 * right_z;
    out
}

/// PC port: one co-op player's trick lines this session, banked as the trick HUD banks them
/// (`TrickBackend::consume`: a completed sequence that did not end in a wipeout).
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct CoopScore {
    /// Every banked line's `finished_sequence_score`, summed.
    pub banked: u64,
    /// The best single banked line.
    pub best_line: u32,
    /// How many lines were banked.
    pub lines: u32,
    /// The line in progress (`hud_score` of the latest frame), 0 between lines.
    pub current_line: u32,
}

impl CoopScore {
    /// One simulation tick's scoring frame for this player.
    pub fn record(&mut self, frame: &crate::ui::trick_backend::PhysicsFrame) {
        if frame.sequence_completed && !frame.just_wiped_out && !frame.interrupted {
            self.banked = self.banked.saturating_add(u64::from(frame.finished_sequence_score));
            self.best_line = self.best_line.max(frame.finished_sequence_score);
            self.lines = self.lines.saturating_add(1);
            self.current_line = 0;
        } else if frame.sequence_completed || frame.interrupted || frame.just_wiped_out {
            self.current_line = 0;
        } else {
            self.current_line = frame.hud_score;
        }
    }
}

/// PC port: the shared co-op scoreboard, both players' banked lines and the team total.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct CoopScoreboard {
    /// Index 0 is the first player, index 1 the co-op player.
    pub players: [CoopScore; 2],
}

impl CoopScoreboard {
    /// Both players' banked points together.
    #[must_use]
    pub fn team_total(&self) -> u64 {
        self.players[0].banked.saturating_add(self.players[1].banked)
    }
}

/// Complete two-player session state.
pub struct TwoPlayerSession {
    /// Current session state (waiting, both active, removing player 2).
    pub state: TwoPlayerState,
    /// Shared objective tracking.
    pub objective: TwoPlayerObjective,
    /// The id `activate_player2` was given. The spawn does not read it: the co-op player plays
    /// the first player's current character.
    pub player2_character_id: Option<u64>,
    /// Whether player 2 needs to be spawned in the world.
    /// Set when activate_player2() is called; cleared when spawn is complete.
    pub player2_spawn_pending: bool,
    /// Whose pad the open pause menu reads.
    pub pause_owner: PauseOwner,
    /// Both players' banked trick lines since the co-op player joined.
    pub scoreboard: CoopScoreboard,
    /// How often the co-op skater was brought back beside the first player.
    pub regroups: u32,
    /// Ticks left before the tether checks again (`REGROUP_HOLD_TICKS` after a regroup).
    pub regroup_hold: u32,
    /// The latest co-op failure reported, so a repeating one prints once.
    pub last_report: Option<String>,
}

impl TwoPlayerSession {
    /// Create a new two-player session in waiting state (player 2 not yet joined).
    pub fn new_waiting() -> Self {
        Self {
            state: TwoPlayerState::Waiting,
            objective: TwoPlayerObjective::new_standard(),
            player2_character_id: None,
            player2_spawn_pending: false,
            pause_owner: PauseOwner::Player1,
            scoreboard: CoopScoreboard::default(),
            regroups: 0,
            regroup_hold: 0,
            last_report: None,
        }
    }

    /// Whether the tether checks this tick; a pending hold counts down instead.
    pub fn take_regroup_check(&mut self) -> bool {
        if self.regroup_hold > 0 {
            self.regroup_hold -= 1;
            return false;
        }
        true
    }

    /// The co-op skater was set beside the first player.
    pub fn note_regroup(&mut self) {
        self.regroups = self.regroups.saturating_add(1);
        self.regroup_hold = REGROUP_HOLD_TICKS;
    }

    /// Prints a co-op failure, once until a different one is reported.
    pub fn report(&mut self, message: String) {
        if self.last_report.as_ref() != Some(&message) {
            eprintln!("Skate 2: co-op: {message}");
            self.last_report = Some(message);
        }
    }

    /// PC port: the front-end input for this frame in split-screen co-op. With the pause menu
    /// closed, the first player's input goes through and the co-op player's START opens the
    /// pause menu for the co-op player; with it open, only its owner's pad reaches it.
    pub fn route_frontend_input(
        &mut self,
        player1: crate::ui::pause_menu::PauseMenuInput,
        player2: crate::ui::pause_menu::PauseMenuInput,
        pause_open: bool,
    ) -> crate::ui::pause_menu::PauseMenuInput {
        if !self.is_player2_active() {
            self.pause_owner = PauseOwner::Player1;
            return player1;
        }
        if pause_open {
            return match self.pause_owner {
                PauseOwner::Player1 => player1,
                PauseOwner::Player2 => player2,
            };
        }
        self.pause_owner = PauseOwner::Player1;
        if player2.start && !player1.start {
            self.pause_owner = PauseOwner::Player2;
            return crate::ui::pause_menu::PauseMenuInput { start: true, ..player1 };
        }
        player1
    }

    /// Activate player 2 (they pressed START to join).
    pub fn activate_player2(&mut self, character_id: u64) {
        self.state = TwoPlayerState::BothActive;
        self.player2_character_id = Some(character_id);
        self.player2_spawn_pending = true;
        self.scoreboard = CoopScoreboard::default();
        self.regroups = 0;
        self.regroup_hold = 0;
    }

    /// The co-op player's join failed to spawn a skater: back to waiting, so a later START
    /// can try again.
    pub fn cancel_player2_join(&mut self) {
        self.complete_remove_player2();
    }

    /// Begin removal of player 2 (pad 1 held BACK past the input router's leave hold).
    pub fn begin_remove_player2(&mut self) {
        self.state = TwoPlayerState::RemovingPlayer2;
    }

    /// Complete removal of player 2 (transition back to single-player).
    pub fn complete_remove_player2(&mut self) {
        self.state = TwoPlayerState::Waiting;
        self.player2_character_id = None;
        self.player2_spawn_pending = false;
        self.pause_owner = PauseOwner::Player1;
    }

    /// Whether player 2's skater is in the world (joined and spawned).
    #[must_use]
    pub fn is_player2_spawned(&self) -> bool {
        self.is_player2_active() && !self.player2_spawn_pending
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
            .field("player2_spawn_pending", &self.player2_spawn_pending)
            .field("pause_owner", &self.pause_owner)
            .field("scoreboard", &self.scoreboard)
            .field("regroups", &self.regroups)
            .field("regroup_hold", &self.regroup_hold)
            .field("last_report", &self.last_report)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_regroup_holds_the_tether_for_its_hold_ticks() {
        let mut session = TwoPlayerSession::new_waiting();
        assert!(session.take_regroup_check());
        session.note_regroup();
        assert_eq!(session.regroups, 1);
        for _ in 0..REGROUP_HOLD_TICKS {
            assert!(!session.take_regroup_check());
        }
        assert!(session.take_regroup_check());
    }

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

    fn pressed_start() -> crate::ui::pause_menu::PauseMenuInput {
        crate::ui::pause_menu::PauseMenuInput { start: true, ..Default::default() }
    }

    #[test]
    fn single_player_frontend_input_is_player1() {
        let mut session = TwoPlayerSession::new_waiting();
        let routed = session.route_frontend_input(crate::ui::pause_menu::PauseMenuInput::default(), pressed_start(), false);
        assert!(!routed.start);
        assert_eq!(session.pause_owner, PauseOwner::Player1);
    }

    #[test]
    fn player2_start_opens_pause_and_owns_it_until_closed() {
        let mut session = TwoPlayerSession::new_waiting();
        session.activate_player2(1);
        let p1 = crate::ui::pause_menu::PauseMenuInput { down: true, ..Default::default() };
        let routed = session.route_frontend_input(p1, pressed_start(), false);
        assert!(routed.start && routed.down);
        assert_eq!(session.pause_owner, PauseOwner::Player2);
        let p2 = crate::ui::pause_menu::PauseMenuInput { select: true, ..Default::default() };
        let routed = session.route_frontend_input(p1, p2, true);
        assert!(routed.select && !routed.down);
        let routed = session.route_frontend_input(p1, p2, false);
        assert!(routed.down && !routed.select);
        assert_eq!(session.pause_owner, PauseOwner::Player1);
    }

    #[test]
    fn player1_keeps_the_pause_it_opened() {
        let mut session = TwoPlayerSession::new_waiting();
        session.activate_player2(1);
        let routed = session.route_frontend_input(pressed_start(), pressed_start(), false);
        assert!(routed.start);
        assert_eq!(session.pause_owner, PauseOwner::Player1);
        let p2 = crate::ui::pause_menu::PauseMenuInput { select: true, ..Default::default() };
        assert!(!session.route_frontend_input(crate::ui::pause_menu::PauseMenuInput::default(), p2, true).select);
    }

    #[test]
    fn two_player_session_lifecycle() {
        let mut session = TwoPlayerSession::new_waiting();
        assert!(!session.is_player2_active());
        assert!(!session.player2_spawn_pending);

        session.activate_player2(0x123);
        assert!(session.is_player2_active());
        assert_eq!(session.player2_character_id, Some(0x123));
        assert!(session.player2_spawn_pending);

        session.begin_remove_player2();
        assert!(matches!(session.state, TwoPlayerState::RemovingPlayer2));

        session.complete_remove_player2();
        assert!(!session.is_player2_active());
        assert_eq!(session.player2_character_id, None);
        assert!(!session.player2_spawn_pending);
    }

    #[test]
    fn coop_score_banks_completed_lines_only() {
        use crate::ui::trick_backend::PhysicsFrame;
        let mut score = CoopScore::default();
        score.record(&PhysicsFrame { hud_score: 120, ..Default::default() });
        assert_eq!(score.current_line, 120);
        score.record(&PhysicsFrame { sequence_completed: true, finished_sequence_score: 300, ..Default::default() });
        assert_eq!((score.banked, score.best_line, score.lines, score.current_line), (300, 300, 1, 0));
        score.record(&PhysicsFrame {
            sequence_completed: true, just_wiped_out: true, finished_sequence_score: 900, ..Default::default()
        });
        score.record(&PhysicsFrame { interrupted: true, hud_score: 50, ..Default::default() });
        assert_eq!((score.banked, score.lines, score.current_line), (300, 1, 0));
        score.record(&PhysicsFrame { sequence_completed: true, finished_sequence_score: 100, ..Default::default() });
        assert_eq!((score.banked, score.best_line, score.lines), (400, 300, 2));
    }

    #[test]
    fn scoreboard_resets_when_player2_joins_again() {
        let mut session = TwoPlayerSession::new_waiting();
        session.activate_player2(1);
        session.scoreboard.players[1].banked = 50;
        session.scoreboard.players[0].banked = 25;
        assert_eq!(session.scoreboard.team_total(), 75);
        session.regroups = 2;
        session.cancel_player2_join();
        assert!(!session.is_player2_active());
        session.activate_player2(1);
        assert_eq!(session.scoreboard, CoopScoreboard::default());
        assert_eq!(session.regroups, 0);
    }

    #[test]
    fn regroup_turns_the_spawn_to_player1_heading_beside_them() {
        let mut spawn = [0.0; 16];
        (spawn[0], spawn[5], spawn[10], spawn[15]) = (1.0, 1.0, 1.0, 1.0);
        let mut player1 = [0.0; 16];
        // Heading +X (yaw 90 degrees): right axis -Z, forward +X.
        (player1[2], player1[5], player1[8], player1[15]) = (-1.0, 1.0, 1.0, 1.0);
        (player1[12], player1[13], player1[14]) = (10.0, 2.0, -4.0);
        let out = regroup_transform(spawn, player1);
        let near = |a: f32, b: f32| (a - b).abs() < 1e-5;
        assert!(near(out[8], 1.0) && near(out[10], 0.0), "forward {:?}", &out[8..11]);
        assert!(near(out[0], 0.0) && near(out[2], -1.0), "right {:?}", &out[0..3]);
        assert!(near(out[5], 1.0));
        assert!(near(out[12], 10.0) && near(out[13], 2.5) && near(out[14], -5.5), "{:?}", &out[12..15]);
    }
}
