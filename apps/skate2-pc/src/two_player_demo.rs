//! `--live-controls-demo two-player-split-screen` (PC port diagnostic): two scripted raw pads
//! play local split-screen co-op from the native main menu. Pad 0 enters gameplay with A once
//! the main menu takes input (row 0, Continue Career with a save); on the gameplay clock pad 1
//! joins with START, pushes and ollies, and the pads then walk the co-op front end: each pad's
//! own pause menu stops only its own skater while the other keeps skating and both open stop
//! the game, Boot Player 2 on either pad's menu takes the co-op player out (START joins again),
//! and each pad's Challenge Map teleports only its owner, whom the other follows with LB + D-pad
//! right. Each step's observable result is checked; the demo fails with every step that did not
//! happen.
//!
//! The clock counts every staged tick after the first completed gameplay frame, paused
//! ticks included, so the pause steps keep their schedule. The map steps wait on the game's
//! loads, so each follows what the session shows, within a deadline; each pad walks its map's
//! lists (Locations, then a group of them) by what the lists show, so the save's own rows decide
//! where it goes. With `SKATE2_TWO_PLAYER_TRACE` set, both players' speeds, screens and places
//! are traced to stderr every [`TRACE_EVERY`] ticks.
use std::cmp::Ordering;

use skate2_core::{
    SelectedCoopPlayer2, XboxPadSample,
    challenge::stuff_menu::{ChallengeMapView, MenuText, ROOT_HAL},
    two_player_session::{CoopPause, CoopPlayer, TwoPlayerState},
    ui::pause_menu::{PauseMenuOption, PauseMenuSnapshot},
};

const A: u16 = 0x1000;
const START: u16 = 0x0010;
const DPAD_UP: u16 = 0x0001;
const DPAD_DOWN: u16 = 0x0002;
const DPAD_RIGHT: u16 = 0x0008;
const BACK: u16 = 0x0020;
const LEFT_SHOULDER: u16 = 0x0100;

/// The main menu's first tick at which pad 0's A may enter gameplay (as `main-menu-gameplay`),
/// repeated while the menu stays up.
const ENTER_GAMEPLAY: u64 = 600;
const ENTER_RETRY: u64 = 90;
const JOIN: u64 = 100;
/// Player 2's menu alone: their START opens it, their skater brakes and player 1's skates on.
const PLAYER2_PAUSE: u64 = 600;
const PLAYER2_PAUSE_CHECK: u64 = 690;
const PLAYER1_PROBE: u64 = 700;
const PLAYER1_PROBE_CHECK: u64 = 720;
const PLAYER2_NAVIGATE: u64 = 740;
const PLAYER2_NAVIGATE_CHECK: u64 = 760;
const PLAYER2_RESUME: u64 = 780;
const PLAYER2_RESUME_CHECK: u64 = 800;
/// Player 1's menu alone, then both menus: only both stop the game. Player 1 brakes from the
/// speed their line gave them (about 10 m/s) in about a hundred ticks, so the check waits for
/// that; player 2 pushes through it and is skating when the second menu opens.
const PLAYER1_PAUSE: u64 = 820;
const PLAYER1_PAUSE_CHECK: u64 = 940;
const BOTH_PAUSE: u64 = 950;
/// The game stops from the tick both menus are open: the check's window is the 40 ticks before
/// it, which start after the second menu opened.
const BOTH_PAUSE_CHECK: u64 = 1000;
const BOTH_RESUME_PLAYER2: u64 = 1010;
const BOTH_RESUME_CHECK: u64 = 1050;
const PLAYER1_RESUME: u64 = 1060;
const PLAYER1_RESUME_CHECK: u64 = 1100;
/// Player 1 boots player 2 from their menu, and player 2 joins again.
const PLAYER1_BOOT_OPEN: u64 = 1120;
/// A menu takes input from its intro's end: from here a press every [`MENU_STEP`] ticks walks it.
const PLAYER1_BOOT_WALK: u64 = 1220;
const PLAYER1_BOOT_CHECK: u64 = 1420;
const REJOIN: u64 = 1440;
const REJOIN_CHECK: u64 = 1540;
/// Player 2 leaves from their own menu, and joins again.
const PLAYER2_BOOT_OPEN: u64 = 1560;
const PLAYER2_BOOT_WALK: u64 = 1660;
const PLAYER2_BOOT_CHECK: u64 = 1860;
const REJOIN_AGAIN: u64 = 1880;
const REJOIN_AGAIN_CHECK: u64 = 1980;
/// Player 2 leaves by holding BACK (past the router's 120-frame hold), and joins again.
const LEAVE: std::ops::Range<u64> = 2000..2140;
const LEAVE_CHECK: u64 = 2180;
const REJOIN_LEAVE: u64 = 2200;
const REJOIN_LEAVE_CHECK: u64 = 2300;
/// The Challenge Map steps start here.
const MAP_START: u64 = 2320;
/// A map step that has not happened by this many ticks after it began has failed.
const MAP_DEADLINE: u64 = 2400;
/// The ticks the demo runs on after its last step.
const FINISH_DELAY: u64 = 60;
/// The demo ends here whatever the steps did.
const MAX_CLOCK: u64 = MAP_START + 5 * MAP_DEADLINE;
/// A press on a menu every this many ticks, which its animations keep up with.
const MENU_STEP: u64 = 20;
/// A START press on a hub that has not opened every this many ticks.
const START_RETRY: u64 = 60;
/// A press on the Challenge Map after it opened or a press that descended a level, whose rows
/// animate in, and between presses that only move within a list.
const MAP_PRESS: u64 = 90;
const MAP_MOVE: u64 = MENU_STEP;
/// The most presses a Challenge Map takes before the demo gives up on it.
const MAP_PRESS_LIMIT: usize = 40;
/// Where each pad's Challenge Map goes: the root's Locations, then a group of them (by
/// `stuff_menu::CATEGORY_HAL`), whose first location is the destination, and Teleport. The
/// groups differ so the second teleport is not to where the first one went.
const MAP_ROUTES: [[&str; 2]; 2] = [
    [
        "ID_STUFFMENU_CATEGORY_LOCATIONS",
        "ID_STUFFMENU_CATEGORY_LANDMARK_PLAZAS",
    ],
    [
        "ID_STUFFMENU_CATEGORY_LOCATIONS",
        "ID_STUFFMENU_CATEGORY_LANDMARKS",
    ],
];
/// The LB + D-pad right chord is held this many ticks.
const CHORD_TICKS: u8 = 3;
/// How close a player who joined a teleport must end to the one who made it.
const JOIN_NEAR: f32 = 12.0;
/// A teleporting player ends further than this from where their map closed.
const TELEPORTED: f32 = 20.0;
/// The ticks an offer may take to reach the other player after a teleport landed.
const OFFER_WAIT: u64 = 300;

/// Pad 0's pushes, kept clear of the pause steps that are not about it. The last two keep
/// player 1 skating through player 2's menu and up to their own pause. Pad 1's last keeps
/// player 2 skating through player 1's pause and up to both menus opening.
const PLAYER1_PUSHES: [std::ops::Range<u64>; 5] =
    [30..150, 300..360, 450..510, 610..700, 710..818];
