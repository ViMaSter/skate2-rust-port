//! PC port: split-screen co-op's shared front end and travel. The reference has one player, one
//! pause menu and one stream focus; here each player has their own pause menu, one open menu
//! only stops that player's skater, the Challenge Map teleports only the player who picked,
//! and the other may join their teleport. Nothing here is recovered reference behaviour
//! (confidence: hypothesis throughout); it composes recovered pieces (`PlayerUI`'s unfocused
//! brake, `Actor::SetTransform`, the stream focus) for a second local player.
use super::career;
use super::freeskate::actor_set_transform;
use super::world_travel::TravelWorld;
use super::{SelectedLiveError, SelectedLiveSession, SelectedLiveStage as Stage};
use crate::challenge::stuff_menu::{MapAction, challenge_world_location, location_world_file};
use crate::selected_session_boot_owner::party_skaters::PartySkater;
use crate::two_player_session::{
    CoopPause, CoopPlayer, PLAYER2_INDEX, PLAYER2_INTRO_FALLBACK_FRAMES, Player2TravelStep, TELEPORT_OFFER_NEAR,
    regroup_transform,
};
use crate::ui::pause_menu::{PauseMenuController, PauseMenuInput, PauseMenuOption, PauseMenuSnapshot};
use std::sync::Arc;

fn error(e: impl std::fmt::Display) -> SelectedLiveError {
    SelectedLiveError::failed(Stage::TerrainResident, e.to_string())
}

/// The distance on the ground plane between two positions.
fn ground_distance(a: [f32; 3], b: [f32; 3]) -> f32 {
    (a[0] - b[0]).hypot(a[2] - b[2])
}

fn position_of(transform: [f32; 16]) -> [f32; 3] {
    [transform[12], transform[13], transform[14]]
}

impl<T, C> SelectedLiveSession<T, C> {
    fn player2_skater(&self) -> Option<&PartySkater> {
        self.inner.party.iter().find(|skater| skater.player_index() == PLAYER2_INDEX)
    }

    /// The co-op skater's root as its presentation last published it.
    pub(super) fn player2_root(&self) -> Option<[f32; 16]> {
        self.player2_skater()?.presentation_runtime.published_root()
    }

    /// Where the co-op skater is for the stream focus and the offers: its teleport's destination
    /// once the skater was set there, else its published root.
    pub(super) fn player2_position(&self) -> Option<[f32; 3]> {
        if let Some(travel) = self.two_player.player2_travel.filter(|travel| travel.placed) {
            return Some(travel.destination());
        }
        self.player2_root().map(position_of)
    }

    /// The first player's position (their root, as their presentation last published it).
    fn player1_position(&self) -> Option<[f32; 3]> {
        self.player1_root().map(position_of)
    }

    /// The stream foci the co-op skater adds to the first player's: where it stands (so its
    /// collisions stay while its teleport's destination streams in) and where it is going. Once
    /// it was set at the destination, its published root lags behind and only the destination
    /// counts.
    pub(super) fn coop_secondary_foci(&self) -> Vec<[f32; 3]> {
        if !self.two_player.is_player2_spawned() {
            return Vec::new();
        }
        let travel = self.two_player.player2_travel;
        let mut foci = Vec::with_capacity(2);
        if travel.is_none_or(|travel| !travel.placed)
            && let Some(root) = self.player2_root()
        {
            foci.push(position_of(root));
        }
        if let Some(travel) = travel {
            foci.push(travel.destination());
        }
        foci
    }

    /// Whether the players stand together: close enough that an offer to join a teleport is moot.
    pub(super) fn players_together(&self) -> bool {
        match (self.player1_position(), self.player2_position()) {
            (Some(player1), Some(player2)) => ground_distance(player1, player2) <= TELEPORT_OFFER_NEAR,
            _ => false,
        }
    }

