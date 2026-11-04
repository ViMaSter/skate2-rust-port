//! `--live-controls-demo two-player-split-screen` (PC port diagnostic): two scripted raw pads
//! play local split-screen co-op from the native main menu. Pad 0 enters gameplay with A once
//! the main menu takes input (row 0, Continue Career with a save); on the
//! gameplay clock pad 1 joins with START, pushes and ollies, opens and steers the pause menu it
//! owns, leaves by holding BACK, and joins again; pad 0 pushes and ollies throughout. Each step's observable result is checked; the
//! demo fails with every step that did not happen.
//!
//! The clock counts every staged tick after the first completed gameplay frame, paused
//! ticks included, so the pause steps keep their schedule.
use skate2_core::{
    SelectedCoopPlayer2, XboxPadSample,
    two_player_session::{PauseOwner, TwoPlayerState},
};

const A: u16 = 0x1000;
const START: u16 = 0x0010;
const BACK: u16 = 0x0020;
const DPAD_DOWN: u16 = 0x0002;

/// The main menu's first tick at which pad 0's A may enter gameplay (as `main-menu-gameplay`),
/// repeated while the menu stays up.
const ENTER_GAMEPLAY: u64 = 600;
const ENTER_RETRY: u64 = 90;
const JOIN: u64 = 100;
const PAUSE: u64 = 600;
const PAUSE_CHECK: u64 = 660;
const PLAYER1_PROBE: u64 = 700;
const PLAYER1_PROBE_CHECK: u64 = 720;
const PLAYER2_NAVIGATE: u64 = 740;
const PLAYER2_NAVIGATE_CHECK: u64 = 760;
const RESUME: u64 = 780;
const RESUME_CHECK: u64 = 800;
/// BACK is held past the router's 120-frame leave hold.
const LEAVE: std::ops::Range<u64> = 850..1000;
const LEAVE_CHECK: u64 = 1050;
const REJOIN: u64 = 1100;
const REJOIN_CHECK: u64 = 1200;
const END: u64 = 1300;

/// Pad 0's pushes, kept clear of the pause steps so they test only ownership.
const PLAYER1_PUSHES: [std::ops::Range<u64>; 6] = [
    30..150,
    300..360,
    450..510,
    820..880,
    1060..1120,
    1200..1260,
];
const PLAYER2_PUSHES: [std::ops::Range<u64>; 3] = [160..280, 340..400, 1150..1250];
const PLAYER2_STEER: std::ops::Range<u64> = 330..450;
/// Each pad's ollie: the right stick held down, then flicked up. Player 2's is on the flat
/// stretch its first push reaches (one on a ramp's lip only crouches), and player 1's line
/// banks before it, so each banked line shows whose scoreboard it reached.
const PLAYER1_OLLIE: u64 = 170;
const PLAYER2_OLLIE: u64 = 284;
const OLLIE_CROUCH: u64 = 12;
const OLLIE_FLICK: u64 = 4;
/// A skater's root moving further than this in one tick was placed (a spawn, a regroup or the
/// career's own teleport), not skated.
const PLACED_STEP: f32 = 1.0;

/// What the session showed after one staged tick.
pub(crate) struct Observation {
    pub(crate) state: TwoPlayerState,
    pub(crate) pause_owner: PauseOwner,
    pub(crate) pause_visible: bool,
    pub(crate) pause_selected: usize,
    pub(crate) coop: Option<SelectedCoopPlayer2>,
}

#[derive(Default)]
pub(crate) struct TwoPlayerDemo {
    menu_ticks: u64,
    /// The menu tick of pad 0's latest A on the main menu.
    menu_press: Option<u64>,
    clock: Option<u64>,
    joined_at: Option<u64>,
    /// Both players' roots after the previous tick with a co-op skater.
    last_roots: Option<[[f32; 3]; 2]>,
    /// Each player's skated ground distance while both were in (placements left out).
    travel: [f32; 2],
    paused_selection: Option<usize>,
    passed: Vec<&'static str>,
    failures: Vec<String>,
    left: bool,
    rejoined: bool,
    split_frames: u64,
    best_team_total: u64,
    /// Each player's best banked co-op score.
    best_banked: [u64; 2],
    /// The clock of each player's first banked line.
    first_banked: [Option<u64>; 2],
    /// Each player's running line score after the previous co-op tick.
    lines: [u32; 2],
}

