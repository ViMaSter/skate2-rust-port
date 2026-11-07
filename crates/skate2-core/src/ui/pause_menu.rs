//! StartHub (FE state 7): the hub states `OnIntroDone` picks, their options, `IsItemEnabled` and
//! confirmations, the open/close policy and the authored picker navigation. The runtime
//! dispatches destination actions from the selected row.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum PauseMenuOption {
    Replay = 0,
    Map = 1,
    Blog = 2,
    Profile = 3,
    EditSkater = 4,
    TrickGuide = 5,
    Settings = 6,
    Quit = 7,
    QuitCreateASpot = 9,
    CreateASpotSpotInfo = 10,
    RestartChallenge = 11,
    QuitChallenge = 12,
    SaveReplay = 13,
    /// Leave Challenge (PC port addition).
    LeaveChallenge = 14,
    /// Boot Player 2 out of the split-screen co-op game (PC port addition).
    BootPlayer2 = 15,
}

#[cfg(test)]
mod quit_tests {
    use super::*;
    #[test]
    fn quit_confirmation_defaults_to_no_and_requires_yes() {
        let mut menu=PauseMenuController::default();
        menu.update(PauseMenuInput{start:true,..Default::default()},true);
        // Nothing reaches the hub before its screen's intro completes.
        menu.update(PauseMenuInput{up:true,..Default::default()},true);
        assert_eq!(menu.selected_option(),PauseMenuOption::Map);
        assert!(menu.awaiting_intro());
        menu.acknowledge_intro();
        // The hub opens on Map, the first enabled row; up wraps past the disabled Replay row to
        // Quit using the same picker input as the pad.
        menu.update(PauseMenuInput{up:true,..Default::default()},true);
        assert_eq!(menu.selected_option(),PauseMenuOption::Quit);
        let state=menu.update(PauseMenuInput{select:true,..Default::default()},true);
        assert_eq!(state.popup,Some((HubPopup::QuitNoDataLoss,0)));
        assert!(!menu.take_quit_confirmed());
        let state=menu.update(PauseMenuInput{cancel:true,..Default::default()},true);
        assert_eq!(state.popup,Some((HubPopup::QuitNoDataLoss,0)));
        let state=menu.update(PauseMenuInput{select:true,..Default::default()},true);
        assert!(state.visible);
        assert_eq!(state.popup,None);
        assert!(!menu.take_quit_confirmed());
        menu.update(PauseMenuInput{select:true,..Default::default()},true);
        menu.update(PauseMenuInput{down:true,..Default::default()},true);
        menu.update(PauseMenuInput{select:true,..Default::default()},false);
        assert!(!menu.take_quit_confirmed());
        menu.update(PauseMenuInput{select:true,..Default::default()},true);
        assert!(menu.take_quit_confirmed());
        assert!(!menu.take_quit_confirmed());
    }
    #[test]
    fn sounds_follow_starthub_popup_and_picker() {
        let mut menu=PauseMenuController::default();
        menu.update(PauseMenuInput{start:true,..Default::default()},true);
        menu.acknowledge_intro();
        assert_eq!(menu.take_sounds(),["FE_SOUND_PAUSE_MENU_PRESS"]);
        // Map -> Profile, then back up to Map.
        menu.update(PauseMenuInput{down:true,..Default::default()},true);
        menu.update(PauseMenuInput{up:true,..Default::default()},true);
        assert_eq!(menu.take_sounds(),["FE_SOUND_CORE_NAV_DOWN","FE_SOUND_CORE_NAV_UP"]);
        menu.update(PauseMenuInput{up:true,..Default::default()},true);
        menu.update(PauseMenuInput{select:true,..Default::default()},true);
        assert_eq!(menu.take_sounds(),["FE_SOUND_CORE_NAV_UP","FE_SOUND_CORE_A_BUTTON","FE_SOUND_CORE_POPUP"]);
        menu.update(PauseMenuInput{down:true,..Default::default()},true);
        menu.update(PauseMenuInput{up:true,..Default::default()},true);
        menu.update(PauseMenuInput{select:true,..Default::default()},true);
        menu.update(PauseMenuInput{cancel:true,..Default::default()},true);
        assert_eq!(menu.take_sounds(),["FE_SOUND_CORE_NAV_DOWN","FE_SOUND_CORE_NAV_UP","FE_SOUND_CORE_A_BUTTON",
            "FE_SOUND_PAUSE_MENU_PRESS"]);
        assert!(!menu.snapshot().visible && !menu.take_quit_confirmed());
    }
    #[test]
    fn challenge_hub_follows_the_main_challenge() {
        let mut menu=PauseMenuController::default();
        menu.set_hub(StartHub::SinglePlayer);
        menu.set_main_challenge(Some(MainChallenge{challenge_type:40,quit_enabled:true}));
        let state=menu.update(PauseMenuInput{start:true,..Default::default()},true);
        assert_eq!((state.hub,state.screen_state()),(StartHub::SinglePlayerChallenge,1));
        assert_eq!(menu.selected_option(),PauseMenuOption::RestartChallenge);
        menu.acknowledge_intro();
        menu.update(PauseMenuInput{down:true,..Default::default()},true);
        assert_eq!(menu.selected_option(),PauseMenuOption::QuitChallenge);
        let state=menu.update(PauseMenuInput{select:true,..Default::default()},true);
        assert_eq!(state.popup,Some((HubPopup::QuitChallenge,0)));
        menu.update(PauseMenuInput{down:true,..Default::default()},true);
        let state=menu.update(PauseMenuInput{select:true,..Default::default()},true);
        assert!(!state.visible && menu.take_abandon_confirmed() && !menu.take_quit_confirmed());
        // Own the Spot (9) has no Restart Challenge; without its flag no Quit Challenge either.
        menu.set_main_challenge(Some(MainChallenge{challenge_type:9,quit_enabled:false}));
        menu.update(PauseMenuInput{start:true,..Default::default()},true);
        assert_eq!(menu.selected_option(),PauseMenuOption::Profile);
        menu.close();
        menu.set_main_challenge(Some(MainChallenge{challenge_type:35,quit_enabled:true}));
        let state=menu.update(PauseMenuInput{start:true,..Default::default()},true);
        assert_eq!((state.hub,state.screen_state()),(StartHub::CreateASpot,0));
        assert_eq!(menu.selected_option(),PauseMenuOption::CreateASpotSpotInfo);
        menu.close();
        menu.set_main_challenge(None);
        let state=menu.update(PauseMenuInput{start:true,..Default::default()},true);
        assert_eq!((state.hub,menu.selected_option()),(StartHub::SinglePlayer,PauseMenuOption::Map));
        menu.close();
        // Party Play keeps its hub; Quit Game asks the main challenge's confirmation.
        menu.set_hub(StartHub::PartyPlay);
        menu.set_main_challenge(Some(MainChallenge{challenge_type:40,quit_enabled:true}));
        let state=menu.update(PauseMenuInput{start:true,..Default::default()},true);
        assert_eq!((state.hub,menu.selected_option()),(StartHub::PartyPlay,PauseMenuOption::RestartChallenge));
        menu.acknowledge_intro();
        menu.update(PauseMenuInput{up:true,..Default::default()},true);
        let state=menu.update(PauseMenuInput{select:true,..Default::default()},true);
        assert_eq!(state.popup,Some((HubPopup::Quit,0)));
    }

