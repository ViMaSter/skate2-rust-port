//! Split-screen pause menu controller for player-independent navigation.
//!
//! Each player can pause the game and see their own menu with independent navigation.
//! Either player can remove the other player from the session.

use std::fmt;

/// Pause menu state for one player.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PauseMenuState {
    /// Menu is not open.
    Closed,
    /// Menu is open and active.
    Open,
    /// Menu is transitioning (fade in/out).
    Transitioning,
}

/// Options available in the pause menu for split-screen two-player.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PauseMenuOption {
    /// Resume gameplay.
    Resume,
    /// Remove the other player from the session.
    RemoveOtherPlayer,
    /// Quit to main menu.
    QuitToMenu,
    /// Settings (graphics, audio, controls, etc.).
    Settings,
}

impl PauseMenuOption {
    /// All available menu options.
    pub const ALL: [Self; 4] = [
        Self::Resume,
        Self::RemoveOtherPlayer,
        Self::QuitToMenu,
        Self::Settings,
    ];
}

/// Pause menu navigation and selection for one player.
pub struct PauseMenuController {
    /// Current menu state.
    state: PauseMenuState,
    /// Currently highlighted option (index in the menu).
    selected_index: usize,
    /// Whether this player has triggered removal of the other player.
    removal_triggered: bool,
}

impl PauseMenuController {
    /// Create a new pause menu controller.
    pub fn new() -> Self {
        Self {
            state: PauseMenuState::Closed,
            selected_index: 0,
            removal_triggered: false,
        }
    }

    /// Open the pause menu.
    pub fn open(&mut self) {
        self.state = PauseMenuState::Open;
    }

    /// Close the pause menu.
    pub fn close(&mut self) {
        self.state = PauseMenuState::Closed;
        self.removal_triggered = false;
    }

    /// Get current menu state.
    pub fn state(&self) -> PauseMenuState {
        self.state
    }

    /// Get the currently selected menu option.
    pub fn selected_option(&self) -> PauseMenuOption {
        PauseMenuOption::ALL[self.selected_index % PauseMenuOption::ALL.len()]
    }

    /// Navigate up in the menu (wrap around to bottom).
    pub fn navigate_up(&mut self) {
        if self.selected_index == 0 {
            self.selected_index = PauseMenuOption::ALL.len() - 1;
        } else {
            self.selected_index -= 1;
        }
    }

    /// Navigate down in the menu (wrap around to top).
    pub fn navigate_down(&mut self) {
        self.selected_index = (self.selected_index + 1) % PauseMenuOption::ALL.len();
    }

    /// Confirm the currently selected option.
    /// Returns the action to perform.
    pub fn confirm_selection(&mut self) -> PauseMenuAction {
        match self.selected_option() {
            PauseMenuOption::Resume => PauseMenuAction::Resume,
            PauseMenuOption::RemoveOtherPlayer => {
                self.removal_triggered = true;
                PauseMenuAction::RemoveOtherPlayer
            }
            PauseMenuOption::QuitToMenu => PauseMenuAction::QuitToMenu,
            PauseMenuOption::Settings => PauseMenuAction::OpenSettings,
        }
    }

    /// Check if this player has requested removal of the other player.
    pub fn removal_triggered(&self) -> bool {
        self.removal_triggered
    }

    /// Reset removal trigger flag.
    pub fn reset_removal_trigger(&mut self) {
        self.removal_triggered = false;
    }

    /// Get the selected option index (0-3).
    pub fn selected_index(&self) -> usize {
        self.selected_index
    }

    /// Set selected option by index.
    pub fn set_selected_index(&mut self, index: usize) {
        self.selected_index = index % PauseMenuOption::ALL.len();
    }
}

impl Default for PauseMenuController {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for PauseMenuController {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PauseMenuController")
            .field("state", &self.state)
            .field("selected_option", &self.selected_option())
            .field("removal_triggered", &self.removal_triggered)
            .finish()
    }
}

/// Actions resulting from pause menu selection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PauseMenuAction {
    /// No action taken.
    None,
    /// Resume gameplay.
    Resume,
    /// Remove the other player from the session.
    RemoveOtherPlayer,
    /// Quit to main menu (both players).
    QuitToMenu,
    /// Open settings submenu.
    OpenSettings,
}

/// Manages pause menus for both players in split-screen.
pub struct TwoPlayerPauseMenuManager {
    /// Player 1's pause menu.
    player1: PauseMenuController,
    /// Player 2's pause menu.
    player2: PauseMenuController,
    /// Whether gameplay is paused (either player opened menu).
    is_paused: bool,
    /// If set, which player initiated the pause.
    paused_by: Option<u32>,
}

impl TwoPlayerPauseMenuManager {
    /// Create a new two-player pause menu manager.
    pub fn new() -> Self {
        Self {
            player1: PauseMenuController::new(),
            player2: PauseMenuController::new(),
            is_paused: false,
            paused_by: None,
        }
    }

    /// Player opens their pause menu.
    pub fn open_menu(&mut self, player_index: u32) {
        self.is_paused = true;
        self.paused_by = Some(player_index);

        match player_index {
            0 => self.player1.open(),
            1 => self.player2.open(),
            _ => {}
        }
    }