const PLAYER2_PUSHES: [std::ops::Range<u64>; 4] = [160..280, 340..400, 540..598, 850..930];
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
/// A skater that braked is slower than this fraction of the speed it paused at.
const BRAKED_TO: f32 = 0.5;
/// The speed (m/s) a skater must have had for its braking to show.
const MIN_SPEED: f32 = 0.5;
/// A speed is the ground covered over this many ticks.
const SPEED_TICKS: u64 = 10;
const SPEED_SECONDS: f32 = 10.0 / 60.0;
/// The trace line's interval in ticks.
const TRACE_EVERY: u64 = 10;
/// A stopped game moves a skater no further than this over a window.
const FROZEN: f32 = 0.01;
/// The least a skating player moves in a window of the other's menu being open.
const SKATED_ON: f32 = 2.0;

/// What the session showed after one staged tick.
pub(crate) struct Observation {
    pub(crate) state: TwoPlayerState,
    pub(crate) pause: CoopPause,
    /// Each player's pause menu, the first player's then the second's.
    pub(crate) menus: [PauseMenuSnapshot; 2],
    /// Whose Challenge Map is open, if one is.
    pub(crate) map_owner: Option<CoopPlayer>,
    /// What that map shows, which its owner's pad navigates by.
    pub(crate) map: Option<MapSight>,
    /// Whether each player has a teleport of the other's to join.
    pub(crate) offers: [bool; 2],
    /// The teleports joined so far.
    pub(crate) joins: u32,
    pub(crate) coop: Option<SelectedCoopPlayer2>,
}

/// What a Challenge Map shows, as the pad that opened it reads it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct MapSight {
    /// The list's title: the root's, or a category's HAL id.
    pub(crate) title: String,
    /// The list's rows: categories' HAL ids, or locations' names.
    pub(crate) rows: Vec<String>,
    pub(crate) selected: usize,
    /// The info component (and its Teleport option) is up.
    pub(crate) info: bool,
}

impl MapSight {
    pub(crate) fn of(view: &ChallengeMapView) -> Self {
        let text = |text: &MenuText| match text {
            MenuText::Empty => String::new(),
            MenuText::Copied(text) | MenuText::Formatted(text) => text.clone(),
            MenuText::Category { hal, .. } => (*hal).to_owned(),
        };
        Self {
            title: text(&view.title),
            rows: view.rows.iter().map(text).collect(),
            selected: view.selected,
            info: view.info.is_some(),
        }
    }
}

/// The screens as the last tick left them, which the pads answer.
#[derive(Clone, Copy, Default)]
struct Screens {
    menus: [PauseMenuSnapshot; 2],
    offers: [bool; 2],
}

/// The Challenge Map steps, in order. The player named is the one who teleports.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum MapStep {
    /// The scheduled steps come first.
    #[default]
    Waiting,
    /// The player opens their Challenge Map and teleports, the other skating on.
    Teleports(CoopPlayer),
    /// The other player joins the teleport with LB + D-pad right.
    Joins(CoopPlayer),
    Done,
}