    #[test]
    fn leave_challenge_sits_above_quit_game_while_the_round_runs() {
        let mut menu=PauseMenuController::default();
        menu.set_hub(StartHub::OnlineChallenge);
        assert_eq!(menu.options(),[PauseMenuOption::Settings,PauseMenuOption::LeaveChallenge,PauseMenuOption::Quit]);
        // A finished round offers no Leave Challenge: down goes from Settings to Quit Game.
        menu.update(PauseMenuInput{start:true,..Default::default()},true);
        menu.acknowledge_intro();
        menu.update(PauseMenuInput{down:true,..Default::default()},true);
        assert_eq!(menu.selected_option(),PauseMenuOption::Quit);
        menu.update(PauseMenuInput{start:true,..Default::default()},true);
        menu.set_leave_challenge(true);
        menu.update(PauseMenuInput{start:true,..Default::default()},true);
        menu.acknowledge_intro();
        menu.update(PauseMenuInput{down:true,..Default::default()},true);
        assert_eq!(menu.selected_option(),PauseMenuOption::LeaveChallenge);
        let state=menu.update(PauseMenuInput{select:true,..Default::default()},true);
        assert_eq!(state.popup,Some((HubPopup::LeaveChallenge,0)));
        // No, the default, leaves the hub up and nothing confirmed.
        let state=menu.update(PauseMenuInput{select:true,..Default::default()},true);
        assert!(state.visible && state.popup.is_none() && !menu.take_leave_challenge_confirmed());
        menu.update(PauseMenuInput{select:true,..Default::default()},true);
        menu.update(PauseMenuInput{down:true,..Default::default()},true);
        let state=menu.update(PauseMenuInput{select:true,..Default::default()},true);
        assert!(!state.visible && menu.take_leave_challenge_confirmed() && !menu.take_quit_confirmed());
        // The round finished with the row selected: A on it opens nothing.
        menu.update(PauseMenuInput{start:true,..Default::default()},true);
        menu.acknowledge_intro();
        menu.update(PauseMenuInput{down:true,..Default::default()},true);
        menu.set_leave_challenge(false);
        let state=menu.update(PauseMenuInput{select:true,..Default::default()},true);
        assert_eq!(state.popup,None);
    }
    #[test]
    fn edit_skater_sits_above_game_settings_while_the_online_game_offers_it() {
        let mut menu=PauseMenuController::default();
        menu.set_hub(StartHub::OnlineFreeSkate);
        assert_eq!(menu.options(),[PauseMenuOption::SaveReplay,PauseMenuOption::EditSkater,PauseMenuOption::Settings,PauseMenuOption::Quit]);
        // Not offered (a round loading, a travel): the hub opens on Game Settings and up wraps
        // past both disabled rows to Quit Game.
        menu.update(PauseMenuInput{start:true,..Default::default()},true);
        menu.acknowledge_intro();
        assert_eq!(menu.selected_option(),PauseMenuOption::Settings);
        menu.update(PauseMenuInput{up:true,..Default::default()},true);
        assert_eq!(menu.selected_option(),PauseMenuOption::Quit);
        menu.update(PauseMenuInput{start:true,..Default::default()},true);
        // Offered: the hub opens on it, and A on it opens no popup (the runtime opens the editor).
        menu.set_edit_skater(true);
        menu.update(PauseMenuInput{start:true,..Default::default()},true);
        menu.acknowledge_intro();
        assert_eq!(menu.selected_option(),PauseMenuOption::EditSkater);
        let state=menu.update(PauseMenuInput{select:true,..Default::default()},true);
        assert!(state.visible && state.popup.is_none() && state.item_enabled(PauseMenuOption::EditSkater));
        menu.set_edit_skater(false);
        assert!(!menu.snapshot().item_enabled(PauseMenuOption::EditSkater));
    }

    #[test]
    fn boot_player_2_sits_above_quit_game_while_the_co_op_player_is_in_the_game() {
        use PauseMenuOption as O;
        let mut menu=PauseMenuController::default();
        let alone=menu.options();
        assert!(!alone.contains(&O::BootPlayer2));
        menu.set_coop_boot(true);
        let with_boot=menu.options();
        assert_eq!(with_boot.len(),alone.len()+1);
        let at=with_boot.iter().position(|&row|row==O::BootPlayer2).unwrap();
        assert_eq!(with_boot[at+1],O::Quit);
        assert_eq!(with_boot.iter().copied().filter(|&row|row!=O::BootPlayer2).collect::<Vec<_>>(),alone.to_vec());
        // A hub without Quit Game gets it at the end.
        let mut spot=PauseMenuController::default();
        spot.set_hub(StartHub::CreateASpot);
        spot.set_coop_boot(true);
        assert_eq!(spot.options(),[O::CreateASpotSpotInfo,O::Replay,O::TrickGuide,O::Settings,O::BootPlayer2]);
        // The first player's hub can boot: A opens the confirmation, which defaults to No.
        menu.update(PauseMenuInput{start:true,..Default::default()},true);
        menu.acknowledge_intro();
        menu.update(PauseMenuInput{up:true,..Default::default()},true);
        assert_eq!(menu.selected_option(),O::Quit);
        menu.update(PauseMenuInput{up:true,..Default::default()},true);
        assert_eq!(menu.selected_option(),O::BootPlayer2);
        let state=menu.update(PauseMenuInput{select:true,..Default::default()},true);
        assert_eq!(state.popup,Some((HubPopup::BootPlayer2,0)));
        let state=menu.update(PauseMenuInput{select:true,..Default::default()},true);
        assert!(state.visible && state.popup.is_none() && !menu.take_boot_player2_confirmed());
        menu.update(PauseMenuInput{select:true,..Default::default()},true);
        menu.update(PauseMenuInput{down:true,..Default::default()},true);
        let state=menu.update(PauseMenuInput{select:true,..Default::default()},true);
        assert!(!state.visible && menu.take_boot_player2_confirmed());
        assert!(!menu.take_boot_player2_confirmed() && !menu.take_quit_confirmed());
    }