    /// One completed tick of the co-op skater's teleport: once its destination is resident as
    /// the second stream focus, the skater is set there (`Actor::SetTransform` 0x825ed698, as the
    /// AI's teleport does; medium confidence for a human skater) and the first player is offered
    /// the teleport.
    pub(super) fn update_player2_travel(&mut self) -> Result<(), String> {
        let Some(destination) = self.two_player.player2_travel_destination() else { return Ok(()) };
        let Some(skater) = self.player2_skater() else {
            self.two_player.player2_travel = None;
            return Ok(());
        };
        let actor = Arc::clone(&skater.actor);
        let root = skater.presentation_runtime.published_root_position();
        let resident = match self.inner.terrain.as_ref() {
            Some(terrain) => terrain.secondary_focus_resident(destination).map_err(|e| format!("{e:?}"))?,
            None => false,
        };
        match self.two_player.tick_player2_travel(resident, root) {
            Player2TravelStep::Place(transform) => {
                if let Err(e) = actor_set_transform(&actor, transform) {
                    self.two_player.player2_travel = None;
                    return Err(e);
                }
                self.offer_teleport(CoopPlayer::Player2, destination);
            }
            Player2TravelStep::TimedOut => {
                return Err("the destination did not stream in; the teleport was dropped".to_owned());
            }
            Player2TravelStep::Idle
            | Player2TravelStep::Waiting
            | Player2TravelStep::Holding
            | Player2TravelStep::Settled => {}
        }
        Ok(())
    }

    /// The co-op player's teleport to `transform` starts (the skater brakes until its
    /// destination is streamed in and it is set there). False when the stream holds nothing at
    /// the destination (`landable`): there is nowhere to stand.
    pub(super) fn start_player2_teleport(&mut self, transform: [f32; 16]) -> Result<bool, SelectedLiveError> {
        let terrain = self.inner.terrain.as_ref().ok_or_else(|| error("terrain absent"))?;
        if !terrain.landable(position_of(transform)).map_err(error)? {
            return Ok(false);
        }
        self.two_player.begin_player2_travel(transform);
        Ok(true)
    }

    /// `teleporter` arrived at `destination`: the other player is offered to join, unless they
    /// stand there already. The teleporter's own offers end: they chose a destination of their own.
    pub(super) fn offer_teleport(&mut self, teleporter: CoopPlayer, destination: [f32; 3]) {
        if !self.two_player.is_player2_spawned() {
            return;
        }
        self.two_player.offers = [None; 2];
        let other = match teleporter {
            CoopPlayer::Player1 => self.player2_position(),
            CoopPlayer::Player2 => self.player1_position(),
        };
        // A co-op player still waiting for a destination of their own is not offered another.
        let moot = other.is_some_and(|other| ground_distance(other, destination) <= TELEPORT_OFFER_NEAR)
            || (teleporter == CoopPlayer::Player1 && self.two_player.player2_travelling());
        if !moot {
            self.two_player.offer_join(teleporter);
        }
    }

    /// `receiver` joins the teleport offered to them, if any. The co-op player is set beside the
    /// first player at once (the first player's area is resident); the first player travels to
    /// where the co-op player is, behind the fade as a Challenge Map teleport does.
    fn join_teleport(&mut self, receiver: CoopPlayer) -> Result<(), SelectedLiveError> {
        if self.two_player.offer_for(receiver).is_none() {
            return Ok(());
        }
        let spawn = self.inner.spawn.ok_or_else(|| error("spawn position absent"))?;
        match receiver {
            CoopPlayer::Player2 => {
                let player1 = self.player1_root().ok_or_else(|| error("the first player's root is not published"))?;
                let actor = Arc::clone(&self.player2_skater().ok_or_else(|| error("the co-op skater is absent"))?.actor);
                actor_set_transform(&actor, regroup_transform(spawn, player1)).map_err(error)?;
                self.two_player.player2_travel = None;
            }
            CoopPlayer::Player1 => {
                let target = match self.two_player.player2_travel.filter(|travel| travel.placed) {
                    Some(travel) => travel.transform,
                    None => self.player2_root().ok_or_else(|| error("the co-op skater's root is not published"))?,
                };
                self.two_player.offer_on_arrival = false;
                self.start_world_travel(TravelWorld::Keep, regroup_transform(spawn, target), false);
            }
        }
        self.two_player.take_offer(receiver);
        self.two_player.note_join();
        Ok(())
    }