/// What the current Challenge Map step has seen so far.
#[derive(Clone, Copy, Debug, Default)]
struct MapProgress {
    /// The clock the step began and the joins taken by then.
    since: u64,
    joins: u32,
    /// The clock the teleporting player's map was first seen open and seen closed again.
    opened: Option<u64>,
    closed: Option<u64>,
    /// Both players' roots when the map closed, and the clock the teleporting player's root
    /// was seen [`TELEPORTED`] metres from there.
    origin: Option<[[f32; 3]; 2]>,
    landed: Option<u64>,
    /// Whether the game stopped while the map was open before its pick (the pick's own travel
    /// fades the window and stops it), and whether the other player was placed anywhere (a
    /// teleport of theirs) since it opened.
    stopped: bool,
    other_placed: bool,
    /// The map presses made, the clock the next may be made, and the clock of the press that
    /// asked for the teleport (A on the info component).
    presses: usize,
    next_press: u64,
    picked: Option<u64>,
    /// The ticks the join chord is still held, and the clock the join was taken.
    chord_left: u8,
    taken: Option<u64>,
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
    /// Each player's skated ground distance per tick by clock (a placement counts nothing).
    steps: Vec<[f32; 2]>,
    /// How far each player's root was placed on the latest tick, if it was.
    placements: [Option<f32>; 2],
    screens: Screens,
    /// What the open Challenge Map showed after the last tick.
    sight: Option<MapSight>,
    /// What each pad presses on the tick being staged, where the screens decide it.
    presses: [u16; 2],
    paused_selection: Option<usize>,
    passed: Vec<String>,
    failures: Vec<String>,
    /// Whether player 2 was taken out, and let in again, by each way out: player 1's menu,
    /// their own menu and holding BACK.
    left: [bool; 3],
    rejoined: [bool; 3],
    split_frames: u64,
    best_team_total: u64,
    /// Each player's best banked co-op score.
    best_banked: [u64; 2],
    /// The clock of each player's first banked line.
    first_banked: [Option<u64>; 2],
    /// Each player's running line score after the previous co-op tick.
    lines: [u32; 2],
    map_step: MapStep,
    map: MapProgress,
    done_at: Option<u64>,
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
        if let Some(clock) = self.clock {
            self.presses = self.reactive_presses(clock);
        }
    }

    pub(crate) fn finished(&self) -> bool {
        self.clock.is_some_and(|clock| {
            clock >= MAX_CLOCK || self.done_at.is_some_and(|done| clock >= done + FINISH_DELAY)
        })
    }

    /// What each pad presses on the tick about to be staged where the screens decide it: the
    /// boot walks on the pause menus, and the Challenge Map steps.
    fn reactive_presses(&mut self, clock: u64) -> [u16; 2] {
        let mut presses = [0; 2];
        if (PLAYER1_BOOT_WALK..PLAYER1_BOOT_CHECK).contains(&clock) && clock % MENU_STEP == 0 {
            presses[0] = boot_press(&self.screens.menus[0]);
        }
        if (PLAYER2_BOOT_WALK..PLAYER2_BOOT_CHECK).contains(&clock) && clock % MENU_STEP == 0 {
            presses[1] = boot_press(&self.screens.menus[1]);
        }
        match self.map_step {
            MapStep::Teleports(mover) => {
                presses[mover.other().slot()] = A;
                presses[mover.slot()] = self.map_press(mover, clock);
            }
            MapStep::Joins(mover) => {
                let follower = mover.other();
                if self.map.chord_left == 0
                    && self.map.taken.is_none()
                    && self.screens.offers[follower.slot()]
                {
                    self.map.chord_left = CHORD_TICKS;
                }
                if self.map.chord_left > 0 {
                    self.map.chord_left -= 1;
                    presses[follower.slot()] = LEFT_SHOULDER | DPAD_RIGHT;
                }
            }
            MapStep::Waiting | MapStep::Done => {}
        }
        presses
    }

    /// The teleporting player's press: START until their hub is up, A on its Map row, then
    /// their route through the Challenge Map's lists to Teleport.
    fn map_press(&mut self, mover: CoopPlayer, clock: u64) -> u16 {
        if self.map.closed.is_some() {
            return 0;
        }
        if self.map.opened.is_some() {
            return self.navigate_map(mover, clock);
        }
        let menu = &self.screens.menus[mover.slot()];
        if menu.visible {
            if clock % MENU_STEP != 0 {
                0
            } else if selected_option(menu) == Some(PauseMenuOption::Map) {
                A
            } else {
                DPAD_DOWN
            }
        } else if clock >= self.map.since && (clock - self.map.since) % START_RETRY == 0 {
            START
        } else {
            0
        }
    }

    /// One press of `mover`'s walk through their open Challenge Map, by the lists it shows. A
    /// map without the row the route needs fails the step.
    fn navigate_map(&mut self, mover: CoopPlayer, clock: u64) -> u16 {
        if clock < self.map.next_press || self.map.picked.is_some() || self.map.presses >= MAP_PRESS_LIMIT {
            return 0;
        }
        let Some(sight) = self.sight.clone() else {
            return 0;
        };
        let press = match map_navigation(&sight, &MAP_ROUTES[mover.slot()]) {
            Ok(press) => press,
            Err(reason) => {
                self.fail(format!("{}'s Challenge Map: {reason}", player_name(mover)));
                self.finish(clock);
                return 0;
            }
        };
        eprintln!(
            "two-player demo {clock}: {} presses {} at row {} of {:?} in {:?}{}",
            player_name(mover),
            button_name(press),
            sight.selected,
            sight.rows,
            sight.title,
            if sight.info { " (the info component)" } else { "" },
        );
        self.map.presses += 1;
        self.map.next_press = clock + if press == A { MAP_PRESS } else { MAP_MOVE };
        if press == A && sight.info {
            self.map.picked = Some(clock);
        }
        press
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
                if matches!(clock, PLAYER1_PAUSE | PLAYER1_RESUME | PLAYER1_BOOT_OPEN) {
                    sample.buttons = START;
                }
                sample.buttons |= self.presses[0];
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
        if matches!(
            clock,
            JOIN | PLAYER2_PAUSE
                | PLAYER2_RESUME
                | BOTH_PAUSE
                | BOTH_RESUME_PLAYER2
                | REJOIN
                | PLAYER2_BOOT_OPEN
                | REJOIN_AGAIN
                | REJOIN_LEAVE
        ) {
            sample.buttons = START;
        }
        if clock == PLAYER2_NAVIGATE {
            sample.buttons = DPAD_DOWN;
        }
        if LEAVE.contains(&clock) {
            sample.buttons = BACK;
        }
        sample.buttons |= self.presses[1];
        sample
    }

    /// The session after the tick staged at the current clock.
    pub(crate) fn observe(&mut self, observation: &Observation) {
        let Some(clock) = self.clock else { return };
        let offers_before = self.screens.offers;
        self.screens = Screens {
            menus: observation.menus,
            offers: observation.offers,
        };
        self.sight.clone_from(&observation.map);
        self.track_scores(clock, observation.coop.as_ref());
        self.track_roots(clock, observation.coop.as_ref());
        self.trace(clock, observation, offers_before);
        self.scheduled_checks(clock, observation);
        self.map_checks(clock, observation);
    }

    /// With `SKATE2_TWO_PLAYER_TRACE` set: both players' speeds, screens and places every
    /// [`TRACE_EVERY`] ticks, and every change of the offers.
    fn trace(&self, clock: u64, observation: &Observation, offers_before: [bool; 2]) {
        if std::env::var_os("SKATE2_TWO_PLAYER_TRACE").is_none() {
            return;
        }
        let changed = offers_before != observation.offers;
        if clock % TRACE_EVERY != 0 && !changed {
            return;
        }
        let roots = observation.coop.as_ref().map(|coop| [coop.player1_root, coop.root]);
        eprintln!(
            "two-player trace {clock}: {:?}, menus {}/{}, map {:?}, offers {:?}{}, speeds {:.1}/{:.1} m/s, roots {roots:?}",
            observation.pause,
            observation.menus[0].visible,
            observation.menus[1].visible,
            observation.map_owner,
            observation.offers,
            if changed { " (changed)" } else { "" },
            self.speed(0, clock),
            self.speed(1, clock),
        );
    }

    /// The scoreboard's lines and banked scores, and the tick player 2 first appeared.
    fn track_scores(&mut self, clock: u64, coop: Option<&SelectedCoopPlayer2>) {
        if let Some(coop) = coop {
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
                let apart = ground_distance(coop.player1_root, coop.root);
                eprintln!(
                    "two-player demo {clock}: player 2 spawned at {:?}, player 1 at {:?}",
                    coop.root, coop.player1_root
                );
                self.expect(
                    apart <= JOIN_NEAR,
                    "player 2's START while the game was still loading put them beside player 1 once it landed",
                    &format!("{apart:.1} m apart, {} ticks after START", clock.saturating_sub(JOIN)),
                );
            }
        }
    }

    /// Each player's skated ground distance this tick, and how far one was placed instead.
    fn track_roots(&mut self, clock: u64, coop: Option<&SelectedCoopPlayer2>) {
        let Ok(index) = usize::try_from(clock) else {
            return;
        };
        if self.steps.len() <= index {
            self.steps.resize(index + 1, [0.0; 2]);
        }
        self.placements = [None; 2];
        let Some(coop) = coop else {
            self.last_roots = None;
            return;
        };
        let roots = [coop.player1_root, coop.root];
        if let Some(last) = self.last_roots {
            for (player, (from, to)) in last.into_iter().zip(roots).enumerate() {
                let step = ground_distance(from, to);
                if step <= PLACED_STEP {
                    self.steps[index][player] = step;
                } else {
                    self.placements[player] = Some(step);
                }
            }
        }
        self.last_roots = Some(roots);
    }

    /// A player's skated ground distance over the clock ticks of `window`.
    fn skated(&self, player: usize, window: std::ops::Range<u64>) -> f32 {
        window
            .filter_map(|clock| usize::try_from(clock).ok())
            .filter_map(|index| self.steps.get(index))
            .map(|step| step[player])
            .sum()
    }

    /// A player's ground speed in m/s over the [`SPEED_TICKS`] ticks before `end`.
    fn speed(&self, player: usize, end: u64) -> f32 {
        self.skated(player, end.saturating_sub(SPEED_TICKS)..end) / SPEED_SECONDS
    }

    /// The player whose pause menu opened at `paused_at` braked while the other skated on.
    fn expect_braked(&mut self, braker: usize, paused_at: u64, now: u64) {
        let (before, after) = (self.speed(braker, paused_at), self.speed(braker, now));
        let other = self.skated(1 - braker, paused_at..now);
        self.expect(
            before > MIN_SPEED && after < before * BRAKED_TO && other >= SKATED_ON,
            &format!(
                "player {} braked while player {} skated on",
                braker + 1,
                2 - braker
            ),
            &format!("{before:.1} to {after:.1} m/s, the other skated {other:.1} m"),
        );
    }

    /// The checks that fall on a clock of the schedule.
    fn scheduled_checks(&mut self, clock: u64, observation: &Observation) {
        let [first, second] = observation.menus;
        let pause = observation.pause;
        let menus = format!(
            "{pause:?}, menus visible {}/{}",
            first.visible, second.visible
        );
        match clock {
            PLAYER2_PAUSE_CHECK => {
                self.paused_selection = Some(second.selected);
                self.expect(
                    pause == CoopPause::Single && second.visible && !first.visible,
                    "player 2's START opened their own pause menu and the game kept running",
                    &menus,
                );
                self.expect_braked(1, PLAYER2_PAUSE, clock);
            }
            PLAYER1_PROBE_CHECK => {
                let held = second.visible && Some(second.selected) == self.paused_selection;
                self.expect(
                    held,
                    "player 1's D-pad did not move player 2's pause menu",
                    &format!(
                        "visible {} selected {} (was {:?})",
                        second.visible, second.selected, self.paused_selection
                    ),
                );
            }
            PLAYER2_NAVIGATE_CHECK => {
                let moved = second.visible
                    && self
                        .paused_selection
                        .is_some_and(|selected| second.selected != selected);
                self.expect(
                    moved,
                    "player 2's D-pad moved their pause menu's selection",
                    &format!(
                        "visible {} selected {} (was {:?})",
                        second.visible, second.selected, self.paused_selection
                    ),
                );
            }
            PLAYER2_RESUME_CHECK => {
                self.expect(
                    pause == CoopPause::Running && !second.visible && !first.visible,
                    "player 2's START closed their pause menu; both skate again",
                    &menus,
                );
            }
            PLAYER1_PAUSE_CHECK => {
                self.expect(
                    pause == CoopPause::Single && first.visible && !second.visible,
                    "player 1's START opened their own pause menu and the game kept running",
                    &menus,
                );
                self.expect_braked(0, PLAYER1_PAUSE, clock);
            }
            BOTH_PAUSE_CHECK => {
                let moved = [
                    self.skated(0, clock - 40..clock),
                    self.skated(1, clock - 40..clock),
                ];
                // A stopped game moves nothing, so the freeze only shows if a skater was moving
                // when the second menu opened.
                let moving = self.speed(0, BOTH_PAUSE).max(self.speed(1, BOTH_PAUSE));
                self.expect(
                    pause == CoopPause::Full
                        && first.visible
                        && second.visible
                        && moving > MIN_SPEED
                        && moved.iter().all(|moved| *moved < FROZEN),
                    "both pause menus open stopped the game",
                    &format!(
                        "{menus}, a skater was moving {moving:.1} m/s when the second menu opened, the skaters moved {:.3} and {:.3} m over 40 ticks",
                        moved[0], moved[1]
                    ),
                );
            }
            BOTH_RESUME_CHECK => {
                self.expect(
                    pause == CoopPause::Single && first.visible && !second.visible,
                    "player 2's START closed their menu with player 1's open: the game runs again",
                    &menus,
                );
            }
            PLAYER1_RESUME_CHECK => {
                self.expect(
                    pause == CoopPause::Running && !first.visible && !second.visible,
                    "player 1's START closed their pause menu",
                    &menus,
                );
            }
            PLAYER1_BOOT_CHECK => {
                self.check_left(0, observation, "Boot Player 2 on player 1's pause menu");
            }
            REJOIN_CHECK => self.check_rejoined(0, observation),
            PLAYER2_BOOT_CHECK => {
                self.check_left(1, observation, "Boot Player 2 on their own pause menu");
            }
            REJOIN_AGAIN_CHECK => self.check_rejoined(1, observation),
            LEAVE_CHECK => self.check_left(2, observation, "holding BACK"),
            REJOIN_LEAVE_CHECK => self.check_rejoined(2, observation),
            _ => {}
        }
    }

    /// Player 2 is out of the game, by way of `how`.
    fn check_left(&mut self, way: usize, observation: &Observation, how: &str) {
        let [first, second] = observation.menus;
        let out = matches!(observation.state, TwoPlayerState::Waiting)
            && observation.coop.is_none()
            && !first.visible
            && !second.visible;
        self.left[way] = out;
        self.expect(
            out,
            &format!("{how} took player 2 out; one player again"),
            &format!(
                "{:?}, skater {}, menus visible {}/{}",
                observation.state,
                observation.coop.is_some(),
                first.visible,
                second.visible
            ),
        );
    }

    /// Player 2's START brought them back after the way out `way`.
    fn check_rejoined(&mut self, way: usize, observation: &Observation) {
        let back = self.left[way]
            && observation.coop.is_some()
            && matches!(observation.state, TwoPlayerState::BothActive);
        self.rejoined[way] = back;
        self.expect(
            back,
            "player 2 joined again with START",
            &format!(
                "{:?}, skater {}",
                observation.state,
                observation.coop.is_some()
            ),
        );
    }

    /// The Challenge Map steps: each follows what the session shows, since a teleport waits on
    /// the game's loads, and fails after [`MAP_DEADLINE`] ticks.
    fn map_checks(&mut self, clock: u64, observation: &Observation) {
        match self.map_step {
            MapStep::Waiting => {
                if clock + 1 >= MAP_START {
                    self.begin(
                        MapStep::Teleports(CoopPlayer::Player2),
                        clock,
                        observation.joins,
                    );
                }
            }
            MapStep::Teleports(mover) => self.observe_teleport(clock, observation, mover),
            MapStep::Joins(mover) => self.observe_join(clock, observation, mover),
            MapStep::Done => {}
        }
        if !matches!(self.map_step, MapStep::Waiting | MapStep::Done)
            && clock > self.map.since + MAP_DEADLINE
        {
            self.fail(format!(
                "{:?} did not finish within {MAP_DEADLINE} ticks: {:?}, the Challenge Map showed {:?}",
                self.map_step, self.map, self.sight
            ));
            self.finish(clock);
        }
    }

    fn begin(&mut self, step: MapStep, clock: u64, joins: u32) {
        eprintln!("two-player demo {clock}: map step {step:?}");
        self.map_step = step;
        self.map = MapProgress {
            since: clock + 1,
            joins,
            ..MapProgress::default()
        };
    }

    fn finish(&mut self, clock: u64) {
        self.map_step = MapStep::Done;
        self.done_at = Some(clock);
    }

    /// `mover` opens their Challenge Map and teleports; the other skates on and is offered the
    /// teleport.
    fn observe_teleport(&mut self, clock: u64, observation: &Observation, mover: CoopPlayer) {
        let follower = mover.other();
        let (name, other) = (player_name(mover), player_name(follower));
        let open = observation.map_owner == Some(mover);
        if open && self.map.opened.is_none() {
            self.map.opened = Some(clock);
            self.map.next_press = clock + MAP_PRESS;
            self.expect(
                observation.pause == CoopPause::Single,
                &format!("{name}'s Challenge Map opened on their half and the game kept running"),
                &format!("{:?}", observation.pause),
            );
        }
        if self.map.opened.is_some() && self.placements[follower.slot()].is_some() {
            self.map.other_placed = true;
        }
        if let (Some(opened), None) = (self.map.opened, self.map.closed) {
            // A pick's own travel fades the window and stops the game, so only the ticks before
            // it show whether the open map stopped anything.
            self.map.stopped |=
                observation.pause == CoopPause::Full && self.map.picked.is_none();
            if !open {
                self.map.closed = Some(clock);
                self.map.origin = self.last_roots;
                let skated = self.skated(follower.slot(), opened..clock);
                let stopped = self.map.stopped;
                self.expect(
                    skated >= SKATED_ON && !stopped,
                    &format!("{other} skated on while {name}'s Challenge Map was open"),
                    &format!("{skated:.1} m, the game stopped {stopped}"),
                );
            }
        }
        if self.map.closed.is_some() && self.map.landed.is_none() {
            if let (Some(origin), Some(roots)) = (self.map.origin, self.last_roots) {
                let moved = ground_distance(origin[mover.slot()], roots[mover.slot()]);
                if moved > TELEPORTED {
                    self.map.landed = Some(clock);
                    eprintln!("two-player demo {clock}: {name} landed {moved:.0} m from where they were");
                }
            }
        }
        let Some(landed) = self.map.landed else {
            return;
        };
        if observation.offers[follower.slot()] {
            self.expect(
                !self.map.other_placed,
                &format!("only {name} teleported"),
                &format!("{other} was placed {:?}", self.placements[follower.slot()]),
            );
            self.pass(format!(
                "{other} was offered {name}'s teleport {} ticks after it landed",
                clock - landed
            ));
            self.begin(MapStep::Joins(mover), clock, observation.joins);
        } else if clock > landed + OFFER_WAIT {
            let apart = self
                .last_roots
                .map(|roots| ground_distance(roots[0], roots[1]));
            self.fail(format!(
                "no offer reached {other} within {OFFER_WAIT} ticks of {name}'s teleport landing ({apart:?} m apart)"
            ));
            self.finish(clock);
        }
    }

    /// The other player joins `mover`'s teleport with LB + D-pad right and ends beside them.
    fn observe_join(&mut self, clock: u64, observation: &Observation, mover: CoopPlayer) {
        let follower = mover.other();
        if self.map.taken.is_none() && observation.joins > self.map.joins {
            self.map.taken = Some(clock);
            self.pass(format!(
                "LB + RIGHT joined {}'s teleport",
                player_name(mover)
            ));
        }
        let (Some(taken), Some(roots)) = (self.map.taken, self.last_roots) else {
            return;
        };
        let apart = ground_distance(roots[0], roots[1]);
        if apart <= JOIN_NEAR {
            self.pass(format!(
                "{} arrived {apart:.1} m from {}, {} ticks after the chord",
                player_name(follower),
                player_name(mover),
                clock - taken
            ));
            match mover {
                CoopPlayer::Player2 => self.begin(
                    MapStep::Teleports(CoopPlayer::Player1),
                    clock,
                    observation.joins,
                ),
                CoopPlayer::Player1 => self.finish(clock),
            }
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
            Some(clock) if clock > JOIN && clock < PLAYER2_PAUSE => {}
            Some(clock) => failures.push(format!("player 2 spawned only at gameplay tick {clock}")),
            None => failures.push("player 2's START never spawned a co-op skater".to_owned()),
        }
        // Each pad's pushes must move its own skater.
        let travel = [self.skated_total(0), self.skated_total(1)];
        for (index, travel) in travel.iter().enumerate() {
            if *travel < 1.0 {
                failures.push(format!("player {} moved only {travel:.2} m", index + 1));
            }
        }
        if self.split_frames == 0 {
            failures.push("no frame was drawn split-screen".to_owned());
        }
        if self.left != [true; 3] || self.rejoined != [true; 3] {
            failures.push(format!(
                "player 2 was not taken out and let in again by each way out (player 1's menu, their own menu, holding BACK): out {:?}, back {:?}",
                self.left, self.rejoined
            ));
        }
        if self.map_step != MapStep::Done {
            failures.push(format!("the Challenge Map steps stopped at {:?}", self.map_step));
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
                travel[0],
                travel[1],
                self.split_frames,
                self.best_banked[0],
                self.best_banked[1],
                self.best_team_total
            ))
        } else {
            Err(failures.join("; "))
        }
    }

    /// A player's skated ground distance over the whole run.
    fn skated_total(&self, player: usize) -> f32 {
        self.steps.iter().map(|step| step[player]).sum()
    }

    fn pass(&mut self, step: String) {
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

    /// Records the step as passed or failed, with what was measured.
    fn expect(&mut self, ok: bool, step: &str, measured: &str) {
        if ok {
            self.pass(format!("{step} ({measured})"));
        } else {
            self.fail(format!("{step}: {measured}"));
        }
    }
}