    #[test]
    fn the_selection_stays_on_its_row_when_boot_player_2_appears_or_goes() {
        let mut menu=PauseMenuController::default();
        menu.update(PauseMenuInput{start:true,..Default::default()},true);
        menu.acknowledge_intro();
        menu.update(PauseMenuInput{up:true,..Default::default()},true);
        assert_eq!(menu.selected_option(),PauseMenuOption::Quit);
        menu.set_coop_boot(true);
        assert_eq!(menu.selected_option(),PauseMenuOption::Quit);
        menu.update(PauseMenuInput{up:true,..Default::default()},true);
        assert_eq!(menu.selected_option(),PauseMenuOption::BootPlayer2);
        // Player 2 left with the row selected: it moves to a row that is still enabled.
        menu.set_coop_boot(false);
        assert!(menu.snapshot().item_enabled(menu.selected_option()));
        assert!(!menu.options().contains(&PauseMenuOption::BootPlayer2));
    }

    #[test]
    fn the_co_op_player_has_their_own_hub_with_map_settings_and_boot() {
        use PauseMenuOption as O;
        let mut menu=PauseMenuController::default();
        menu.set_hub(StartHub::CoopPlayer2);
        menu.set_coop_boot(true);
        assert_eq!(menu.options(),[O::Map,O::Settings,O::BootPlayer2]);
        // A main challenge never swaps it for a Create-a-Spot hub, and it has no Quit Game.
        menu.set_main_challenge(Some(MainChallenge{challenge_type:36,quit_enabled:true}));
        let state=menu.update(PauseMenuInput{start:true,..Default::default()},true);
        assert_eq!(state.hub,StartHub::CoopPlayer2);
        assert!(!state.item_enabled(O::Quit) && state.item_enabled(O::Map) && state.item_enabled(O::Settings));
        menu.acknowledge_intro();
        assert_eq!(menu.selected_option(),O::Map);
        // Up wraps from the first row to Boot Player 2, which asks to leave the game.
        menu.update(PauseMenuInput{up:true,..Default::default()},true);
        assert_eq!(menu.selected_option(),O::BootPlayer2);
        let state=menu.update(PauseMenuInput{select:true,..Default::default()},true);
        assert_eq!(state.popup,Some((HubPopup::LeaveCoop,0)));
        menu.update(PauseMenuInput{select:true,..Default::default()},true);
        assert!(!menu.take_boot_player2_confirmed());
        menu.update(PauseMenuInput{select:true,..Default::default()},true);
        menu.update(PauseMenuInput{down:true,..Default::default()},true);
        let state=menu.update(PauseMenuInput{select:true,..Default::default()},true);
        assert!(!state.visible && menu.take_boot_player2_confirmed());
    }

    #[test]
    fn the_co_op_hub_shows_no_wallet_like_free_skate() {
        let mut menu=PauseMenuController::default();
        menu.set_hub(StartHub::CoopPlayer2);
        assert!(!menu.snapshot().shows_cash());
    }
}

impl PauseMenuOption {
    /// IStartHubState::m_pStartHubOptionDefinitions at 82fdfc54.
    #[must_use]
    pub const fn localization_key(self) -> &'static str {
        match self {
            Self::Replay => "ID_START_HUB_SKATE_REPLAY_EDITOR",
            Self::Map => "ID_START_HUB_CHALLENGE_MAP",
            Self::Blog => "ID_START_HUB_SLAPPYS_BLOG",
            Self::Profile => "ID_START_HUB_PLAYER_PROFILE",
            Self::EditSkater => "ID_START_HUB_EDIT_SKATER",
            Self::TrickGuide => "ID_START_HUB_TRICK_GUIDE",
            Self::Settings => "ID_START_HUB_GAME_SETTINGS",
            Self::Quit => "ID_START_HUB_QUIT_GAME",
            Self::QuitCreateASpot => "ID_START_HUB_QUIT_CAS",
            Self::CreateASpotSpotInfo => "ID_CREATEASPOT_RETURN_TO_SPOT_INFO",
            Self::RestartChallenge => "ID_START_HUB_RESTART_CHALLENGE",
            Self::QuitChallenge => "ID_START_HUB_QUIT_CHALLENGE",
            Self::SaveReplay => "ID_START_HUB_SAVE_REPLAY",
            Self::LeaveChallenge => "#Leave Challenge",
            Self::BootPlayer2 => "#Boot Player 2",
        }
    }

    /// SinglePlayer::IsItemEnabled82644600. Replay requires 150 tracked frames
    /// (OnEnter8265e4d0); Snapshot::item_enabled supplies that runtime gate.
    #[must_use]
    pub const fn enabled(self) -> bool {
        !matches!(self, Self::Replay)
    }

    /// `IStartHubState::m_pStartHubOptionDefinitionsNew` (`0x82fdfc8c`): the labels with a count
    /// that `StartHubState_SinglePlayer::GetOptionText` (`0x8264c788`) uses; the other rows have
    /// the string "NULL", which it never reads.
    #[must_use]
    pub const fn new_localization_key(self) -> Option<&'static str> {
        match self {
            Self::Map => Some("ID_START_HUB_CHALLENGE_MAP_NEW"),
            Self::Blog => Some("ID_START_HUB_SLAPPYS_BLOG_NEW"),
            _ => None,
        }
    }
}

/// SinglePlayer::m_eMenuOptions8208e248 (GetOption82644370) without the debug build's first
/// row, Debug (`ID_START_HUB_DEBUG_MENU`), which a retail hub does not offer.
pub const SINGLE_PLAYER_OPTIONS: [PauseMenuOption; 8] = [
    PauseMenuOption::Replay,
    PauseMenuOption::Map,
    PauseMenuOption::Profile,
    PauseMenuOption::Blog,
    PauseMenuOption::EditSkater,
    PauseMenuOption::TrickGuide,
    PauseMenuOption::Settings,
    PauseMenuOption::Quit,
];

/// Offline_FreeSkate::m_eMenuOptions8208e230 (GetOption82644358), the hub
/// `OnIntroDone`82653918 picks in a free-skate game mode. Its IsItemEnabled826444e0
/// enables Replay only with mbReplayAvailable, as SinglePlayer's does. Its debug Debug row is
/// left out.
pub const OFFLINE_FREESKATE_OPTIONS: [PauseMenuOption; 5] = [
    PauseMenuOption::Replay,
    PauseMenuOption::Map,
    PauseMenuOption::TrickGuide,
    PauseMenuOption::Settings,
    PauseMenuOption::Quit,
];