    /// The players' LB + D-pad right: a player's chord joins the teleport offered to them while
    /// their pause menu is closed, and no travel is under way for them. Returns the chords that
    /// were pressed with an offer standing, which the first player's D-pad HUD must not see.
    pub(super) fn update_join_chords(&mut self, chords: [bool; 2]) -> [bool; 2] {
        let mut consumed = [false; 2];
        if !self.two_player.is_player2_spawned() {
            return consumed;
        }
        for receiver in CoopPlayer::ALL {
            if !chords[receiver.slot()] || self.two_player.offer_for(receiver).is_none() {
                continue;
            }
            consumed[receiver.slot()] = true;
            if !self.can_join_teleport(receiver) {
                continue;
            }
            if let Err(e) = self.join_teleport(receiver) {
                self.two_player.report(format!("joining the teleport failed: {e:?}"));
            }
        }
        consumed
    }

    /// Whether `receiver`'s chord joins the teleport offered to them now: their pause menu is
    /// closed and no travel of theirs is under way.
    fn can_join_teleport(&self, receiver: CoopPlayer) -> bool {
        let travelling = match receiver {
            CoopPlayer::Player1 => self.map_travel.is_some(),
            CoopPlayer::Player2 => self.two_player.player2_travelling(),
        };
        self.two_player.offer_for(receiver).is_some() && !self.player_menu(receiver).snapshot().visible && !travelling
    }

    /// Which player the host prompts to join the other's teleport (LB + D-pad right): those with
    /// an offer standing that their chord would take now.
    #[must_use]
    pub fn coop_join_prompts(&self) -> [bool; 2] {
        let spawned = self.two_player.is_player2_spawned();
        CoopPlayer::ALL.map(|receiver| spawned && self.can_join_teleport(receiver))
    }

    fn player_menu(&self, player: CoopPlayer) -> &PauseMenuController {
        match player {
            CoopPlayer::Player1 => &self.pause_menu,
            CoopPlayer::Player2 => &self.pause_menu_p2,
        }
    }

    /// The pause menu of the player whose pad drives the screens both menus share.
    pub(super) fn screen_owner_menu(&mut self) -> &mut PauseMenuController {
        match self.two_player.screen_owner {
            CoopPlayer::Player1 => &mut self.pause_menu,
            CoopPlayer::Player2 => &mut self.pause_menu_p2,
        }
    }

    /// The screens that fill the window and stop both players: the co-op pause is `Full` while
    /// one is up. (The Challenge Map is not one: it fills its owner's half.)
    pub(super) fn full_window_screen_open(&self) -> bool {
        self.options_menu.visible()
            || self.audio.menu.visible()
            || self.skate_profile.visible()
            || self.trick_guide.is_some()
            || self.trick_guide_pending
            || self.replay_editor.is_some()
            || self.replay_editor_pending
            || self.replay_file_pending.is_some()
            || self.editor.is_some()
            || self.editor_enter_pending
            || self.main_menu.is_some()
            || self.main_menu_pending
            || self.movie_state_open()
            || self.online_hub_open()
            || self.online.start.is_some()
            || (self.map_travel.is_some() && !self.map_travel_dispatched)
            || self.pause_menu.snapshot().jacked
            || self.career.as_ref().is_some_and(career::CareerSession::overlay_open)
    }