fn player_name(player: CoopPlayer) -> &'static str {
    match player {
        CoopPlayer::Player1 => "player 1",
        CoopPlayer::Player2 => "player 2",
    }
}

/// The row a pause menu has selected.
fn selected_option(menu: &PauseMenuSnapshot) -> Option<PauseMenuOption> {
    menu.options().get(menu.selected).copied()
}

/// The press that walks an open pause menu to Boot Player 2 and confirms it: D-pad up toward
/// the row (it is above Quit Game on player 1's menu, and wraps to the last row on player 2's),
/// A on it, D-pad down from No to Yes on the confirmation, then A.
fn boot_press(menu: &PauseMenuSnapshot) -> u16 {
    if !menu.visible {
        return 0;
    }
    match menu.popup {
        Some((_, 0)) => DPAD_DOWN,
        Some(_) => A,
        None if selected_option(menu) == Some(PauseMenuOption::BootPlayer2) => A,
        None => DPAD_UP,
    }
}

/// The press that walks an open Challenge Map toward Teleport along `route`: A on the info
/// component; at the root toward the route's first category, inside it toward the second, and
/// inside that to its first location (D-pad toward the row, A on it). The map has no wrap. A
/// category the lists do not show, or a group without locations, is an error.
fn map_navigation(sight: &MapSight, route: &[&str; 2]) -> Result<u16, String> {
    if sight.info {
        return Ok(A);
    }
    let wanted = if sight.title == ROOT_HAL {
        Some(route[0])
    } else if sight.title == route[0] {
        Some(route[1])
    } else {
        None
    };
    let target = match wanted {
        Some(hal) => sight
            .rows
            .iter()
            .position(|row| row == hal)
            .ok_or_else(|| format!("{:?} has no {hal} row: {:?}", sight.title, sight.rows))?,
        None if sight.rows.is_empty() => {
            return Err(format!("{:?} has no rows to teleport to", sight.title));
        }
        None => 0,
    };
    Ok(match sight.selected.cmp(&target) {
        Ordering::Less => DPAD_DOWN,
        Ordering::Greater => DPAD_UP,
        Ordering::Equal => A,
    })
}