/// PartyPlay::m_eMenuOptions8208e2cc (GetOption82644400), the hub `OnIntroDone`82653918
/// picks in Party Play. Its IsItemEnabled82644570 enables Replay only with mbReplayAvailable.
/// Its debug Debug row is left out.
pub const PARTY_PLAY_OPTIONS: [PauseMenuOption; 5] = [
    PauseMenuOption::RestartChallenge,
    PauseMenuOption::Replay,
    PauseMenuOption::TrickGuide,
    PauseMenuOption::Settings,
    PauseMenuOption::Quit,
];

/// `StartHubState_SinglePlayer_Challenge::m_eMenuOptions` (`0x8208e26c`, eight rows from
/// `GetNumOptions`), the single-player hub while a main challenge runs. Its debug Debug row is
/// left out.
pub const SINGLE_PLAYER_CHALLENGE_OPTIONS: [PauseMenuOption; 7] = [
    PauseMenuOption::RestartChallenge,
    PauseMenuOption::QuitChallenge,
    PauseMenuOption::Replay,
    PauseMenuOption::Profile,
    PauseMenuOption::TrickGuide,
    PauseMenuOption::Settings,
    PauseMenuOption::Quit,
];

/// `StartHubState_Create_A_Spot::m_eMenuOptions` (`0x8208e28c`, five rows), during a
/// Create-a-Spot challenge (type 35). Its debug Debug row is left out.
pub const CREATE_A_SPOT_OPTIONS: [PauseMenuOption; 4] = [
    PauseMenuOption::CreateASpotSpotInfo,
    PauseMenuOption::Replay,
    PauseMenuOption::TrickGuide,
    PauseMenuOption::Settings,
];

/// `StartHubState_Create_A_Spot_Setup::m_eMenuOptions` (`0x8208e2a0`, six rows). Its debug Debug
/// row is left out.
pub const CREATE_A_SPOT_SETUP_OPTIONS: [PauseMenuOption; 5] = [
    PauseMenuOption::QuitCreateASpot,
    PauseMenuOption::Replay,
    PauseMenuOption::TrickGuide,
    PauseMenuOption::Settings,
    PauseMenuOption::Quit,
];

/// `StartHubState_Create_A_Spot_Setup_WithScore::m_eMenuOptions` (`0x8208e2b8`, five rows),
/// during a Create-a-Spot setup challenge (type 36). Its debug Debug row is left out.
pub const CREATE_A_SPOT_SETUP_WITH_SCORE_OPTIONS: [PauseMenuOption; 4] = [
    PauseMenuOption::CreateASpotSpotInfo,
    PauseMenuOption::Replay,
    PauseMenuOption::TrickGuide,
    PauseMenuOption::Settings,
];

/// `StartHubState_Online_FreeSkate::m_eMenuOptions` (`0x8208e2e4`, four rows). Its debug Debug
/// row is left out; Edit Skater above Game Settings is a PC port addition.
pub const ONLINE_FREESKATE_OPTIONS: [PauseMenuOption; 4] =
    [PauseMenuOption::SaveReplay, PauseMenuOption::EditSkater, PauseMenuOption::Settings, PauseMenuOption::Quit];

/// `StartHubState_Online_Challenge::m_eMenuOptions` (`0x8208e2f4`, three rows). Its debug Debug
/// row is left out; Leave Challenge above Quit Game is a PC port addition.
pub const ONLINE_CHALLENGE_OPTIONS: [PauseMenuOption; 3] =
    [PauseMenuOption::Settings, PauseMenuOption::LeaveChallenge, PauseMenuOption::Quit];

/// The co-op player's own pause menu (PC port addition): the Challenge Map to teleport with,
/// Game Settings, and Boot Player 2, which takes that player out of the game. It has no Quit
/// Game: only the first player ends the game.
pub const COOP_PLAYER2_OPTIONS: [PauseMenuOption; 3] =
    [PauseMenuOption::Map, PauseMenuOption::Settings, PauseMenuOption::BootPlayer2];

/// A hub's rows: its authored `m_eMenuOptions` and, while the co-op session offers it, Boot
/// Player 2 above Quit Game (at the end of a hub without one), a PC port addition. A small
/// stack value that dereferences to the rows.
#[derive(Clone, Copy)]
pub struct HubOptions {
    rows: [PauseMenuOption; Self::CAPACITY],
    len: usize,
}

impl HubOptions {
    /// The longest hub (`SINGLE_PLAYER_OPTIONS`, eight rows) and Boot Player 2.
    const CAPACITY: usize = 9;

    /// The authored rows, with Boot Player 2 added when `boot` is set.
    #[must_use]
    pub const fn new(authored: &[PauseMenuOption], boot: bool) -> Self {
        let mut rows = [PauseMenuOption::Map; Self::CAPACITY];
        let mut len = 0;
        let mut boot_pending = boot;
        let mut index = 0;
        while index < authored.len() {
            if boot_pending && matches!(authored[index], PauseMenuOption::Quit) {
                rows[len] = PauseMenuOption::BootPlayer2;
                len += 1;
                boot_pending = false;
            }
            rows[len] = authored[index];
            len += 1;
            index += 1;
        }
        if boot_pending {
            rows[len] = PauseMenuOption::BootPlayer2;
            len += 1;
        }
        Self { rows, len }
    }
}

impl std::ops::Deref for HubOptions {
    type Target = [PauseMenuOption];

    fn deref(&self) -> &[PauseMenuOption] {
        &self.rows[..self.len]
    }
}

impl std::fmt::Debug for HubOptions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Debug::fmt(&**self, f)
    }
}

impl PartialEq for HubOptions {
    fn eq(&self, other: &Self) -> bool {
        **self == **other
    }
}

impl Eq for HubOptions {}

impl<const N: usize> PartialEq<[PauseMenuOption; N]> for HubOptions {
    fn eq(&self, other: &[PauseMenuOption; N]) -> bool {
        **self == *other
    }
}

/// The main challenge as the hub reads it (`cActivityManager::GetMainChallenge`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MainChallenge {
    /// Its `challenge_global_data` `ChallengeType` (`eChallengeTypes`).
    pub challenge_type: i32,
    /// Its `challenge_local_data` 0x23F394473183C613, which
    /// `StartHubState_SinglePlayer_Challenge::IsItemEnabled` (`0x8264c560`) reads for Quit
    /// Challenge.
    pub quit_enabled: bool,
}