    /// The co-op pause state for this moment ([`CoopPause::of`]).
    pub(super) fn refresh_coop_pause(&mut self) {
        self.two_player.pause = CoopPause::of(
            self.two_player.is_player2_spawned(),
            self.pause_menu.snapshot().visible,
            self.pause_menu_p2.snapshot().visible,
            self.full_window_screen_open(),
        );
    }

    /// Both pause menus' co-op state: they offer Boot Player 2 while the co-op player is in, and
    /// the co-op player's menu closes with them.
    pub(super) fn sync_coop_menus(&mut self) {
        let spawned = self.two_player.is_player2_spawned();
        self.pause_menu.set_coop_boot(spawned);
        self.pause_menu_p2.set_coop_boot(spawned);
        if !spawned && self.pause_menu_p2.snapshot().visible {
            self.pause_menu_p2.close();
        }
    }

    /// `kPAUSE_STATE_FRONTEND` follows the co-op pause: pushed only while the game is `Full`
    /// paused. One open pause menu leaves the stack alone and the game simulating. Without the
    /// co-op player the first player's menu pushes and pulls it as the reference does.
    pub(super) fn reconcile_coop_frontend_pause(&mut self) -> Result<(), SelectedLiveError> {
        if self.two_player.pause == CoopPause::Off {
            return Ok(());
        }
        let want = self.two_player.pause == CoopPause::Full;
        if want != self.pause_menu_pause_pushed {
            self.inner
                .bound_input
                .as_mut()
                .ok_or_else(|| error("input absent"))?
                .try_set_frontend_pause(want)
                .map_err(|e| SelectedLiveError::failed(Stage::PreupdateInput, e))?;
            self.pause_menu_pause_pushed = want;
        }
        Ok(())
    }

    /// The co-op player left: a Challenge Map they opened closes with them, their menu with it,
    /// and the pause stack follows the first player's menu alone again.
    pub(super) fn finish_player2_removal(&mut self, screen_owner: CoopPlayer) -> Result<(), SelectedLiveError> {
        if screen_owner == CoopPlayer::Player2 && self.map_menu.snapshot().visible {
            if let Some(career) = self.career.as_mut() {
                career.close_challenge_map();
            }
            self.map_menu.close();
        }
        self.pause_menu_p2.close();
        self.sync_frontend_pause_to_first_menu()
    }

    /// The pause stack follows the first player's menu alone again (the co-op player left): its
    /// FRONTEND pause, if any, is what the menu keeps.
    pub(super) fn sync_frontend_pause_to_first_menu(&mut self) -> Result<(), SelectedLiveError> {
        let want = self.pause_menu.snapshot().visible;
        if want != self.pause_menu_pause_pushed {
            self.inner
                .bound_input
                .as_mut()
                .ok_or_else(|| error("input absent"))?
                .try_set_frontend_pause(want)
                .map_err(|e| SelectedLiveError::failed(Stage::PreupdateInput, e))?;
            self.pause_menu_pause_pushed = want;
        }
        Ok(())
    }

