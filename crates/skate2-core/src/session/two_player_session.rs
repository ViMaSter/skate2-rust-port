//! PC port: split-screen two-player co-op session state.
//!
//! The reference build has no local split-screen free skate; this whole module is a documented
//! PC extension (`docs/SPLIT_SCREEN_MULTIPLAYER.md`). `TwoPlayerSession` tracks whether the
//! co-op player (pad 1, player index 1) joined, how the two players' pause menus pause the game
//! ([`CoopPause`]), whose pad drives the screen they share (the Challenge Map and the full-window
//! screens), the teleport offers and the co-op player's own teleport, and the shared co-op
//! scoreboard. The HUD, save and career profile stay the first player's.
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

/// One of the two split-screen co-op players.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum CoopPlayer {
    /// The first player, on pad 0, as in the single-player reference build.
    #[default]
    Player1,
    /// The co-op player, on pad 1.
    Player2,
}

impl CoopPlayer {
    /// Both players, the first first.
    pub const ALL: [Self; 2] = [Self::Player1, Self::Player2];

    /// The other player.
    #[must_use]
    pub const fn other(self) -> Self {
        match self {
            Self::Player1 => Self::Player2,
            Self::Player2 => Self::Player1,
        }
    }

    /// The player's index into per-player arrays: 0 for the first player, 1 for the co-op player.
    #[must_use]
    pub const fn slot(self) -> usize {
        match self {
            Self::Player1 => 0,
            Self::Player2 => 1,
        }
    }
}

/// PC port: how the game pauses while two players share it. Each player has their own pause
/// menu; one open menu only stops that player's skater (it brakes, as a skater without
/// `PlayerUI`'s focus does), and the game stops once both are open, or while a full-window
/// screen (Game Settings, the Trick Guide, the Blog, ...) is up.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum CoopPause {
    /// Not a two-player game: the reference's single pause (`kPAUSE_STATE_FRONTEND` while the
    /// one pause menu is open).
    #[default]
    Off,
    /// Both players are skating.
    Running,
    /// One player's menu is open. The game runs and that player's skater brakes.
    Single,
    /// Both menus are open, or a full-window screen is. Nothing simulates.
    Full,
}

impl CoopPause {
    /// The pause state for a frame. `player1_menu` and `player2_menu` are the players' own pause
    /// menus being open (a menu stays open under the Challenge Map and the screens it opens), and
    /// `full_window_screen` is a screen that fills the window, which the other player cannot
    /// play beside.
    #[must_use]
    pub const fn of(two_players: bool, player1_menu: bool, player2_menu: bool, full_window_screen: bool) -> Self {
        if !two_players {
            Self::Off
        } else if full_window_screen || (player1_menu && player2_menu) {
            Self::Full
        } else if player1_menu || player2_menu {
            Self::Single
        } else {
            Self::Running
        }
    }

    /// Whether the player's skater brakes: their own menu is open while the other player plays on.
    #[must_use]
    pub const fn brakes(self, player_menu: bool) -> bool {
        matches!(self, Self::Single) && player_menu
    }
}

/// PC port: how long (completed simulation ticks, 60 a second) a teleport offer waits for the
/// other player to take it: one minute. A hypothesis, not a measured reference value.
pub const TELEPORT_OFFER_TICKS: u32 = 3600;

/// PC port: how close (metres, on the ground plane) the players are before a teleport offer is
/// moot: they stand together already.
pub const TELEPORT_OFFER_NEAR: f32 = 25.0;

/// PC port: how long (completed simulation ticks) a new offer stands whatever the distance. The
/// teleporter's published root follows their teleport a few ticks late, and would read as still
/// standing with the other player. Two seconds, a hypothesis.
pub const TELEPORT_OFFER_GRACE_TICKS: u32 = 120;

/// PC port: how many frames the co-op player's teleport waits for its destination to stream in
/// before it gives up (thirty seconds at 60 frames a second).
pub const PLAYER2_TRAVEL_FRAMES: u32 = 1800;

/// PC port: how many frames the stream focus stays on a teleport destination after the co-op
/// skater was set there, if its published root has not followed by then (two seconds).
pub const PLAYER2_SETTLE_FRAMES: u32 = 120;