/// The hub state `FrontEndState_StartHub::OnIntroDone` (`0x826536e0`) picks: a main challenge of
/// type 36 or 35 gives the Create-a-Spot hubs; then Party Play, the online hubs, free skate (or a
/// mode other than single player), and the single-player hub, its challenge variant while a main
/// challenge runs.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum StartHub {
    /// `StartHubState_SinglePlayer`.
    #[default]
    SinglePlayer,
    /// `StartHubState_SinglePlayer_Challenge`, which sets screen state 1.
    SinglePlayerChallenge,
    /// `StartHubState_Offline_FreeSkate` (`GameBackEnd::IsFreeSkateMode`).
    OfflineFreeSkate,
    /// `StartHubState_PartyPlay` (`GameBackEnd::IsPartyPlayMode`).
    PartyPlay,
    /// `StartHubState_Create_A_Spot` (a main challenge of type 35).
    CreateASpot,
    /// `StartHubState_Create_A_Spot_Setup`, which `OnIntroDone` does not pick.
    CreateASpotSetup,
    /// `StartHubState_Create_A_Spot_Setup_WithScore` (a main challenge of type 36).
    CreateASpotSetupWithScore,
    /// `StartHubState_Online_FreeSkate` and `StartHubState_Online_Challenge`
    /// (`GameBackEnd::IsOnlineMode`, which the port never is).
    OnlineFreeSkate,
    OnlineChallenge,
    /// The co-op player's own hub (PC port addition): [`COOP_PLAYER2_OPTIONS`].
    CoopPlayer2,
}

impl StartHub {
    /// `OnIntroDone`'s pick for a game mode's hub (`SinglePlayer`, `OfflineFreeSkate` or
    /// `PartyPlay`) and the main challenge.
    #[must_use]
    pub const fn for_challenge(self, main: Option<MainChallenge>) -> Self {
        match main {
            _ if matches!(self, Self::CoopPlayer2) => self,
            Some(MainChallenge { challenge_type: 36, .. }) => Self::CreateASpotSetupWithScore,
            Some(MainChallenge { challenge_type: 35, .. }) => Self::CreateASpot,
            Some(_) if matches!(self, Self::SinglePlayer) => Self::SinglePlayerChallenge,
            _ => self,
        }
    }
}

/// A hub confirmation (`FrontEndManager::OpenPopup`82614b58): Quit Game's
/// `eSk8FEPopup_QuitGameConfirmation` (152) while a main challenge runs, else
/// `QuitGameConfirmationNoDataLoss` (153); Restart Challenge's `Options_RetryChallenge` (204);
/// Quit Challenge's `QuitChallengeConfirmation` (154). Each template (gPopupTemplate82fdbc58) is
/// No/Yes under `ID_COMMON_WARNING_TITLE`, defaulting to No and not cancellable. Leave
/// Challenge's, the same, is a PC port addition, as are Boot Player 2's (the first player's menu)
/// and Leave Co-op's (the co-op player's own).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HubPopup {
    Quit,
    QuitNoDataLoss,
    RetryChallenge,
    QuitChallenge,
    LeaveChallenge,
    BootPlayer2,
    LeaveCoop,
}

impl HubPopup {
    #[must_use]
    pub const fn message(self) -> &'static str {
        match self {
            Self::Quit => "ID_QUIT_GAME_MESSAGE",
            Self::QuitNoDataLoss => "ID_QUIT_GAME_MESSAGE_NO_DATA_LOSS",
            Self::RetryChallenge => "ID_OPTIONS_MENU_RETRY_CHALLENGE",
            Self::QuitChallenge => "ID_QUIT_CHALLENGE_MESSAGE",
            Self::LeaveChallenge => "#Leave the challenge? You will wait in the pre-game lobby until the others finish.",
            Self::BootPlayer2 => "#Boot Player 2 from the game?",
            Self::LeaveCoop => "#Leave the game? Player 2 is removed from the session.",
        }
    }
}

/// Events from the retained original input map. Directions are repeat pulses;
/// Start/cancel/select are press pulses. This type never polls a device.
#[derive(Clone, Copy, Debug, Default)]
pub struct PauseMenuInput {
    pub start: bool,
    pub cancel: bool,
    pub up: bool,
    pub down: bool,
    pub select: bool,
    pub left: bool,
    pub right: bool,
    pub morph_axis: f32,
    pub morph_axis_secondary: f32,
    /// `AptOption` (Y), FE keycode 4.
    pub option: bool,
    /// `AptMisc` (X), FE keycode 5.
    pub misc: bool,
    /// `AptLShift2` (left trigger), FE keycode 10.
    pub left_shift: bool,
    /// `AptRShift2` (right trigger), FE keycode 7.
    pub right_shift: bool,
    /// `AptLeftReleased` and `AptRightReleased` (`LoadInputMap` states 164 and 165).
    pub left_released: bool,
    pub right_released: bool,
    /// Back, FE keycode 1 (`LoadInputMap` state 135).
    pub back: bool,
    /// `AptLShift2Released` and `AptRShift2Released` (`LoadInputMap` states 188 and 185): the
    /// triggers let go.
    pub left_shift_released: bool,
    pub right_shift_released: bool,
    /// `AptDUp` and `AptDDown` (`LoadInputMap` states 150 and 151): the D-pad alone, without the
    /// stick navigation `AptUp` and `AptDown` take (PC port: the Stick Settings pages).
    pub dpad_up: bool,
    pub dpad_down: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PauseMenuSnapshot {
    pub visible: bool,
    pub selected: usize,
    pub jacked: bool,
    /// The open confirmation and its selected row.
    pub popup: Option<(HubPopup, usize)>,
    pub hub: StartHub,
    /// The game mode's hub (`SinglePlayer`, `OfflineFreeSkate` or `PartyPlay`), which
    /// `OnIntroDone` refines by the main challenge.
    pub mode: StartHub,
    /// The main challenge as the hub's `IsItemEnabled` reads it.
    pub main: Option<MainChallenge>,
    /// `FrontEndState::ShowFEScreen(false)` (`0x826572f8`; the screen manager's `ShowScreen` sets
    /// the screen's `_visible`): the hub's screen is hidden while its state stays on the stack.
    pub screen_hidden: bool,
    /// StartHub::OnEnter8265e4d0 samples GetNumFramesTracked >=150.
    pub replay_available: bool,
    /// Leave Challenge is offered: the local player plays an online challenge's round and has
    /// not finished it (PC port addition).
    pub leave_challenge: bool,
    /// Edit Skater is offered in an online game: the local player is in its pre-game lobby, with
    /// no round, travel or activity start coming (PC port addition).
    pub edit_skater: bool,
    /// The co-op player is in the game, so the hub offers Boot Player 2 (PC port addition).
    pub coop_boot: bool,
}

impl PauseMenuSnapshot {
    /// The hub's `GetOptionText`: the option's label (`m_pStartHubOptionDefinitions`), but in
    /// `SinglePlayer` (`0x8264c788`) Challenge Map with new challenges and Slappy's Blog with new
    /// entries take their "new" label (`FormatString`, the count then printed into it by
    /// `Snprintf`).
    #[must_use]
    pub const fn option_label(self, option: PauseMenuOption, new_challenges: i32, new_blogs: i32) -> (&'static str, Option<i32>) {
        let count = match option {
            PauseMenuOption::Map => new_challenges,
            PauseMenuOption::Blog => new_blogs,
            _ => 0,
        };
        match option.new_localization_key() {
            Some(key) if matches!(self.hub, StartHub::SinglePlayer) && count > 0 => (key, Some(count)),
            _ => (option.localization_key(), None),
        }
    }