    /// Player closes their pause menu (e.g., presses Back/Cancel).
    pub fn close_menu(&mut self, player_index: u32) {
        match player_index {
            0 => self.player1.close(),
            1 => self.player2.close(),
            _ => {}
        }

        // Unpause only if both players have closed their menus
        if self.player1.state() == PauseMenuState::Closed && 
           self.player2.state() == PauseMenuState::Closed {
            self.is_paused = false;
            self.paused_by = None;
        }
    }

    /// Get player 1's pause menu controller.
    pub fn player1_menu(&mut self) -> &mut PauseMenuController {
        &mut self.player1
    }

    /// Get player 2's pause menu controller.
    pub fn player2_menu(&mut self) -> &mut PauseMenuController {
        &mut self.player2
    }

    /// Get read-only access to player 1's menu.
    pub fn player1_menu_ref(&self) -> &PauseMenuController {
        &self.player1
    }

    /// Get read-only access to player 2's menu.
    pub fn player2_menu_ref(&self) -> &PauseMenuController {
        &self.player2
    }

    /// Check if gameplay is currently paused.
    pub fn is_paused(&self) -> bool {
        self.is_paused
    }

    /// Get which player paused the game (if any).
    pub fn paused_by(&self) -> Option<u32> {
        self.paused_by
    }

    /// Player confirms their menu selection.
    pub fn player_confirm(&mut self, player_index: u32) -> PauseMenuAction {
        match player_index {
            0 => self.player1.confirm_selection(),
            1 => self.player2.confirm_selection(),
            _ => PauseMenuAction::None,
        }
    }

    /// Check if either player has requested removal of the other.
    pub fn any_removal_triggered(&self) -> bool {
        self.player1.removal_triggered() || self.player2.removal_triggered()
    }

    /// Get which player(s) have triggered removal (if any).
    pub fn removal_triggered_by(&self) -> Vec<u32> {
        let mut result = Vec::new();
        if self.player1.removal_triggered() {
            result.push(0);
        }
        if self.player2.removal_triggered() {
            result.push(1);
        }
        result
    }

    /// Reset both menus to default state.
    pub fn reset(&mut self) {
        self.player1 = PauseMenuController::new();
        self.player2 = PauseMenuController::new();
        self.is_paused = false;
        self.paused_by = None;
    }
}

impl Default for TwoPlayerPauseMenuManager {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for TwoPlayerPauseMenuManager {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TwoPlayerPauseMenuManager")
            .field("player1", &self.player1)
            .field("player2", &self.player2)
            .field("is_paused", &self.is_paused)
            .field("paused_by", &self.paused_by)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pause_menu_navigation() {
        let mut menu = PauseMenuController::new();
        menu.open();

        assert_eq!(menu.selected_option(), PauseMenuOption::Resume);
        menu.navigate_down();
        assert_eq!(menu.selected_option(), PauseMenuOption::RemoveOtherPlayer);
        menu.navigate_down();
        assert_eq!(menu.selected_option(), PauseMenuOption::QuitToMenu);
        menu.navigate_up();
        assert_eq!(menu.selected_option(), PauseMenuOption::RemoveOtherPlayer);
    }

    #[test]
    fn pause_menu_wrap_around() {
        let mut menu = PauseMenuController::new();
        menu.set_selected_index(3); // Last option
        menu.navigate_down();
        assert_eq!(menu.selected_option(), PauseMenuOption::Resume); // Wraps to first

        menu.navigate_up();
        assert_eq!(menu.selected_option(), PauseMenuOption::Settings); // Wraps to last
    }

    #[test]
    fn pause_menu_confirm_resume() {
        let mut menu = PauseMenuController::new();
        menu.open();
        let action = menu.confirm_selection();
        assert_eq!(action, PauseMenuAction::Resume);
    }

    #[test]
    fn pause_menu_confirm_removal() {
        let mut menu = PauseMenuController::new();
        menu.open();
        menu.set_selected_index(1); // RemoveOtherPlayer
        let action = menu.confirm_selection();
        assert_eq!(action, PauseMenuAction::RemoveOtherPlayer);
        assert!(menu.removal_triggered());
    }

    #[test]
    fn two_player_pause_manager() {
        let mut manager = TwoPlayerPauseMenuManager::new();
        assert!(!manager.is_paused());

        manager.open_menu(0);
        assert!(manager.is_paused());
        assert_eq!(manager.paused_by(), Some(0));

        manager.close_menu(0);
        assert!(!manager.is_paused());
    }

    #[test]
    fn two_player_pause_both_open() {
        let mut manager = TwoPlayerPauseMenuManager::new();

        manager.open_menu(0);
        assert!(manager.is_paused());
        manager.open_menu(1);
        assert!(manager.is_paused());

        manager.close_menu(0);
        assert!(manager.is_paused()); // Still open (player 2 still has menu open)

        manager.close_menu(1);
        assert!(!manager.is_paused());
    }

    #[test]
    fn two_player_removal_triggered() {
        let mut manager = TwoPlayerPauseMenuManager::new();
        manager.open_menu(0);
        manager.player1_menu().set_selected_index(1);
        manager.player_confirm(0);

        assert!(manager.any_removal_triggered());
        let triggered = manager.removal_triggered_by();
        assert_eq!(triggered, vec![0]);
    }
}