/// PC port: how close (metres) the co-op skater's published root is to its teleport destination
/// before the stream focus follows the skater again.
pub const PLAYER2_SETTLE_RADIUS: f32 = 5.0;

/// PC port: how many frames the co-op player's pause menu waits for its screen's intro to
/// complete (the host acknowledges it, as for the first player's) before the menu takes input
/// anyway: a host that never does must not leave the menu stuck. Two seconds.
pub const PLAYER2_INTRO_FALLBACK_FRAMES: u32 = 120;

/// PC port: one player's teleport (Challenge Map) offered to the other, who may join it with
/// LB + D-pad right.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TeleportOffer {
    /// Who teleported. The offer's destination is where they are, wherever they have skated to.
    pub from: CoopPlayer,
    /// Completed ticks left before the offer lapses.
    pub ticks_left: u32,
}

/// PC port: the co-op player's own Challenge Map teleport. The first player's travel fades the
/// window, holds the simulation and streams the destination in; the co-op player's does not stop
/// the first, so the destination streams in as a second stream focus and the skater is set
/// there once it is resident.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Player2Travel {
    /// Where the co-op skater is set (a column-major transform).
    pub transform: [f32; 16],
    /// Frames waited so far: for the destination to stream in, then for the skater's published
    /// root to follow the placement.
    pub frames: u32,
    /// The skater was set at the destination. Its stream focus stays there until the published
    /// root follows, so the destination's collections are not released under the skater.
    pub placed: bool,
}

impl Player2Travel {
    /// The destination's position.
    #[must_use]
    pub const fn destination(&self) -> [f32; 3] {
        [self.transform[12], self.transform[13], self.transform[14]]
    }
}

/// What one completed tick did to the co-op player's teleport.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Player2TravelStep {
    /// No teleport under way.
    Idle,
    /// The destination has not streamed in yet.
    Waiting,
    /// The destination is resident: the skater is set at this transform now.
    Place([f32; 16]),
    /// The destination never streamed in and the teleport was dropped.
    TimedOut,
    /// The skater is set; the stream focus waits for its published root to follow.
    Holding,
    /// The published root followed (or the wait ran out): the stream focus is the skater's again.
    Settled,
}

/// The pad index the co-op player plays on.
pub const PLAYER2_CONTROLLER: usize = 1;

/// The players' index of the co-op skater (`PlayerLoader` member and `GameInputPlayer`).
pub const PLAYER2_INDEX: u32 = 1;