    /// challenge_info's wallet (`EnableCash`): always in the challenge mode (`SetMode(1)`); in the
    /// pause mode (`SetMode(0)`, `0x49a4`) unless the game is Party Play or free skate
    /// (`Game.IsPartyPlayMode`, `Game.IsFreeSkateMode`).
    #[must_use]
    pub const fn shows_cash(self) -> bool {
        self.screen_state() == 1
            || !matches!(self.mode, StartHub::OfflineFreeSkate | StartHub::PartyPlay | StartHub::CoopPlayer2)
    }

    /// The hub's `m_eMenuOptions`, with Boot Player 2 while the co-op player is in the game.
    #[must_use]
    pub const fn options(self) -> HubOptions {
        let authored: &[PauseMenuOption] = match self.hub {
            StartHub::SinglePlayer => &SINGLE_PLAYER_OPTIONS,
            StartHub::SinglePlayerChallenge => &SINGLE_PLAYER_CHALLENGE_OPTIONS,
            StartHub::OfflineFreeSkate => &OFFLINE_FREESKATE_OPTIONS,
            StartHub::PartyPlay => &PARTY_PLAY_OPTIONS,
            StartHub::CreateASpot => &CREATE_A_SPOT_OPTIONS,
            StartHub::CreateASpotSetup => &CREATE_A_SPOT_SETUP_OPTIONS,
            StartHub::CreateASpotSetupWithScore => &CREATE_A_SPOT_SETUP_WITH_SCORE_OPTIONS,
            StartHub::OnlineFreeSkate => &ONLINE_FREESKATE_OPTIONS,
            StartHub::OnlineChallenge => &ONLINE_CHALLENGE_OPTIONS,
            StartHub::CoopPlayer2 => &COOP_PLAYER2_OPTIONS,
        };
        // The co-op player's hub lists Boot Player 2 itself.
        HubOptions::new(authored, self.coop_boot && !matches!(self.hub, StartHub::CoopPlayer2))
    }

    /// The hub's `IsItemEnabled`: SinglePlayer (`0x82644600`, as [`PauseMenuOption::enabled`]),
    /// SinglePlayer_Challenge (`0x8264c560`: Restart Challenge outside Own the Spot (9), Quit
    /// Challenge with the main challenge's flag), Offline_FreeSkate (`0x826444e0`), PartyPlay
    /// (`0x82644570`), Create_A_Spot (`0x826446a8`), Create_A_Spot_Setup (`0x82644710`),
    /// Create_A_Spot_Setup_WithScore (`0x82644770`), Online_FreeSkate (`0x8265e928`, Save Replay
    /// only online; Edit Skater while the game offers it, a PC port addition) and Online_Challenge
    /// (`0x826447d0`). Replay needs `mbReplayAvailable`, which is sampled when the hub opens.
    #[must_use]
    pub fn item_enabled(self, option: PauseMenuOption) -> bool {
        use PauseMenuOption as O;
        if option == O::BootPlayer2 {
            return self.coop_boot;
        }
        if option==O::Replay {
            return self.replay_available && matches!(self.hub,StartHub::SinglePlayer|StartHub::SinglePlayerChallenge|StartHub::OfflineFreeSkate|StartHub::PartyPlay);
        }
        match self.hub {
            StartHub::SinglePlayer => option.enabled(),
            StartHub::SinglePlayerChallenge => match option {
                O::QuitChallenge => self.main.is_some_and(|main| main.quit_enabled),
                O::RestartChallenge => self.main.is_some_and(|main| main.challenge_type != 9),
                O::Profile | O::Blog | O::EditSkater | O::Settings | O::TrickGuide | O::Quit => true,
                _ => false,
            },
            StartHub::OfflineFreeSkate => matches!(option, O::Map | O::Settings | O::TrickGuide | O::Quit),
            StartHub::PartyPlay => matches!(option, O::RestartChallenge | O::Settings | O::TrickGuide | O::Quit),
            StartHub::CreateASpot => {
                matches!(option, O::CreateASpotSpotInfo | O::QuitChallenge | O::Settings | O::TrickGuide | O::Quit)
            }
            StartHub::CreateASpotSetup => matches!(option, O::QuitCreateASpot | O::Settings | O::TrickGuide | O::Quit),
            StartHub::CreateASpotSetupWithScore => {
                matches!(option, O::CreateASpotSpotInfo | O::QuitChallenge | O::Settings | O::TrickGuide)
            }
            StartHub::OnlineFreeSkate => {
                matches!(option, O::Map | O::Settings | O::Quit) || (option == O::EditSkater && self.edit_skater)
            }
            StartHub::OnlineChallenge => {
                matches!(option, O::QuitChallenge | O::Settings | O::Quit) || (option == O::LeaveChallenge && self.leave_challenge)
            }
            StartHub::CoopPlayer2 => matches!(option, O::Map | O::Settings),
        }
    }

    /// The first row `IsItemEnabled` allows, where the hub's picker opens.
    #[must_use]
    pub fn first_enabled(self) -> usize {
        self.options().iter().position(|&option| self.item_enabled(option)).unwrap_or(0)
    }