impl TwoPlayerDemo {
    /// One staged tick; `gameplay` is whether a gameplay frame has completed and `menu_ready`
    /// whether the main menu shows its rows.
    pub(crate) fn advance(&mut self, gameplay: bool, menu_ready: bool) {
        self.menu_ticks += 1;
        self.clock = match self.clock {
            Some(clock) => Some(clock + 1),
            None if gameplay => Some(0),
            None => None,
        };
        if self.clock.is_none() && self.menu_ticks % 300 == 0 {
            eprintln!(
                "two-player demo: menu tick {}, main menu ready {menu_ready}",
                self.menu_ticks
            );
        }
        if self.clock.is_none()
            && menu_ready
            && self.menu_ticks >= ENTER_GAMEPLAY
            && self
                .menu_press
                .is_none_or(|press| self.menu_ticks >= press + ENTER_RETRY)
        {
            eprintln!(
                "two-player demo: player 1 presses A on the main menu at menu tick {}",
                self.menu_ticks
            );
            self.menu_press = Some(self.menu_ticks);
        }
    }

    pub(crate) fn finished(&self) -> bool {
        self.clock.is_some_and(|clock| clock >= END)
    }

    pub(crate) fn player1(&self) -> XboxPadSample {
        let mut sample = idle();
        match self.clock {
            None => {
                if self.menu_press == Some(self.menu_ticks) {
                    sample.buttons = A;
                }
            }
            Some(clock) => {
                if PLAYER1_PUSHES.iter().any(|push| push.contains(&clock)) {
                    sample.buttons = A;
                }
                if clock == PLAYER1_PROBE {
                    sample.buttons = DPAD_DOWN;
                }
                sample.right_y = ollie(clock, PLAYER1_OLLIE);
            }
        }
        sample
    }

    pub(crate) fn player2(&self) -> XboxPadSample {
        let mut sample = idle();
        let Some(clock) = self.clock else {
            return sample;
        };
        if PLAYER2_PUSHES.iter().any(|push| push.contains(&clock)) {
            sample.buttons = A;
        }
        if PLAYER2_STEER.contains(&clock) {
            sample.left_x = 16000;
        }
        sample.right_y = ollie(clock, PLAYER2_OLLIE);
        if matches!(clock, JOIN | PAUSE | RESUME | REJOIN) {
            sample.buttons = START;
        }
        if clock == PLAYER2_NAVIGATE {
            sample.buttons = DPAD_DOWN;
        }
        if LEAVE.contains(&clock) {
            sample.buttons = BACK;
        }
        sample
    }