/// PC port: where a skater joining the other player goes (and where the co-op skater spawns).
/// `spawn`'s axes (column-major, columns 0-2) turned about the world's Y axis to the other
/// player's root heading (column 2 on the ground plane), placed at their root 1.5 m along their
/// right axis and 0.5 m up.
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
    /// How the game pauses this frame ([`CoopPause`]).
    pub pause: CoopPause,
    /// Whose pad drives the screen both players' menus share (the Challenge Map, Game Settings
    /// and the other screens a hub opens), the player whose menu opened it.
    pub screen_owner: CoopPlayer,
    /// The teleport offered to each player, by [`CoopPlayer::slot`].
    pub offers: [Option<TeleportOffer>; 2],
    /// Frames the co-op player's pause menu has waited for its intro to complete
    /// ([`PLAYER2_INTRO_FALLBACK_FRAMES`]).
    pub player2_intro_frames: u32,
    /// The first player's travel (their Challenge Map's pick) is to be offered to the co-op
    /// player once they arrive.
    pub offer_on_arrival: bool,
    /// The co-op player's teleport while its destination streams in.
    pub player2_travel: Option<Player2Travel>,
    /// Both players' banked trick lines since the co-op player joined.
    pub scoreboard: CoopScoreboard,
    /// How often a player joined the other's teleport (LB + D-pad right).
    pub joins: u32,
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
            pause: CoopPause::Off,
            screen_owner: CoopPlayer::Player1,
            offers: [None; 2],
            player2_intro_frames: 0,
            offer_on_arrival: false,
            player2_travel: None,
            scoreboard: CoopScoreboard::default(),
            joins: 0,
            last_report: None,
        }
    }

    /// A player joined the other's teleport.
    pub fn note_join(&mut self) {
        self.joins = self.joins.saturating_add(1);
    }

    /// Prints a co-op failure, once until a different one is reported.
    pub fn report(&mut self, message: String) {
        if self.last_report.as_ref() != Some(&message) {
            eprintln!("Skate 2: co-op: {message}");
            self.last_report = Some(message);
        }
    }

    /// A player's teleport (their Challenge Map's pick) is offered to the other player, who may
    /// join it. It replaces an earlier offer to that player, and ends one the teleporter had:
    /// they chose a destination of their own.
    pub fn offer_join(&mut self, teleporter: CoopPlayer) {
        self.offers[teleporter.other().slot()] = Some(TeleportOffer { from: teleporter, ticks_left: TELEPORT_OFFER_TICKS });
        self.offers[teleporter.slot()] = None;
    }

    /// The teleport offered to `player`, if any.
    #[must_use]
    pub fn offer_for(&self, player: CoopPlayer) -> Option<TeleportOffer> {
        self.offers[player.slot()]
    }

    /// `player` joins the teleport offered to them: the offer ends and is returned.
    pub fn take_offer(&mut self, player: CoopPlayer) -> Option<TeleportOffer> {
        self.offers[player.slot()].take()
    }

    /// One completed simulation tick: offers lapse after [`TELEPORT_OFFER_TICKS`], and end once
    /// the players stand together (`together`), which a new offer ignores for
    /// [`TELEPORT_OFFER_GRACE_TICKS`].
    pub fn tick_offers(&mut self, together: bool) {
        for offer in &mut self.offers {
            let Some(open) = offer else { continue };
            let age = TELEPORT_OFFER_TICKS.saturating_sub(open.ticks_left);
            open.ticks_left = open.ticks_left.saturating_sub(1);
            if open.ticks_left == 0 || (together && age >= TELEPORT_OFFER_GRACE_TICKS) {
                *offer = None;
            }
        }
    }

    /// The co-op player's teleport to `transform` starts, replacing one under way. It is their
    /// own choice of destination, so an offer to them ends.
    pub fn begin_player2_travel(&mut self, transform: [f32; 16]) {
        self.player2_travel = Some(Player2Travel { transform, frames: 0, placed: false });
        self.offers[CoopPlayer::Player2.slot()] = None;
    }

    /// Whether the co-op skater is waiting for its teleport destination to stream in (it brakes
    /// meanwhile and cannot take an offer).
    #[must_use]
    pub fn player2_travelling(&self) -> bool {
        self.player2_travel.is_some_and(|travel| !travel.placed)
    }

    /// The co-op teleport's destination while its stream focus is held there: from the start of
    /// the teleport until the skater's published root has followed the placement.
    #[must_use]
    pub fn player2_travel_destination(&self) -> Option<[f32; 3]> {
        self.player2_travel.as_ref().map(Player2Travel::destination)
    }

    /// One completed tick of the co-op player's teleport. `resident` is whether the
    /// destination's collections are registered and `root` where the co-op skater's presentation
    /// last published its root.
    pub fn tick_player2_travel(&mut self, resident: bool, root: Option<[f32; 3]>) -> Player2TravelStep {
        let Some(travel) = self.player2_travel.as_mut() else {
            return Player2TravelStep::Idle;
        };
        if !travel.placed {
            if resident {
                travel.placed = true;
                travel.frames = 0;
                return Player2TravelStep::Place(travel.transform);
            }
            travel.frames = travel.frames.saturating_add(1);
            if travel.frames >= PLAYER2_TRAVEL_FRAMES {
                self.player2_travel = None;
                return Player2TravelStep::TimedOut;
            }
            return Player2TravelStep::Waiting;
        }
        travel.frames = travel.frames.saturating_add(1);
        let destination = travel.destination();
        let followed = root.is_some_and(|root| {
            let delta: [f32; 3] = std::array::from_fn(|axis| root[axis] - destination[axis]);
            delta.iter().map(|d| d * d).sum::<f32>() <= PLAYER2_SETTLE_RADIUS * PLAYER2_SETTLE_RADIUS
        });
        if followed || travel.frames >= PLAYER2_SETTLE_FRAMES {
            self.player2_travel = None;
            return Player2TravelStep::Settled;
        }
        Player2TravelStep::Holding
    }

    /// Activate player 2 (they pressed START to join).
    pub fn activate_player2(&mut self, character_id: u64) {
        self.state = TwoPlayerState::BothActive;
        self.player2_character_id = Some(character_id);
        self.player2_spawn_pending = true;
        self.scoreboard = CoopScoreboard::default();
        self.joins = 0;
        self.reset_shared_screens();
    }

    /// No co-op screen, offer or travel outlives the co-op player, or the game it was in.
    pub fn reset_shared_screens(&mut self) {
        self.pause = CoopPause::Off;
        self.screen_owner = CoopPlayer::Player1;
        self.offers = [None; 2];
        self.player2_intro_frames = 0;
        self.offer_on_arrival = false;
        self.player2_travel = None;
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
        self.reset_shared_screens();
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
            .field("pause", &self.pause)
            .field("screen_owner", &self.screen_owner)
            .field("offers", &self.offers)
            .field("player2_intro_frames", &self.player2_intro_frames)
            .field("offer_on_arrival", &self.offer_on_arrival)
            .field("player2_travel", &self.player2_travel)
            .field("scoreboard", &self.scoreboard)
            .field("joins", &self.joins)
            .field("last_report", &self.last_report)
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
    fn pause_state_follows_the_open_menus() {
        // Not two players: the reference's pause, whatever the menus say.
        assert_eq!(CoopPause::of(false, true, true, true), CoopPause::Off);
        assert_eq!(CoopPause::of(true, false, false, false), CoopPause::Running);
        assert_eq!(CoopPause::of(true, true, false, false), CoopPause::Single);
        assert_eq!(CoopPause::of(true, false, true, false), CoopPause::Single);
        assert_eq!(CoopPause::of(true, true, true, false), CoopPause::Full);
        // A full-window screen stops both players, even from one player's menu.
        assert_eq!(CoopPause::of(true, true, false, true), CoopPause::Full);
        assert_eq!(CoopPause::of(true, false, true, true), CoopPause::Full);
    }

    #[test]
    fn only_the_player_with_an_open_menu_brakes_in_a_single_pause() {
        let single = CoopPause::of(true, true, false, false);
        assert!(single.brakes(true));
        assert!(!single.brakes(false));
        for other in [CoopPause::Off, CoopPause::Running, CoopPause::Full] {
            assert!(!other.brakes(true));
        }
    }

    #[test]
    fn a_teleport_is_offered_to_the_other_player_only() {
        let mut session = TwoPlayerSession::new_waiting();
        session.activate_player2(1);
        session.offer_join(CoopPlayer::Player1);
        assert_eq!(session.offer_for(CoopPlayer::Player1), None);
        assert_eq!(
            session.offer_for(CoopPlayer::Player2),
            Some(TeleportOffer { from: CoopPlayer::Player1, ticks_left: TELEPORT_OFFER_TICKS })
        );
        // The other player teleports: their own pick replaces the offer to them, and the first player
        // is offered theirs.
        session.offer_join(CoopPlayer::Player2);
        assert_eq!(session.offer_for(CoopPlayer::Player2), None);
        assert_eq!(session.offer_for(CoopPlayer::Player1).map(|offer| offer.from), Some(CoopPlayer::Player2));
        assert!(session.take_offer(CoopPlayer::Player1).is_some());
        assert!(session.take_offer(CoopPlayer::Player1).is_none());
    }

    #[test]
    fn an_offer_lapses_with_time_or_when_the_players_stand_together() {
        let mut session = TwoPlayerSession::new_waiting();
        session.activate_player2(1);
        session.offer_join(CoopPlayer::Player1);
        for _ in 0..TELEPORT_OFFER_TICKS - 1 {
            session.tick_offers(false);
        }
        assert_eq!(session.offer_for(CoopPlayer::Player2).map(|offer| offer.ticks_left), Some(1));
        session.tick_offers(false);
        assert_eq!(session.offer_for(CoopPlayer::Player2), None);

        // Standing together does not end a new offer at once: the teleporter's root lags.
        session.offer_join(CoopPlayer::Player2);
        for _ in 0..TELEPORT_OFFER_GRACE_TICKS {
            session.tick_offers(true);
            assert!(session.offer_for(CoopPlayer::Player1).is_some());
        }
        session.tick_offers(true);
        assert_eq!(session.offer_for(CoopPlayer::Player1), None);
    }

    #[test]
    fn the_co_op_players_own_teleport_ends_an_offer_to_them_and_leaves_with_them() {
        let mut session = TwoPlayerSession::new_waiting();
        session.activate_player2(1);
        session.offer_join(CoopPlayer::Player1);
        session.begin_player2_travel([1.0; 16]);
        assert!(session.player2_travelling());
        assert_eq!(session.offer_for(CoopPlayer::Player2), None);
        session.pause = CoopPause::Single;
        session.screen_owner = CoopPlayer::Player2;
        session.offer_join(CoopPlayer::Player1);
        session.begin_remove_player2();
        session.complete_remove_player2();
        assert!(!session.player2_travelling());
        assert_eq!((session.pause, session.screen_owner, session.offers), (CoopPause::Off, CoopPlayer::Player1, [None; 2]));
    }

    #[test]
    fn the_co_op_teleport_waits_for_its_destination_then_holds_the_focus_while_settling() {
        let mut session = TwoPlayerSession::new_waiting();
        session.activate_player2(1);
        assert_eq!(session.tick_player2_travel(true, None), Player2TravelStep::Idle);
        let mut transform = [0.0; 16];
        (transform[12], transform[13], transform[14]) = (100.0, 5.0, -40.0);
        session.begin_player2_travel(transform);
        assert!(session.player2_travelling());
        assert_eq!(session.player2_travel_destination(), Some([100.0, 5.0, -40.0]));
        assert_eq!(session.tick_player2_travel(false, None), Player2TravelStep::Waiting);
        assert!(session.player2_travelling());
        // Resident: the skater is set once, then the focus holds until its root follows.
        assert_eq!(session.tick_player2_travel(true, Some([0.0; 3])), Player2TravelStep::Place(transform));
        assert!(!session.player2_travelling());
        assert_eq!(session.player2_travel_destination(), Some([100.0, 5.0, -40.0]));
        assert_eq!(session.tick_player2_travel(true, Some([0.0; 3])), Player2TravelStep::Holding);
        assert_eq!(session.tick_player2_travel(true, Some([101.0, 5.0, -41.0])), Player2TravelStep::Settled);
        assert_eq!(session.player2_travel_destination(), None);
        assert_eq!(session.tick_player2_travel(true, None), Player2TravelStep::Idle);
    }

    #[test]
    fn the_co_op_teleport_gives_up_and_the_held_focus_is_released_after_a_wait() {
        let mut session = TwoPlayerSession::new_waiting();
        session.activate_player2(1);
        session.begin_player2_travel([0.0; 16]);
        for _ in 0..PLAYER2_TRAVEL_FRAMES - 1 {
            assert_eq!(session.tick_player2_travel(false, None), Player2TravelStep::Waiting);
        }
        assert_eq!(session.tick_player2_travel(false, None), Player2TravelStep::TimedOut);
        assert!(session.player2_travel.is_none());

        session.begin_player2_travel([0.0; 16]);
        assert!(matches!(session.tick_player2_travel(true, None), Player2TravelStep::Place(_)));
        for _ in 0..PLAYER2_SETTLE_FRAMES - 1 {
            assert_eq!(session.tick_player2_travel(true, Some([500.0; 3])), Player2TravelStep::Holding);
        }
        assert_eq!(session.tick_player2_travel(true, Some([500.0; 3])), Player2TravelStep::Settled);
        assert!(session.player2_travel.is_none());
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
        session.note_join();
        session.note_join();
        session.cancel_player2_join();
        assert!(!session.is_player2_active());
        session.activate_player2(1);
        assert_eq!(session.scoreboard, CoopScoreboard::default());
        assert_eq!(session.joins, 0);
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