    /// `OnIntroDone`'s `SetScreenState`: 1 for the in-challenge single-player hub (starthubmenu
    /// `SetMode(1)`: challenge_info's challenge mode and `mHub`'s `tall`), else 0.
    #[must_use]
    pub const fn screen_state(self) -> i32 {
        if matches!(self.hub, StartHub::SinglePlayerChallenge) { 1 } else { 0 }
    }
}

#[derive(Clone, Debug, Default)]
pub struct PauseMenuController {
    state: PauseMenuSnapshot,
    pending_jacked: bool,
    /// `FrontEndStateBase::mLocalInputState` at `eInputState_Block`: the hub takes no input
    /// (`QuitPopupCallback`'s save, `DoQuit`).
    input_blocked: bool,
    /// `QuitPopupCallback`'s profile save is being written.
    saving: bool,
    quit_confirmed: bool,
    retry_confirmed: bool,
    /// `QuitChallengeCallback` (`0x8265e868`) answered yes.
    abandon_confirmed: bool,
    /// Leave Challenge's confirmation answered yes.
    leave_challenge_confirmed: bool,
    /// Boot Player 2's (or Leave Co-op's) confirmation answered yes.
    boot_player2_confirmed: bool,
    /// `FrontEndState_StartHub::mbDoingQuit`: `DoQuit` (`0x8266b1a0`) faded the background in,
    /// whose callback `FadeToExit` (`0x8265e7f0`) pops the hub and calls `ExitToMainMenu`.
    doing_quit: bool,
    /// `FE_SOUND_*` globals this input played, in order: `GlobalFEPlaySound` calls of the
    /// front-end states and the `Audio.PlaySound` calls of the pickers' scripts.
    sounds: Vec<&'static str>,
    /// `IsScreenReady` 0x82d661a8: the hub takes input once its screen's `IntroComplete`
    /// (`OnScreenReady` 0x82d65f98) ran.
    ready: bool,
}

impl PauseMenuController {
    pub fn set_replay_available(&mut self,available:bool) {self.state.replay_available=available;}
    pub fn hide_for_replay(&mut self) {self.state.screen_hidden=true;}
    pub fn return_from_replay(&mut self) {self.state.screen_hidden=false;self.reenter();}
    /// FrontEnd.PushState42; gFEStateInfo42 requests FRONTEND pause and dimmer.
    pub fn request_jacked(&mut self) { self.pending_jacked=true; }
    #[must_use]
    pub fn selected_option(&self) -> PauseMenuOption {
        self.options()[self.state.selected]
    }

    #[must_use]
    pub const fn options(&self) -> HubOptions {
        self.state.options()
    }

    pub fn set_hub(&mut self, hub: StartHub) {
        self.state.mode = hub;
        self.state.hub = hub;
        self.state.selected = self.state.first_enabled();
    }

    /// `cActivityManager::GetMainChallenge` as the hub reads it: Quit Game's popup, the hub
    /// variant and `IsItemEnabled`.
    pub fn set_main_challenge(&mut self, main: Option<MainChallenge>) {
        self.state.main = main;
    }

    /// `QuitChallengeCallback`'s yes, which also pops the hub: the runtime sends
    /// `cMsgAbandonChallenge` with `IsQuitting` false.
    pub fn take_abandon_confirmed(&mut self) -> bool {
        std::mem::take(&mut self.abandon_confirmed)
    }

    /// Whether Leave Challenge is offered (PC port addition).
    pub fn set_leave_challenge(&mut self, offered: bool) {
        self.state.leave_challenge = offered;
    }

    /// Whether the online game offers Edit Skater (PC port addition).
    pub fn set_edit_skater(&mut self, offered: bool) {
        self.state.edit_skater = offered;
    }

    /// Leave Challenge's yes, which also pops the hub: the runtime leaves the round for the
    /// pre-game lobby.
    pub fn take_leave_challenge_confirmed(&mut self) -> bool {
        std::mem::take(&mut self.leave_challenge_confirmed)
    }

    /// Whether the co-op player is in the game, so the hub offers Boot Player 2 (PC port
    /// addition). The selection is kept on its row.
    pub fn set_coop_boot(&mut self, offered: bool) {
        if self.state.coop_boot == offered {
            return;
        }
        let kept = self.options().get(self.state.selected).copied();
        self.state.coop_boot = offered;
        self.state.selected = kept
            .and_then(|option| self.options().iter().position(|&row| row == option))
            .unwrap_or_else(|| self.state.first_enabled());
    }

    /// Boot Player 2's yes (the co-op player's own hub: Leave Co-op's), which also pops the hub:
    /// the runtime removes the co-op player.
    pub fn take_boot_player2_confirmed(&mut self) -> bool {
        std::mem::take(&mut self.boot_player2_confirmed)
    }

    /// `QuitPopupCallback`'s save (`0x82671678`): the hub's input blocked and its screen hidden
    /// (`ShowFEScreen(false)`) while the profile is written.
    pub fn hide_for_save(&mut self) {
        self.input_blocked = true;
        self.state.screen_hidden = true;
        self.saving = true;
    }

    /// The quit's profile save is being written.
    #[must_use]
    pub const fn saving(&self) -> bool {
        self.saving
    }

    /// The save's `OnComplete` or `OnFail` (`0x82671960`, `0x8266b268`), the hub in mode 0 or 1:
    /// `DoQuit`.
    pub fn save_finished(&mut self) {
        self.saving = false;
        self.do_quit();
    }

    /// `DoQuit` (`0x8266b1a0`): the hub's input blocked and the quit's fade running.
    pub fn do_quit(&mut self) {
        self.doing_quit = true;
        self.input_blocked = true;
    }

    /// The quit's fade is over: `FadeToExit` (`0x8265e7f0`) runs.
    pub fn take_doing_quit(&mut self) -> bool {
        std::mem::take(&mut self.doing_quit)
    }

    /// `OnIntroDone`'s hub for the game mode and the main challenge, on its first enabled row.
    fn pick_hub(&mut self) {
        self.state.hub = self.state.mode.for_challenge(self.state.main);
        self.state.selected = self.state.first_enabled();
    }

    pub fn take_quit_confirmed(&mut self)->bool {std::mem::take(&mut self.quit_confirmed)}

    /// `RetrySelectedCallback`8265ea20's yes, which also pops the hub: the runtime restarts the
    /// main challenge (`RestartMainChallenge(0, false)`).
    pub fn take_retry_confirmed(&mut self) -> bool {
        std::mem::take(&mut self.retry_confirmed)
    }

    pub fn take_sounds(&mut self) -> Vec<&'static str> { std::mem::take(&mut self.sounds) }