fn button_name(buttons: u16) -> &'static str {
    match buttons {
        A => "A",
        DPAD_UP => "D-pad up",
        DPAD_DOWN => "D-pad down",
        _ => "a button",
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
    use skate2_core::ui::pause_menu::{HubPopup, StartHub};

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
    fn each_pad_presses_start_only_on_its_own_steps() {
        const PLAYER1_STARTS: [u64; 3] = [PLAYER1_PAUSE, PLAYER1_RESUME, PLAYER1_BOOT_OPEN];
        const PLAYER2_STARTS: [u64; 9] = [
            JOIN,
            PLAYER2_PAUSE,
            PLAYER2_RESUME,
            BOTH_PAUSE,
            BOTH_RESUME_PLAYER2,
            REJOIN,
            PLAYER2_BOOT_OPEN,
            REJOIN_AGAIN,
            REJOIN_LEAVE,
        ];
        let mut demo = TwoPlayerDemo::default();
        demo.advance(true, false);
        for clock in 1..=REJOIN_LEAVE + 1 {
            demo.advance(true, false);
            let first = demo.player1().buttons & START != 0;
            let second = demo.player2().buttons & START != 0;
            assert_eq!(first, PLAYER1_STARTS.contains(&clock), "player 1, clock {clock}");
            assert_eq!(second, PLAYER2_STARTS.contains(&clock), "player 2, clock {clock}");
        }
    }

    #[test]
    fn player2_holds_back_past_the_routers_leave_hold() {
        // `two_player_input_router::LEAVE_HOLD_FRAMES`
        const { assert!(LEAVE.end - LEAVE.start > 120) };
        let mut demo = TwoPlayerDemo::default();
        demo.advance(true, false);
        run_to(&mut demo, LEAVE.start + 1);
        assert_eq!(demo.player2().buttons, BACK);
        run_to(&mut demo, LEAVE.end);
        assert_eq!(demo.player2().buttons & BACK, 0);
    }

    #[test]
    fn pushes_stay_off_the_pads_own_menu_steps() {
        for push in PLAYER1_PUSHES {
            assert!(push.end <= PLAYER1_PAUSE, "{push:?}");
        }
        for push in PLAYER2_PUSHES {
            assert!(
                push.end <= PLAYER2_PAUSE
                    || (push.start >= PLAYER2_RESUME && push.end <= BOTH_PAUSE),
                "{push:?}"
            );
        }
        assert_eq!(PLAYER1_PROBE, PLAYER2_PAUSE + 100);
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
            assert!(at > JOIN && at + OLLIE_CROUCH + OLLIE_FLICK < PLAYER2_PAUSE);
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

    /// The session with both players in (or not, without `roots`), nobody in a menu.
    fn observation(roots: Option<[[f32; 3]; 2]>) -> Observation {
        Observation {
            state: if roots.is_some() {
                TwoPlayerState::BothActive
            } else {
                TwoPlayerState::Waiting
            },
            pause: CoopPause::Running,
            menus: [PauseMenuSnapshot::default(); 2],
            map_owner: None,
            map: None,
            offers: [false; 2],
            joins: 0,
            coop: roots.map(|[player1_root, root]| SelectedCoopPlayer2 {
                player1_root,
                player1_hips: player1_root,
                root,
                hips: root,
                scoreboard: skate2_core::two_player_session::CoopScoreboard::default(),
            }),
        }
    }

    #[test]
    fn placements_do_not_count_as_skating() {
        let mut demo = TwoPlayerDemo::default();
        demo.advance(true, false);
        for roots in [
            [[0.0; 3], [1.5, 0.0, 0.0]],
            [[0.5, 0.0, 0.0], [1.5, 0.0, 0.25]],
            [[500.0, 0.0, 0.0], [1.5, 0.0, 0.5]],
        ] {
            demo.advance(true, false);
            demo.observe(&observation(Some(roots)));
        }
        assert!((demo.skated_total(0) - 0.5).abs() < 1e-6);
        assert!((demo.skated_total(1) - 0.5).abs() < 1e-6);
        assert_eq!(demo.placements, [Some(499.5), None]);
    }

    #[test]
    fn braking_is_measured_from_the_ground_each_player_covered() {
        let mut demo = TwoPlayerDemo::default();
        let mut steps = vec![[0.0; 2]; 700];
        for step in &mut steps[590..600] {
            step[1] = 0.05;
        }
        for step in &mut steps[600..690] {
            step[0] = 0.1;
        }
        for step in &mut steps[680..690] {
            step[1] = 0.005;
        }
        demo.steps = steps;
        assert!((demo.speed(1, PLAYER2_PAUSE) - 3.0).abs() < 1e-3);
        assert!((demo.skated(0, PLAYER2_PAUSE..PLAYER2_PAUSE_CHECK) - 9.0).abs() < 1e-3);
        demo.expect_braked(1, PLAYER2_PAUSE, PLAYER2_PAUSE_CHECK);
        assert!(demo.failures.is_empty(), "{:?}", demo.failures);
        // A skater that kept its speed did not brake.
        for step in &mut demo.steps[680..690] {
            step[1] = 0.05;
        }
        demo.expect_braked(1, PLAYER2_PAUSE, PLAYER2_PAUSE_CHECK);
        assert_eq!(demo.failures.len(), 1);
    }

    fn coop_menu(selected: usize, popup: Option<(HubPopup, usize)>) -> PauseMenuSnapshot {
        PauseMenuSnapshot {
            visible: true,
            selected,
            popup,
            hub: StartHub::CoopPlayer2,
            coop_boot: true,
            ..PauseMenuSnapshot::default()
        }
    }

    #[test]
    fn a_pad_walks_its_menu_to_boot_player_2_and_confirms() {
        assert_eq!(boot_press(&PauseMenuSnapshot::default()), 0);
        // Map, then up wraps to Boot Player 2 on player 2's menu.
        assert_eq!(boot_press(&coop_menu(0, None)), DPAD_UP);
        assert_eq!(boot_press(&coop_menu(2, None)), A);
        let boot = Some(HubPopup::LeaveCoop);
        assert_eq!(boot_press(&coop_menu(2, boot.map(|popup| (popup, 0)))), DPAD_DOWN);
        assert_eq!(boot_press(&coop_menu(2, boot.map(|popup| (popup, 1)))), A);
    }

    /// One tick of the demo with the session as `scene` shows it.
    struct Scene {
        roots: [[f32; 3]; 2],
        pause: CoopPause,
        map_owner: Option<CoopPlayer>,
        sight: Option<MapSight>,
        menus: [PauseMenuSnapshot; 2],
        offers: [bool; 2],
        joins: u32,
    }

    /// The demo stages a tick, then sees the session `scene` shows.
    fn tick(demo: &mut TwoPlayerDemo, scene: &Scene) {
        demo.advance(true, false);
        look(demo, scene);
    }

    fn look(demo: &mut TwoPlayerDemo, scene: &Scene) {
        let mut seen = observation(Some(scene.roots));
        seen.pause = scene.pause;
        seen.map_owner = scene.map_owner;
        seen.map.clone_from(&scene.sight);
        seen.menus = scene.menus;
        seen.offers = scene.offers;
        seen.joins = scene.joins;
        demo.observe(&seen);
    }

    /// Ticks until `done` holds, moving `skater` along as a skating player does.
    fn tick_until(
        demo: &mut TwoPlayerDemo,
        scene: &mut Scene,
        skater: Option<usize>,
        done: impl Fn(&TwoPlayerDemo) -> bool,
    ) {
        for _ in 0..4000 {
            if done(demo) {
                return;
            }
            if let Some(skater) = skater {
                scene.roots[skater][2] += 0.1;
            }
            tick(demo, scene);
        }
        panic!("the demo never got there: {:?} {:?}", demo.map_step, demo.map);
    }

    fn new_scene() -> Scene {
        Scene {
            roots: [[0.0; 3], [2.0, 0.0, 0.0]],
            pause: CoopPause::Running,
            map_owner: None,
            sight: None,
            menus: [PauseMenuSnapshot::default(); 2],
            offers: [false; 2],
            joins: 0,
        }
    }

    /// A demo whose Challenge Map steps have just begun with player 2.
    fn demo_at_map_start(scene: &Scene) -> TwoPlayerDemo {
        let mut demo = TwoPlayerDemo::default();
        demo.advance(true, false);
        run_to(&mut demo, MAP_START - 1);
        demo.observe(&observation(Some(scene.roots)));
        demo
    }

    fn pad(demo: &TwoPlayerDemo, slot: usize) -> XboxPadSample {
        if slot == 0 { demo.player1() } else { demo.player2() }
    }

    /// A Challenge Map with the lists the demo walks (the root's categories, Locations' groups
    /// and a group's locations), answering the pad as the real one does: no wrap, a level
    /// starts on its first row, A on a location raises the info component and A on that
    /// teleports.
    struct FakeMap {
        sight: MapSight,
        teleported: bool,
    }

    const LOCATIONS: &str = "ID_STUFFMENU_CATEGORY_LOCATIONS";
    const GROUPS: [&str; 3] = [
        "ID_STUFFMENU_CATEGORY_LANDMARKS",
        "ID_STUFFMENU_CATEGORY_LANDMARK_POOLS",
        "ID_STUFFMENU_CATEGORY_LANDMARK_PLAZAS",
    ];

    fn level(title: &str, rows: &[&str]) -> MapSight {
        MapSight {
            title: title.to_owned(),
            rows: rows.iter().map(|row| (*row).to_owned()).collect(),
            selected: 0,
            info: false,
        }
    }

    impl FakeMap {
        fn new() -> Self {
            Self {
                sight: level(ROOT_HAL, &["ID_STUFFMENU_CATEGORY_RACES", LOCATIONS]),
                teleported: false,
            }
        }

        fn press(&mut self, buttons: u16) {
            let sight = &mut self.sight;
            match buttons {
                DPAD_DOWN => sight.selected = (sight.selected + 1).min(sight.rows.len() - 1),
                DPAD_UP => sight.selected = sight.selected.saturating_sub(1),
                A if sight.info => self.teleported = true,
                A => {
                    let row = sight.rows[sight.selected].clone();
                    *sight = match sight.title.as_str() {
                        ROOT_HAL => level(&row, &GROUPS),
                        LOCATIONS => level(&row, &["ID_LOCATION_FIRST", "ID_LOCATION_SECOND"]),
                        _ => MapSight {
                            info: true,
                            ..sight.clone()
                        },
                    };
                }
                _ => {}
            }
        }
    }

    /// `mover` presses START, then A on their hub's Map row on a menu step, and their map
    /// opens showing `sight`. Returns the clock it was seen open.
    fn open_map(demo: &mut TwoPlayerDemo, scene: &mut Scene, mover: CoopPlayer, sight: MapSight) -> u64 {
        let (mover_slot, follower_slot) = (mover.slot(), mover.other().slot());
        assert_eq!(demo.map_step, MapStep::Teleports(mover));
        // The mover presses START and the other keeps pushing.
        tick(demo, scene);
        assert_eq!(pad(demo, mover_slot).buttons, START);
        assert_eq!(pad(demo, follower_slot).buttons, A);
        // Their hub is up: A on its Map row, on a menu step.
        scene.pause = CoopPause::Single;
        scene.menus[mover_slot] = coop_menu(0, None);
        tick(demo, scene);
        tick_until(demo, scene, Some(follower_slot), |demo| {
            demo.clock.is_some_and(|clock| (clock + 1) % MENU_STEP == 0)
        });
        tick(demo, scene);
        assert_eq!(pad(demo, mover_slot).buttons, A);
        scene.map_owner = Some(mover);
        scene.sight = Some(sight);
        tick(demo, scene);
        demo.map.opened.expect("the map was seen open")
    }

    /// One teleport by `mover` and the other's join, as the session would show them, with the
    /// presses each pad makes asserted on the way: `route` is the mover's expected presses on
    /// the Challenge Map. `away` is where the mover lands.
    fn teleport_and_join(
        demo: &mut TwoPlayerDemo,
        scene: &mut Scene,
        mover: CoopPlayer,
        route: &[u16],
        away: [f32; 3],
    ) {
        let (mover_slot, follower_slot) = (mover.slot(), mover.other().slot());
        let mut map = FakeMap::new();
        let opened = open_map(demo, scene, mover, map.sight.clone());
        // Each press follows the last by the wait its kind needs, the map answering each.
        let mut presses: Vec<(u64, u16)> = Vec::new();
        for _ in 0..4000 {
            if map.teleported {
                break;
            }
            scene.roots[follower_slot][2] += 0.1;
            demo.advance(true, false);
            let buttons = pad(demo, mover_slot).buttons;
            if buttons != 0 {
                presses.push((demo.clock.unwrap(), buttons));
                map.press(buttons);
                scene.sight = Some(map.sight.clone());
            }
            look(demo, scene);
        }
        assert!(map.teleported, "{presses:?} in {:?}", map.sight);
        assert_eq!(
            presses.iter().map(|(_, buttons)| *buttons).collect::<Vec<_>>(),
            route
        );
        assert_eq!(presses[0].0, opened + MAP_PRESS);
        for pair in presses.windows(2) {
            let wait = if pair[0].1 == A { MAP_PRESS } else { MAP_MOVE };
            assert_eq!(pair[1].0 - pair[0].0, wait, "{presses:?}");
        }
        // The pick's own travel stops the game while the map is still up: not the open map's.
        scene.pause = CoopPause::Full;
        tick(demo, scene);
        assert!(!demo.map.stopped);
        // The last pick teleports: the map closes, then their skater lands far away.
        scene.sight = None;
        scene.map_owner = None;
        scene.menus[mover_slot] = PauseMenuSnapshot::default();
        scene.pause = CoopPause::Running;
        tick(demo, scene);
        assert!(demo.map.closed.is_some());
        scene.roots[mover_slot] = away;
        tick(demo, scene);
        assert!(demo.map.landed.is_some());
        // The other is offered it and joins with LB + D-pad right, held for a few ticks.
        scene.offers[follower_slot] = true;
        tick(demo, scene);
        assert_eq!(demo.map_step, MapStep::Joins(mover));
        tick(demo, scene);
        assert_eq!(pad(demo, follower_slot).buttons, LEFT_SHOULDER | DPAD_RIGHT);
        assert_eq!(pad(demo, mover_slot).buttons, 0);
        scene.offers[follower_slot] = false;
        scene.joins += 1;
        scene.roots[follower_slot] = [away[0] - 5.0, away[1], away[2]];
        tick(demo, scene);
    }

    /// What each pad presses on its map for the fake map's lists: down to Locations, into it
    /// and into the pad's group (Landmarks, or two rows further down, Plazas), then its first
    /// location and Teleport.
    const PLAYER2_ROUTE: [u16; 5] = [DPAD_DOWN, A, A, A, A];
    const PLAYER1_ROUTE: [u16; 7] = [DPAD_DOWN, A, DPAD_DOWN, DPAD_DOWN, A, A, A];

    #[test]
    fn player_2_teleports_alone_and_player_1_joins_with_the_chord() {
        let mut scene = new_scene();
        let mut demo = demo_at_map_start(&scene);
        assert_eq!(demo.map_step, MapStep::Teleports(CoopPlayer::Player2));
        teleport_and_join(
            &mut demo,
            &mut scene,
            CoopPlayer::Player2,
            &PLAYER2_ROUTE,
            [900.0, 0.0, 0.0],
        );
        assert_eq!(demo.map_step, MapStep::Teleports(CoopPlayer::Player1));
        assert!(demo.failures.is_empty(), "{:?}", demo.failures);
        // Player 1's map is next, the other skating on.
        tick(&mut demo, &scene);
        assert_eq!(demo.player1().buttons, START);
        assert_eq!(demo.player2().buttons, A);
    }

    #[test]
    fn player_1_teleports_next_and_the_demo_ends_after_player_2_joins() {
        let mut scene = new_scene();
        let mut demo = demo_at_map_start(&scene);
        demo.begin(MapStep::Teleports(CoopPlayer::Player1), MAP_START - 1, 0);
        teleport_and_join(
            &mut demo,
            &mut scene,
            CoopPlayer::Player1,
            &PLAYER1_ROUTE,
            [-900.0, 0.0, 0.0],
        );
        assert_eq!(demo.map_step, MapStep::Done);
        assert!(demo.failures.is_empty(), "{:?}", demo.failures);
        assert!(!demo.finished());
        let done = demo.done_at.unwrap();
        tick_until(&mut demo, &mut scene, None, TwoPlayerDemo::finished);
        assert_eq!(demo.clock, Some(done + FINISH_DELAY));
    }

    #[test]
    fn a_map_without_the_route_fails_the_step_with_the_rows_it_showed() {
        let mut scene = new_scene();
        let mut demo = demo_at_map_start(&scene);
        let opened = open_map(
            &mut demo,
            &mut scene,
            CoopPlayer::Player2,
            level(ROOT_HAL, &["ID_STUFFMENU_CATEGORY_RACES"]),
        );
        while demo.map_step != MapStep::Done {
            assert!(demo.clock.unwrap() < opened + MAP_PRESS + 2, "no failure");
            tick(&mut demo, &scene);
        }
        let failure = demo.failures.join("; ");
        assert!(failure.contains("player 2's Challenge Map"), "{failure}");
        assert!(failure.contains("no ID_STUFFMENU_CATEGORY_LOCATIONS row"), "{failure}");
        assert!(failure.contains("ID_STUFFMENU_CATEGORY_RACES"), "{failure}");
        assert!(demo.done_at.is_some());
    }

    #[test]
    fn a_game_stopped_by_the_open_map_before_its_pick_fails_the_step() {
        let mut scene = new_scene();
        let mut demo = demo_at_map_start(&scene);
        open_map(&mut demo, &mut scene, CoopPlayer::Player2, FakeMap::new().sight);
        // Player 1 skates on, but the game stopped with no pick made.
        for _ in 0..30 {
            scene.roots[0][2] += 0.1;
            tick(&mut demo, &scene);
        }
        scene.pause = CoopPause::Full;
        tick(&mut demo, &scene);
        scene.map_owner = None;
        scene.sight = None;
        tick(&mut demo, &scene);
        let failure = demo.failures.join("; ");
        assert!(failure.contains("skated on while player 2's Challenge Map was open"), "{failure}");
        assert!(failure.contains("the game stopped true"), "{failure}");
    }

    #[test]
    fn the_map_navigation_walks_the_lists_toward_teleport() {
        let route = &MAP_ROUTES[0];
        let at = |title: &str, rows: &[&str], selected: usize, info: bool| MapSight {
            selected,
            info,
            ..level(title, rows)
        };
        let root = ["ID_STUFFMENU_CATEGORY_RACES", route[0], "ID_STUFFMENU_CATEGORY_SPOTS"];
        // The root: down to Locations, A on it, up from below it.
        assert_eq!(map_navigation(&at(ROOT_HAL, &root, 0, false), route), Ok(DPAD_DOWN));
        assert_eq!(map_navigation(&at(ROOT_HAL, &root, 1, false), route), Ok(A));
        assert_eq!(map_navigation(&at(ROOT_HAL, &root, 2, false), route), Ok(DPAD_UP));
        // Locations: to the group.
        let groups = ["ID_STUFFMENU_CATEGORY_LANDMARKS", route[1]];
        assert_eq!(map_navigation(&at(route[0], &groups, 0, false), route), Ok(DPAD_DOWN));
        assert_eq!(map_navigation(&at(route[0], &groups, 1, false), route), Ok(A));
        // The group: its first location, then Teleport.
        let places = ["ID_LOCATION_FIRST", "ID_LOCATION_SECOND"];
        assert_eq!(map_navigation(&at(route[1], &places, 0, false), route), Ok(A));
        assert_eq!(map_navigation(&at(route[1], &places, 1, false), route), Ok(DPAD_UP));
        assert_eq!(map_navigation(&at(route[1], &places, 0, true), route), Ok(A));
        // A level the route cannot go through is an error naming the rows.
        let error = map_navigation(&at(ROOT_HAL, &root[..1], 0, false), route).unwrap_err();
        assert!(error.contains(route[0]) && error.contains("RACES"), "{error}");
        let error = map_navigation(&at(route[0], &groups[..1], 0, false), route).unwrap_err();
        assert!(error.contains(route[1]), "{error}");
        assert!(map_navigation(&at(route[1], &[], 0, false), route).is_err());
    }

    #[test]
    fn each_pad_goes_to_a_different_group_of_locations() {
        assert_eq!(MAP_ROUTES[0][0], MAP_ROUTES[1][0]);
        assert_ne!(MAP_ROUTES[0][1], MAP_ROUTES[1][1]);
        assert!(GROUPS.contains(&MAP_ROUTES[0][1]) && GROUPS.contains(&MAP_ROUTES[1][1]));
        assert_eq!(MAP_ROUTES[0][0], LOCATIONS);
    }

    #[test]
    fn a_map_step_that_never_happens_fails_with_its_progress() {
        let scene = new_scene();
        let mut demo = demo_at_map_start(&scene);
        for _ in 0..=MAP_DEADLINE + 2 {
            tick(&mut demo, &scene);
        }
        assert_eq!(demo.map_step, MapStep::Done);
        let failure = demo.failures.join("; ");
        assert!(failure.contains("did not finish within"), "{failure}");
        assert!(demo.done_at.is_some());
    }

    #[test]
    fn verify_reports_every_missing_step() {
        let demo = TwoPlayerDemo::default();
        let error = demo.verify().unwrap_err();
        assert!(error.contains("never spawned"));
        assert!(error.contains("split-screen"));
        assert!(error.contains("taken out and let in again"));
        assert!(error.contains("Challenge Map steps stopped"));
        assert!(error.contains("own scoreboards"));
    }

    #[test]
    fn a_line_banked_before_the_second_players_ollie_is_not_theirs() {
        let mut demo = TwoPlayerDemo::default();
        let banked = |player1: u64, player2: u64| {
            let mut seen = observation(Some([[0.0; 3]; 2]));
            if let Some(coop) = seen.coop.as_mut() {
                coop.scoreboard.players[0].banked = player1;
                coop.scoreboard.players[1].banked = player2;
            }
            seen
        };
        run_to(&mut demo, PLAYER1_OLLIE + 30);
        demo.observe(&banked(2, 2));
        let error = demo.verify().unwrap_err();
        assert!(error.contains("own scoreboards"), "{error}");
        demo.first_banked[1] = Some(PLAYER2_OLLIE + 30);
        assert!(!demo.verify().unwrap_err().contains("own scoreboards"));
    }
}