    /// The session after the tick staged at the current clock.
    pub(crate) fn observe(&mut self, observation: &Observation) {
        let Some(clock) = self.clock else { return };
        if let Some(coop) = observation.coop.as_ref() {
            self.best_team_total = self.best_team_total.max(coop.scoreboard.team_total());
            for (index, score) in coop.scoreboard.players.iter().enumerate() {
                if score.current_line != self.lines[index] {
                    self.lines[index] = score.current_line;
                    eprintln!(
                        "two-player demo {clock}: player {} line {}",
                        index + 1,
                        score.current_line
                    );
                }
                if score.banked > self.best_banked[index] {
                    self.best_banked[index] = score.banked;
                    self.first_banked[index].get_or_insert(clock);
                    eprintln!(
                        "two-player demo {clock}: player {} banked {}",
                        index + 1,
                        score.banked
                    );
                }
            }
            if self.joined_at.is_none() {
                self.joined_at = Some(clock);
                eprintln!(
                    "two-player demo {clock}: player 2 spawned at {:?}, player 1 at {:?}",
                    coop.root, coop.player1_root
                );
            }
            let roots = [coop.player1_root, coop.root];
            if let Some(last) = self.last_roots {
                for (travel, (from, to)) in self.travel.iter_mut().zip(last.into_iter().zip(roots))
                {
                    let step = ground_distance(from, to);
                    if step <= PLACED_STEP {
                        *travel += step;
                    }
                }
            }
            self.last_roots = Some(roots);
        } else {
            self.last_roots = None;
        }
        let owner = observation.pause_owner;
        match clock {
            PAUSE_CHECK => {
                if observation.pause_visible && owner == PauseOwner::Player2 {
                    self.paused_selection = Some(observation.pause_selected);
                    self.pass("player 2's START opened the pause menu it owns");
                } else {
                    self.fail(format!("pause after player 2's START: visible {} owner {owner:?}", observation.pause_visible));
                }
            }
            PLAYER1_PROBE_CHECK => match self.paused_selection {
                Some(selected) if observation.pause_visible && observation.pause_selected == selected => {
                    self.pass("player 1's D-pad did not move player 2's pause menu");
                }
                _ => self.fail(format!(
                    "player 1's D-pad reached player 2's pause menu: visible {} selected {} (was {:?})",
                    observation.pause_visible, observation.pause_selected, self.paused_selection
                )),
            },
            PLAYER2_NAVIGATE_CHECK => match self.paused_selection {
                Some(selected) if observation.pause_visible && observation.pause_selected != selected => {
                    self.pass("player 2's D-pad moved the pause menu's selection");
                }
                _ => self.fail(format!(
                    "player 2's D-pad did not move its pause menu: visible {} selected {} (was {:?})",
                    observation.pause_visible, observation.pause_selected, self.paused_selection
                )),
            },
            RESUME_CHECK => {
                if observation.pause_visible {
                    self.fail("player 2's START did not close its pause menu".to_owned());
                } else {
                    self.pass("player 2's START closed its pause menu");
                }
            }
            LEAVE_CHECK => {
                if matches!(observation.state, TwoPlayerState::Waiting) && observation.coop.is_none() {
                    self.left = true;
                    self.pass("holding BACK took player 2 out; one player again");
                } else {
                    self.fail(format!("player 2 still in after holding BACK: {:?}", observation.state));
                }
            }
            REJOIN_CHECK => {
                if self.left && observation.coop.is_some() && matches!(observation.state, TwoPlayerState::BothActive) {
                    self.rejoined = true;
                    self.pass("player 2 joined again with START");
                } else {
                    self.fail(format!("player 2 did not join again: {:?}, skater {}", observation.state, observation.coop.is_some()));
                }
            }
            _ => {}
        }
    }

    /// One presented frame; `split` is whether it was drawn split-screen.
    pub(crate) fn presented(&mut self, split: bool) {
        if split {
            self.split_frames += 1;
        }
    }

    pub(crate) fn verify(&self) -> Result<String, String> {
        let mut failures = self.failures.clone();
        match self.joined_at {
            Some(clock) if clock > JOIN && clock < PAUSE => {}
            Some(clock) => failures.push(format!("player 2 spawned only at gameplay tick {clock}")),
            None => failures.push("player 2's START never spawned a co-op skater".to_owned()),
        }
        // Each pad's pushes must move its own skater.
        for (index, travel) in self.travel.iter().enumerate() {
            if *travel < 1.0 {
                failures.push(format!("player {} moved only {travel:.2} m", index + 1));
            }
        }
        if self.split_frames == 0 {
            failures.push("no frame was drawn split-screen".to_owned());
        }
        if !self.left || !self.rejoined {
            failures.push("player 2's leave and rejoin did not both happen".to_owned());
        }
        // Each ollie's line banks on its own player's scoreboard.
        match self.first_banked {
            [Some(first), Some(second)] if first > PLAYER1_OLLIE && first < PLAYER2_OLLIE && second > PLAYER2_OLLIE => {}
            banked => failures.push(format!(
                "the ollies' lines did not bank on their own scoreboards: first banks at {banked:?}, ollies at {PLAYER1_OLLIE} and {PLAYER2_OLLIE}"
            )),
        }
        if failures.is_empty() {
            Ok(format!(
                "{}; each ollie banked on its own player's scoreboard; player 1 skated {:.1} m and player 2 {:.1} m; {} split-screen frames; best banked scores P1 {} P2 {}, team {}",
                self.passed.join("; "),
                self.travel[0],
                self.travel[1],
                self.split_frames,
                self.best_banked[0],
                self.best_banked[1],
                self.best_team_total
            ))
        } else {
            Err(failures.join("; "))
        }
    }