    /// The co-op player's own pause menu, run beside the first player's: their START opens it, its
    /// Challenge Map and Game Settings open the screens the first player's hub opens (driven by
    /// this player's pad), and Boot Player 2 (Leave Co-op on this menu) removes them.
    pub(super) fn update_player2_hub(&mut self, input: PauseMenuInput, has_focus: bool) -> Result<(), SelectedLiveError> {
        // A screen that fills the window stops both players, a menu of theirs open under it
        // included.
        if !self.two_player.is_player2_spawned() || self.full_window_screen_open() {
            return Ok(());
        }
        let was = self.pause_menu_p2.snapshot().visible;
        let map_open = self.map_menu.snapshot().visible;
        if self.pause_menu_p2.awaiting_intro() {
            self.two_player.player2_intro_frames = self.two_player.player2_intro_frames.saturating_add(1);
            if self.two_player.player2_intro_frames >= PLAYER2_INTRO_FALLBACK_FRAMES {
                self.pause_menu_p2.acknowledge_intro();
            }
        } else {
            self.two_player.player2_intro_frames = 0;
        }
        // StateManagerFrontEnd::ReceiveInput (0x82656af8): nothing reaches a hub before its
        // screen's IntroComplete, and the Challenge Map this player opened takes their input.
        let owns_map = map_open && self.two_player.screen_owner == CoopPlayer::Player2;
        let input = if self.pause_menu_p2.awaiting_intro() || owns_map { PauseMenuInput::default() } else { input };
        let registered = self.inner.player.as_ref().is_some_and(|player| {
            matches!(player.selected.owners.loader.phase(), crate::player::PlayerLoadPhase::Loaded { .. })
        });
        let can_open = self
            .inner
            .bound_input
            .as_ref()
            .ok_or_else(|| error("input absent"))?
            .try_can_open_frontend()
            .map_err(|e| SelectedLiveError::failed(Stage::PreupdateInput, e))?
            && !self.career.as_ref().is_some_and(|career| career.system.world().pause_menu_disabled)
            && !self.online_loading_screen();
        let map_challenge_loading = self.career.as_ref().is_some_and(career::CareerSession::map_challenge_loading);
        let popup_open = self.career.as_ref().is_some_and(career::CareerSession::popup_open);
        let focused = has_focus
            && self.completed_frames != 0
            && self.map_travel.is_none()
            && !map_challenge_loading
            && (was || (can_open && registered && !popup_open));
        let snapshot = self.pause_menu_p2.update(input, focused);
        if was && snapshot.visible && !snapshot.jacked && has_focus && input.select {
            match self.pause_menu_p2.selected_option() {
                PauseMenuOption::Map if !map_open => {
                    self.map_menu.open();
                    if let Some(career) = self.career.as_mut() {
                        career.open_challenge_map(self.additional_location);
                    }
                    self.two_player.screen_owner = CoopPlayer::Player2;
                }
                PauseMenuOption::Settings if !map_open => {
                    // GameBackEnd::IsFreeSkateMode: a game without the career's challenge system.
                    let video = if self.career.is_none() {
                        crate::ui::options::VideoScreen::FreeSkate
                    } else {
                        crate::ui::options::VideoScreen::Game
                    };
                    self.options_menu.open(true, video, &self.audio.settings);
                    self.two_player.screen_owner = CoopPlayer::Player2;
                }
                _ => {}
            }
        }
        if self.pause_menu_p2.take_boot_player2_confirmed() {
            self.pause_menu_p2.close();
            self.two_player.begin_remove_player2();
        }
        Ok(())
    }

    /// The co-op player's Challenge Map pick: a plain teleport of their skater (never a
    /// challenge's start, and no travel between worlds, which the co-op world cannot follow). The
    /// map and their menu close either way.
    pub(super) fn player2_map_pick(&mut self, action: MapAction) -> Result<(), SelectedLiveError> {
        let (key, challenge) = match action {
            MapAction::TeleportChallenge(key) | MapAction::RetryChallenge(key) => (key, true),
            MapAction::TeleportLocation(key) => (key, false),
            MapAction::None | MapAction::Close => return Ok(()),
        };
        let career = self.career.as_ref().ok_or_else(|| error("career map owner absent"))?;
        let world_location = if challenge { challenge_world_location(career.system.world(), key) } else { Some(key) };
        let world_file = world_location.and_then(|location| location_world_file(career.system.world(), location));
        let transform = if challenge {
            career.challenge_teleport_transform(key)
        } else {
            self.inner.assets.as_ref().and_then(|assets| assets.map_catalog.transform(key))
        };
        let changes_world = self.world_operation(world_location.unwrap_or(u64::MAX), world_file.as_deref()).is_some();
        self.close_player2_map();
        self.player2_teleport_to(transform, changes_world)
    }