    /// The hub's screen ran its `IntroComplete`.
    pub fn acknowledge_intro(&mut self) {
        if self.state.visible && !self.state.jacked {
            self.ready = true;
        }
    }

    /// `ExecutePopState` 0x82d672f8's `UpdateOnEnter(7)` as a state pushed over the hub pops:
    /// `OnEnter` 0x8265e4d0 opens a new screen, whose picker starts on the first enabled row and
    /// takes no input before its intro completes.
    pub fn reenter(&mut self) {
        self.ready = false;
        self.pick_hub();
    }

    /// The hub is up and its screen not ready: `StateManagerFrontEnd::ReceiveInput` 0x82656af8
    /// hands it nothing.
    #[must_use]
    pub const fn awaiting_intro(&self) -> bool {
        self.state.visible && !self.state.jacked && !self.ready
    }

    pub fn close(&mut self) {
        self.state.popup=None;
        self.state.visible = false;
        self.state.screen_hidden = false;
        self.input_blocked = false;
        self.saving = false;
        self.ready = false;
    }

    #[must_use]
    pub const fn snapshot(&self) -> PauseMenuSnapshot {
        self.state
    }

    /// StartHub::ReceiveInput8265e778 closes on AptStart0 or AptPrevious3.
    /// starthub SetState0 selects row0; menu_picker Prev/Next wraps and skips
    /// disabled rows. The runtime owns native PauseMan Push/Pull FRONTEND3 and
    /// observes the visibility transition before advancing any simulation.
    pub fn update(&mut self, input: PauseMenuInput, focused: bool) -> PauseMenuSnapshot {
        if self.pending_jacked {
            self.pending_jacked=false;
            self.state=PauseMenuSnapshot{visible:true,selected:0,jacked:true,popup:None,..self.state};
            return self.state;
        }
        if !focused || self.input_blocked {
            return self.state;
        }
        if self.state.jacked {
            // JackedOverlay.ReceiveInput826598f0 pops only AptNext (A).
            if input.select { self.state.visible=false;self.state.jacked=false; }
            return self.state;
        }
        if !self.state.visible {
            if input.start {
                self.state = PauseMenuSnapshot { visible: true, jacked: false, popup: None, ..self.state };
                self.pick_hub();
                // StateManagerFrontEnd::CheckForOptions82656968 sounds pause_menu_press
                // before PushState(7).
                self.sounds.push("FE_SOUND_PAUSE_MENU_PRESS");
                self.ready = false;
            }
            return self.state;
        }
        if !self.ready {
            return self.state;
        }
        if let Some((popup, mut selected))=self.state.popup {
            // Each template is No/Yes, default No, not cancellable; QuitPopupCallback82671678
            // and RetrySelectedCallback8265ea20 act only for index1. Start/B therefore cannot
            // dismiss these popups. sk8popup hands every other key to its menu_picker, which
            // sounds each move (both wrap here) and A.
            if input.up {selected=(selected+1)%2;self.sounds.push("FE_SOUND_CORE_NAV_UP");}
            if input.down {selected=(selected+1)%2;self.sounds.push("FE_SOUND_CORE_NAV_DOWN");}
            if input.select {self.sounds.push("FE_SOUND_CORE_A_BUTTON");}
            self.state.popup=if input.select {None}else{Some((popup, selected))};
            if input.select && selected==1 {
                match popup {
                    HubPopup::RetryChallenge => {
                        self.retry_confirmed = true;
                        self.state.visible = false;
                    }
                    HubPopup::QuitChallenge => {
                        self.abandon_confirmed = true;
                        self.state.visible = false;
                    }
                    HubPopup::LeaveChallenge => {
                        self.leave_challenge_confirmed = true;
                        self.state.visible = false;
                    }
                    HubPopup::BootPlayer2 | HubPopup::LeaveCoop => {
                        self.boot_player2_confirmed = true;
                        self.state.visible = false;
                    }
                    HubPopup::Quit | HubPopup::QuitNoDataLoss => self.quit_confirmed = true,
                }
            }
            return self.state;
        }
        if input.start || input.cancel {
            // StartHub::ReceiveInput8265e778 sounds pause_menu_press before PopState.
            self.sounds.push("FE_SOUND_PAUSE_MENU_PRESS");
            self.state.visible = false;
            return self.state;
        }
        // The original input table dispatches AptUp before AptDown.
        for forward in [input.up.then_some(false), input.down.then_some(true)]
            .into_iter()
            .flatten()
        {
            let previous = self.state.selected;
            loop {
                let options = self.options();
                self.state.selected = if forward {
                    (self.state.selected + 1) % options.len()
                } else {
                    (self.state.selected + options.len() - 1) % options.len()
                };
                if self.state.item_enabled(options[self.state.selected]) {
                    break;
                }
            }
            // menu_picker Next/PrevValue sound only a value that changed.
            if self.state.selected != previous {
                self.sounds.push(if forward { "FE_SOUND_CORE_NAV_DOWN" } else { "FE_SOUND_CORE_NAV_UP" });
            }
        }
        // AptNext dispatches OnMenuSelect82644478. Runtime owns the destination
        // stack and reads selected_option after this authored picker update.
        if input.select {
            // menu_picker::OnMenuSelect sounds A before Game.OnMenuSelect.
            self.sounds.push("FE_SOUND_CORE_A_BUTTON");
        }
        // IStartHubState::OnMenuSelect82674d08 opens Quit Game's, Restart Challenge's and Quit
        // Challenge's confirmations.
        let popup = match self.selected_option() {
            PauseMenuOption::Quit if self.state.main.is_some() => Some(HubPopup::Quit),
            PauseMenuOption::Quit => Some(HubPopup::QuitNoDataLoss),
            PauseMenuOption::RestartChallenge => Some(HubPopup::RetryChallenge),
            PauseMenuOption::QuitChallenge => Some(HubPopup::QuitChallenge),
            PauseMenuOption::LeaveChallenge if self.state.leave_challenge => Some(HubPopup::LeaveChallenge),
            PauseMenuOption::BootPlayer2 if self.state.coop_boot && matches!(self.state.hub, StartHub::CoopPlayer2) => {
                Some(HubPopup::LeaveCoop)
            }
            PauseMenuOption::BootPlayer2 if self.state.coop_boot => Some(HubPopup::BootPlayer2),
            _ => None,
        };
        if input.select && let Some(popup) = popup {
            self.state.popup=Some((popup, 0));
            // FrontEndState_Popup::OnEnter82658474 sounds core_popup.
            self.sounds.push("FE_SOUND_CORE_POPUP");
        }
        self.state
    }
}