    fn pass(&mut self, step: &'static str) {
        eprintln!("two-player demo {}: {step}", self.clock.unwrap_or_default());
        self.passed.push(step);
    }

    fn fail(&mut self, failure: String) {
        eprintln!(
            "two-player demo {}: FAILED {failure}",
            self.clock.unwrap_or_default()
        );
        self.failures.push(failure);
    }
}

/// A connected pad with nothing held (unlike `neutral_demo_sample`, which holds D-pad Up).
fn idle() -> XboxPadSample {
    XboxPadSample {
        buttons: 0,
        left_trigger: 0,
        right_trigger: 0,
        left_x: 0,
        left_y: 0,
        right_x: 0,
        right_y: 0,
    }
}

/// The right stick's Y for an ollie starting at `at`: down while crouching, then up.
fn ollie(clock: u64, at: u64) -> i16 {
    match clock.checked_sub(at) {
        Some(tick) if tick < OLLIE_CROUCH => -32000,
        Some(tick) if tick < OLLIE_CROUCH + OLLIE_FLICK => 32000,
        _ => 0,
    }
}

fn ground_distance(from: [f32; 3], to: [f32; 3]) -> f32 {
    (to[0] - from[0]).hypot(to[2] - from[2])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run_to(demo: &mut TwoPlayerDemo, clock: u64) {
        assert!(
            demo.clock.is_none_or(|now| now <= clock),
            "clock already past {clock}"
        );
        while demo.clock != Some(clock) {
            demo.advance(true, false);
        }
    }

    fn menu_presses(demo: &mut TwoPlayerDemo, ticks: u64, menu_ready: bool) -> Vec<u64> {
        let mut presses = Vec::new();
        for _ in 0..ticks {
            demo.advance(false, menu_ready);
            if demo.player1().buttons == A {
                presses.push(demo.menu_ticks);
            }
        }
        presses
    }

    #[test]
    fn player1_presses_a_only_on_a_ready_main_menu() {
        let mut demo = TwoPlayerDemo::default();
        assert_eq!(menu_presses(&mut demo, ENTER_GAMEPLAY + 200, false), Vec::<u64>::new());
        let mut demo = TwoPlayerDemo::default();
        let presses = menu_presses(&mut demo, ENTER_GAMEPLAY + ENTER_RETRY + 10, true);
        assert_eq!(presses, [ENTER_GAMEPLAY, ENTER_GAMEPLAY + ENTER_RETRY]);
        assert!(demo.clock.is_none());
        demo.advance(true, true);
        assert_eq!(demo.clock, Some(0));
        assert_eq!(demo.player1().buttons, 0);
    }

    #[test]
    fn player2_schedule_presses_start_alone() {
        let mut demo = TwoPlayerDemo::default();
        demo.advance(true, false);
        for clock in [JOIN, PAUSE, RESUME] {
            run_to(&mut demo, clock);
            assert_eq!(demo.player2().buttons, START, "clock {clock}");
            assert_eq!(demo.player1().buttons & START, 0);
        }
        run_to(&mut demo, LEAVE.start + 1);
        assert_eq!(demo.player2().buttons, BACK);
        const { assert!(LEAVE.end - LEAVE.start > 120) };
        run_to(&mut demo, REJOIN);
        assert_eq!(demo.player2().buttons, START);
    }

    #[test]
    fn player1_stays_off_the_pause_menu_but_for_its_probe() {
        for push in PLAYER1_PUSHES {
            assert!(push.end <= PAUSE || push.start > RESUME_CHECK, "{push:?}");
        }
    }

    #[test]
    fn each_ollie_crouches_then_flicks_clear_of_the_pause() {
        assert_eq!(ollie(PLAYER1_OLLIE - 1, PLAYER1_OLLIE), 0);
        assert_eq!(ollie(PLAYER1_OLLIE, PLAYER1_OLLIE), -32000);
        assert_eq!(ollie(PLAYER1_OLLIE + OLLIE_CROUCH, PLAYER1_OLLIE), 32000);
        assert_eq!(
            ollie(PLAYER1_OLLIE + OLLIE_CROUCH + OLLIE_FLICK, PLAYER1_OLLIE),
            0
        );
        for at in [PLAYER1_OLLIE, PLAYER2_OLLIE] {
            assert!(at > JOIN && at + OLLIE_CROUCH + OLLIE_FLICK < PAUSE);
        }
        const { assert!(PLAYER1_OLLIE + OLLIE_CROUCH + OLLIE_FLICK < PLAYER2_OLLIE) };
        let player2 = PLAYER2_OLLIE..PLAYER2_OLLIE + OLLIE_CROUCH + OLLIE_FLICK;
        for busy in PLAYER2_PUSHES.iter().chain([&PLAYER2_STEER]) {
            assert!(
                player2.end <= busy.start || player2.start >= busy.end,
                "{busy:?}"
            );
        }
    }

    #[test]
    fn placements_do_not_count_as_skating() {
        let mut demo = TwoPlayerDemo::default();
        demo.advance(true, false);
        let coop = |player1_root: [f32; 3], root: [f32; 3]| Observation {
            state: TwoPlayerState::BothActive,
            pause_owner: PauseOwner::Player1,
            pause_visible: false,
            pause_selected: 0,
            coop: Some(SelectedCoopPlayer2 {
                player1_root,
                player1_hips: player1_root,
                root,
                hips: root,
                scoreboard: skate2_core::two_player_session::CoopScoreboard::default(),
            }),
        };
        demo.observe(&coop([0.0; 3], [1.5, 0.0, 0.0]));
        demo.observe(&coop([0.5, 0.0, 0.0], [1.5, 0.0, 0.25]));
        demo.observe(&coop([500.0, 0.0, 0.0], [1.5, 0.0, 0.5]));
        assert!((demo.travel[0] - 0.5).abs() < 1e-6);
        assert!((demo.travel[1] - 0.5).abs() < 1e-6);
    }

    #[test]
    fn verify_reports_every_missing_step() {
        let demo = TwoPlayerDemo::default();
        let error = demo.verify().unwrap_err();
        assert!(error.contains("never spawned"));
        assert!(error.contains("split-screen"));
        assert!(error.contains("leave and rejoin"));
        assert!(error.contains("own scoreboards"));
    }

    #[test]
    fn a_line_banked_before_the_second_players_ollie_is_not_theirs() {
        let mut demo = TwoPlayerDemo::default();
        let banked = |player1: u64, player2: u64| {
            let mut scoreboard = skate2_core::two_player_session::CoopScoreboard::default();
            scoreboard.players[0].banked = player1;
            scoreboard.players[1].banked = player2;
            Observation {
                state: TwoPlayerState::BothActive,
                pause_owner: PauseOwner::Player1,
                pause_visible: false,
                pause_selected: 0,
                coop: Some(SelectedCoopPlayer2 {
                    player1_root: [0.0; 3],
                    player1_hips: [0.0; 3],
                    root: [0.0; 3],
                    hips: [0.0; 3],
                    scoreboard,
                }),
            }
        };
        run_to(&mut demo, PLAYER1_OLLIE + 30);
        demo.observe(&banked(2, 2));
        let error = demo.verify().unwrap_err();
        assert!(error.contains("own scoreboards"), "{error}");
        demo.first_banked[1] = Some(PLAYER2_OLLIE + 30);
        assert!(!demo.verify().unwrap_err().contains("own scoreboards"));
    }
}