    /// The co-op player's pick from the map without a career.
    pub(super) fn player2_location_pick(&mut self, key: u64) -> Result<(), SelectedLiveError> {
        let transform = self.inner.assets.as_ref().and_then(|assets| assets.map_catalog.transform(key));
        let changes_world = self.world_operation(u64::MAX, None).is_some();
        self.close_player2_map();
        self.player2_teleport_to(transform, changes_world)
    }

    fn close_player2_map(&mut self) {
        if let Some(career) = self.career.as_mut() {
            career.close_challenge_map();
        }
        self.map_menu.close();
        self.pause_menu_p2.close();
    }

    fn player2_teleport_to(&mut self, transform: Option<[f32; 16]>, changes_world: bool) -> Result<(), SelectedLiveError> {
        if changes_world {
            self.two_player.report("player 2 cannot teleport into another world; the teleport was dropped".to_owned());
        } else if let Some(transform) = transform {
            if !self.start_player2_teleport(transform)? {
                self.two_player.report("player 2's destination is not in the streamed world; the teleport was dropped".to_owned());
            }
        }
        Ok(())
    }

    /// Whether a travel of the first player's into `world` is refused: the co-op player's skater
    /// stays in the world it was spawned in, so a travel that loads or unloads an additional
    /// world is not offered while they are in the game.
    pub(super) fn coop_refuses_world_change(&mut self, world: &TravelWorld) -> bool {
        if *world == TravelWorld::Keep || !self.two_player.is_player2_spawned() {
            return false;
        }
        self.two_player.report("a teleport into another world is not available in co-op; the teleport was dropped".to_owned());
        true
    }

    /// Whether the first player's Challenge Map pick would travel between worlds, which the
    /// co-op world cannot follow ([`Self::coop_refuses_world_change`]); the map stays open.
    pub(super) fn coop_refuses_map_pick(&mut self, action: MapAction) -> bool {
        if !self.two_player.is_player2_spawned() {
            return false;
        }
        let Some(career) = self.career.as_ref() else { return false };
        let location = match action {
            MapAction::TeleportChallenge(key) | MapAction::RetryChallenge(key) => {
                challenge_world_location(career.system.world(), key)
            }
            MapAction::TeleportLocation(key) => Some(key),
            MapAction::None | MapAction::Close => return false,
        };
        let file = location.and_then(|location| location_world_file(career.system.world(), location));
        let world = self.world_operation(location.unwrap_or(u64::MAX), file.as_deref()).unwrap_or_default();
        self.coop_refuses_world_change(&world)
    }

    /// The first player's Challenge Map pick: the travel behind the fade, offered to the co-op
    /// player once the first player arrives.
    pub(super) fn start_map_travel(&mut self, world: TravelWorld, transform: [f32; 16]) {
        self.start_world_travel(world, transform, false);
        self.two_player.offer_on_arrival = self.two_player.is_player2_spawned();
    }

    /// The first player's travel arrived at `destination`: the co-op player may join it.
    pub(super) fn offer_arrival(&mut self, destination: Option<[f32; 3]>) {
        if std::mem::take(&mut self.two_player.offer_on_arrival)
            && let Some(destination) = destination
        {
            self.offer_teleport(CoopPlayer::Player1, destination);
        }
    }

    /// The co-op player's pause menu, for the host's presentation.
    #[must_use]
    pub fn pause_menu_player2_snapshot(&self) -> PauseMenuSnapshot {
        self.pause_menu_p2.snapshot()
    }

    /// The co-op player's pause menu screen ran its `IntroComplete`, as
    /// [`Self::acknowledge_screens`] reports the first player's.
    pub fn acknowledge_player2_hub_intro(&mut self) {
        self.pause_menu_p2.acknowledge_intro();
    }

    /// Whether the co-op pause is [`CoopPause::Full`]: the front end holds both players, through
    /// whichever player's menus or screens, so the host runs it over the retained world even
    /// when the first player's menu is closed.
    #[must_use]
    pub fn coop_frontend_paused(&self) -> bool {
        self.two_player.pause == CoopPause::Full
    }
}
