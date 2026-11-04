# Two-Player Pause Menu Integration

## Overview

This document describes how to integrate the two-player independent pause menu system (`TwoPlayerPauseMenuManager`) into the Skate 2 game loop. The goal is to enable both players to pause independently with their own menus, where either player can pause and see their own menu options without affecting the other player's view.

---

## Current Single-Player Pause Menu Flow

### How Single-Player Pause Menu Is Triggered

**File:** `crates/skate2-core/src/ui/pause_menu.rs`

1. **State Container:** `SelectedLiveSession<T, C>` owns a single `PauseMenuController`
   - Located in: `crates/skate2-core/src/session/selected_session_public_facade.rs:500`
   - Field: `pause_menu: crate::ui::pause_menu::PauseMenuController`

2. **Input Capture:**
   - Each frame, the session calls `input.try_pause_menu_input::<P>()` 
   - Location: `crates/skate2-core/src/session/selected_production_input_owner.rs`
   - This extracts the frontend (pause menu) input from the controller sample, including:
     - `start` button press (0x0010 bit in XboxPadSample.buttons)
     - Navigation: `up`, `down`, `left`, `right`
     - Confirmation: `select` (A button), `cancel` (B button)

3. **Processing:**
   - In `preupdate_input()` phase (line 2086 of selected_session_public_facade.rs):
     ```rust
     let frontend_input = input.try_pause_menu_input::<P>()?;
     ```
   - Later in the frame, the session updates the menu state (line 2747):
     ```rust
     let pause = self.pause_menu.update(frontend_input, policy.has_focus)?;
     ```

4. **Pause State Check:**
   - The pause menu's `visible` state (`.snapshot().visible`) determines if gameplay is paused
   - When visible, the frontend consumes input and game simulation is paused
   - Any change to pause state is reflected in `PauseMenuSnapshot`

### Key Pause Menu API (Single Player)

**`PauseMenuController`** methods:
- `update(input: PauseMenuInput, focused: bool) -> PauseMenuSnapshot`
  - Processes input and returns new menu state
- `snapshot() -> PauseMenuSnapshot`
  - Read-only state: `visible`, `popup`, `screen_state`, etc.
- `close()`
  - Closes the menu and unpauses gameplay
- `reenter()`
  - Re-opens menu (e.g., after returning from a submenu)
- `selected_option() -> PauseMenuOption`
  - Returns currently highlighted menu item
- `take_quit_confirmed() -> bool`
  - Query if player selected Quit (one-shot)
- `awaiting_intro() -> bool`
  - Menu is transitioning in/out (input not yet active)

### Important States

**Pause Menu Options (single player):**
```rust
pub enum PauseMenuOption {
    Replay = 0,
    Map = 1,
    Blog = 2,
    Profile = 3,
    EditSkater = 4,
    TrickGuide = 5,
    Settings = 6,
    Quit = 7,
    // ... (challenge-specific options)
}
```

**Menu Visibility:**
- `visible` = true → gameplay is paused, menu is on screen
- `visible` = false → normal gameplay, menu is hidden

**Pause State Tracking in Session:**
- Only checked via `self.pause_menu.snapshot().visible`
- When pause menu transitions from closed to open, `input.try_set_frontend_pause(true)` is called
- When pause menu closes, `input.try_set_frontend_pause(false)` is called

---

## Two-Player Pause Menu Architecture

### TwoPlayerPauseMenuManager API

**File:** `crates/skate2-core/src/ui/two_player_pause_menu.rs`

The `TwoPlayerPauseMenuManager` provides:

#### Core Data Structures

```rust
/// Menu state for one player
pub enum PauseMenuState {
    Closed,        // Menu not open
    Open,          // Menu is active
    Transitioning, // Fade in/out
}

/// Menu options for split-screen two-player
pub enum PauseMenuOption {
    Resume,              // Resume gameplay
    RemoveOtherPlayer,   // Remove player 2 (or player 1)
    QuitToMenu,          // Quit to main menu (both players)
    Settings,            // Settings submenu
}

/// Actions resulting from menu selection
pub enum PauseMenuAction {
    None,
    Resume,
    RemoveOtherPlayer,
    QuitToMenu,
    OpenSettings,
}
```

#### Per-Player Controller

```rust
pub struct PauseMenuController {
    state: PauseMenuState,           // Open/Closed/Transitioning
    selected_index: usize,           // Index in menu options
    removal_triggered: bool,         // Whether player selected "Remove Other"
}

impl PauseMenuController {
    pub fn new() -> Self
    pub fn open(&mut self)
    pub fn close(&mut self)
    pub fn state(&self) -> PauseMenuState
    pub fn selected_option(&self) -> PauseMenuOption
    pub fn navigate_up(&mut self)
    pub fn navigate_down(&mut self)
    pub fn confirm_selection(&mut self) -> PauseMenuAction
    pub fn removal_triggered(&self) -> bool
    pub fn reset_removal_trigger(&mut self)
    pub fn selected_index(&self) -> usize
    pub fn set_selected_index(&mut self, index: usize)
}
```

#### Manager (Two Players)

```rust
pub struct TwoPlayerPauseMenuManager {
    player1: PauseMenuController,      // Independent menu for P1
    player2: PauseMenuController,      // Independent menu for P2
    is_paused: bool,                   // Gameplay paused if either menu open
    paused_by: Option<u32>,            // Which player initiated pause (0 or 1)
}

impl TwoPlayerPauseMenuManager {
    pub fn new() -> Self
    
    // Menu control
    pub fn open_menu(&mut self, player_index: u32)
    pub fn close_menu(&mut self, player_index: u32)
    
    // Access player menus
    pub fn player1_menu(&mut self) -> &mut PauseMenuController
    pub fn player2_menu(&mut self) -> &mut PauseMenuController
    pub fn player1_menu_ref(&self) -> &PauseMenuController
    pub fn player2_menu_ref(&self) -> &PauseMenuController
    
    // Pause state queries
    pub fn is_paused(&self) -> bool  // True if either player has menu open
    pub fn paused_by(&self) -> Option<u32>
    
    // Input handling
    pub fn player_confirm(&mut self, player_index: u32) -> PauseMenuAction
    
    // Removal tracking
    pub fn any_removal_triggered(&self) -> bool
    pub fn removal_triggered_by(&self) -> Vec<u32>
    pub fn reset_removal_trigger(&mut self)
    
    pub fn reset(&mut self)
}
```

### Key Behavioral Differences from Single-Player

1. **Dual Menu State:** Each player has independent `PauseMenuState`
   - Player 1 can have menu open while Player 2's is closed (and vice versa)
   - Menu appearance on screen must be split (e.g., P1 menu left side, P2 menu right side)

2. **Unpause Condition:** Gameplay unpauses **only when both players have closed their menus**
   - See `close_menu()` implementation:
     ```rust
     // Unpause only if both players have closed their menus
     if self.player1.state() == PauseMenuState::Closed && 
        self.player2.state() == PauseMenuState::Closed {
         self.is_paused = false;
         self.paused_by = None;
     }
     ```

3. **Removal Signal:** Either player can remove the other
   - `any_removal_triggered()` → true if P1 or P2 (or both) selected "Remove Other Player"
   - `removal_triggered_by()` → Vec of player indices who triggered removal

---

## Integration Points

### 1. Where to Instantiate Two-Player Pause Manager

**File:** `crates/skate2-core/src/session/selected_session_public_facade.rs`

**Current single-player structure (lines ~494–661):**
```rust
pub struct SelectedLiveSession<T, C> {
    // ... many other fields ...
    pause_menu: crate::ui::pause_menu::PauseMenuController,  // Single player only
    two_player: crate::two_player_session::TwoPlayerSession,
}
```

**Proposed change:**
- Add a new field to hold the two-player pause manager:
  ```rust
  pub struct SelectedLiveSession<T, C> {
      // ... existing fields ...
      pause_menu: crate::ui::pause_menu::PauseMenuController,  // Keep for single-player
      two_player_pause_menu: Option<crate::ui::two_player_pause_menu::TwoPlayerPauseMenuManager>,
      two_player: crate::two_player_session::TwoPlayerSession,
  }
  ```

**Initialization (lines ~661):**
```rust
Self {
    // ... other fields ...
    pause_menu: crate::ui::pause_menu::PauseMenuController::default(),
    two_player_pause_menu: None,  // Activated when player 2 joins
    two_player: crate::two_player_session::TwoPlayerSession::new_waiting(),
}
```

### 2. When to Activate Two-Player Pause Manager

**Detection Point:** In the game update loop, when `TwoPlayerState` transitions from `Waiting` to `BothActive`

**File:** `crates/skate2-core/src/session/selected_session_public_facade.rs` (preupdate or appropriate update phase)

**Logic:**
```rust
// Check if player 2 just joined
if self.two_player.state() == TwoPlayerState::BothActive 
    && self.two_player_pause_menu.is_none() {
    // Initialize two-player pause manager
    self.two_player_pause_menu = Some(TwoPlayerPauseMenuManager::new());
}
```

### 3. How to Route Controller Input to Correct Menu

**Input Routing Strategy:**

Player input is already routed by controller index. The key is to extract **separate input samples for each player** and feed them to the correct menu.

**Current single-player input (lines ~2086 of selected_session_public_facade.rs):**
```rust
let frontend_input = input.try_pause_menu_input::<P>()?;
// This only gets Player 1's input
```

**Two-Player Input Routing (proposed):**
1. Extract Player 1's frontend input:
   ```rust
   let p1_frontend_input = input.try_pause_menu_input::<P>()?;  // P1 controller
   ```

2. Extract Player 2's frontend input:
   ```rust
   // Need to modify try_pause_menu_input to accept a player_index parameter
   // OR create a parallel path for player 2
   let p2_frontend_input = input.try_pause_menu_input_for_player::<P>(1)?;  // P2 controller
   ```

3. Route to correct pause menu:
   ```rust
   if let Some(ref mut two_player_pause) = self.two_player_pause_menu {
       // Feed P1 input to P1 menu
       if p1_frontend_input.start {
           two_player_pause.open_menu(0);
       }
       
       // Navigate P1 menu
       let p1_menu = two_player_pause.player1_menu();
       if p1_frontend_input.up { p1_menu.navigate_up(); }
       if p1_frontend_input.down { p1_menu.navigate_down(); }
       
       // Confirm P1 selection
       if p1_frontend_input.select {
           let action = two_player_pause.player_confirm(0);
           // Handle action...
       }
       
       // Same for Player 2:
       if p2_frontend_input.start {
           two_player_pause.open_menu(1);
       }
       
       let p2_menu = two_player_pause.player2_menu();
       if p2_frontend_input.up { p2_menu.navigate_up(); }
       if p2_frontend_input.down { p2_menu.navigate_down(); }
       
       if p2_frontend_input.select {
           let action = two_player_pause.player_confirm(1);
           // Handle action...
       }
   }
   ```

### 4. Pause State Management

**Key Challenge:** When two-player mode is active, defer to `TwoPlayerPauseMenuManager` for pause state instead of single-player `PauseMenuController`.

**Current pause check (lines ~2716, 2737–2758):**
```rust
let was_paused = self.pause_menu.snapshot().visible;

// ... later in frame ...

let frontend_input = if self.pause_menu.awaiting_intro() || career_overlay_open {
    crate::ui::pause_menu::PauseMenuInput::default()
} else {
    frontend_input
};

let pause = self.pause_menu.update(frontend_input, /* ... */);

// Check selected options
if was_paused && self.pause_menu.selected_option() == PauseMenuOption::Map {
    // Handle Map selection
}
```

**Proposed logic (two-player):**
```rust
let is_paused = if let Some(ref two_player_pause) = self.two_player_pause_menu {
    two_player_pause.is_paused()  // Both menus must be closed to unpause
} else {
    self.pause_menu.snapshot().visible
};

// Set frontend pause state
if is_paused != was_paused {
    if is_paused {
        input.try_set_frontend_pause(true);
    } else {
        input.try_set_frontend_pause(false);
    }
}
```

### 5. How to Determine When to Unpause

**Unpause condition in TwoPlayerPauseMenuManager:**
```rust
pub fn close_menu(&mut self, player_index: u32) {
    match player_index {
        0 => self.player1.close(),
        1 => self.player2.close(),
        _ => {}
    }
    
    // Unpause only if BOTH players have closed their menus
    if self.player1.state() == PauseMenuState::Closed && 
       self.player2.state() == PauseMenuState::Closed {
        self.is_paused = false;
        self.paused_by = None;
    }
}
```

**Integration point in session update:**
1. After processing Player 2's menu close input:
   ```rust
   if p2_frontend_input.cancel {  // B button to close menu
       two_player_pause.close_menu(1);
   }
   ```

2. Check unpause condition:
   ```rust
   if !two_player_pause.is_paused() {
       // Both players closed menus → unpause
       input.try_set_frontend_pause(false);
       // Continue with normal game simulation
   }
   ```

---

## Menu Options Handling

### Differences from Single-Player

**Single-player options:**
- Replay, Map, Blog, Profile, EditSkater, TrickGuide, Settings, Quit

**Two-player options (simplified):**
- **Resume:** Unpause (but only when both players select Resume or close menus)
- **RemoveOtherPlayer:** Allow a player to remove the other player mid-session
- **Settings:** Open settings submenu (rendered independently for each player)
- **QuitToMenu:** Quit to main menu (affects both players)

### Handling "RemoveOtherPlayer"

When a player selects "RemoveOtherPlayer":
1. `confirm_selection()` returns `PauseMenuAction::RemoveOtherPlayer`
2. `removal_triggered()` returns true for that player
3. Session should:
   - Check `two_player_pause.removal_triggered_by()` → Vec of player indices
   - Transition `TwoPlayerState::BothActive` → `TwoPlayerState::RemovingPlayer2`
   - Close the two-player pause menu
   - Revert to single-player game loop with remaining player

### Handling "QuitToMenu"

When a player selects "QuitToMenu":
1. Both players' menus should show a confirmation dialog
2. **Decision point:** Should both players need to confirm, or just one?
   - Suggested: One player can quit for the session (like disconnect)
3. Once confirmed:
   - Close two-player pause menu
   - Transition game state to quit/menu

### Handling "Resume"

1. Player selects "Resume" → closes their menu
2. If the other player's menu is also closed → unpause
3. If the other player still has menu open:
   - Gameplay remains paused
   - That player can continue navigating their menu

---

## Implementation Plan

### Phase 1: Refactor Input Routing

**Goal:** Enable extracting Player 2's frontend input independently

**Steps:**
1. Modify `selected_production_input_owner.rs`:
   - Add method `try_pause_menu_input_for_player(player_index: u32)` 
   - Or refactor existing method to accept player index parameter

2. Update input sample handling to map player controllers correctly:
   - Player 1 → controller slot 0
   - Player 2 → controller slot 1

### Phase 2: Integrate TwoPlayerPauseMenuManager

**Steps:**
1. Add `two_player_pause_menu` field to `SelectedLiveSession<T, C>`
2. Initialize on Player 2 join event
3. Instantiate in preupdate phase

### Phase 3: Route and Process Input

**Steps:**
1. Extract Player 1 and Player 2 frontend inputs
2. Feed to correct menu in `TwoPlayerPauseMenuManager`
3. Process navigation (up/down/left/right)
4. Process confirmations (select/cancel)

### Phase 4: Pause State and Simulation Control

**Steps:**
1. Check `two_player_pause.is_paused()` instead of single-player menu
2. Call `input.try_set_frontend_pause()` based on two-player pause state
3. Only unpause when both players' menus are closed

### Phase 5: Menu Actions

**Steps:**
1. Handle RemoveOtherPlayer selection
2. Handle QuitToMenu confirmation
3. Handle Resume (close menu → check if both closed)

### Phase 6: Rendering (Out of Scope for Integration)

**Steps:**
1. Render Player 1's menu on left side of screen
2. Render Player 2's menu on right side of screen
3. Handle simultaneous menu visibility

---

## Pseudo-Code Integration Example

```rust
// In SelectedLiveSession::preupdate_input() or similar phase

// Check if we should activate two-player pause
if self.two_player.state() == TwoPlayerState::BothActive 
    && self.two_player_pause_menu.is_none() {
    self.two_player_pause_menu = Some(TwoPlayerPauseMenuManager::new());
}

// Extract frontend input for both players
let p1_frontend = input.try_pause_menu_input::<P>()?;
let p2_frontend = if let Ok(p2_input) = input.try_pause_menu_input_for_player::<P>(1) {
    p2_input
} else {
    crate::ui::pause_menu::PauseMenuInput::default()
};

// Route to two-player pause manager if active
if let Some(ref mut two_player_pause) = self.two_player_pause_menu {
    
    // Player 1: Handle START press
    if p1_frontend.start && two_player_pause.player1_menu_ref().state() == PauseMenuState::Closed {
        two_player_pause.open_menu(0);
    }
    
    // Player 1: Navigation
    let p1_menu = two_player_pause.player1_menu();
    if p1_frontend.up { p1_menu.navigate_up(); }
    if p1_frontend.down { p1_menu.navigate_down(); }
    
    // Player 1: Confirm selection
    if p1_frontend.select {
        match two_player_pause.player_confirm(0) {
            PauseMenuAction::Resume => {
                two_player_pause.close_menu(0);
            }
            PauseMenuAction::RemoveOtherPlayer => {
                // Queue removal of player 2
                // Close menu, transition state
            }
            PauseMenuAction::QuitToMenu => {
                // Show confirmation, handle quit
            }
            PauseMenuAction::OpenSettings => {
                // Open settings submenu for P1
            }
            _ => {}
        }
    }
    
    // Player 1: Cancel (close menu)
    if p1_frontend.cancel {
        two_player_pause.close_menu(0);
    }
    
    // Repeat for Player 2 with p2_frontend...
    
    // Check if should unpause
    if !two_player_pause.is_paused() {
        input.try_set_frontend_pause(false);
    } else {
        input.try_set_frontend_pause(true);
    }
}
```

---

## Dependencies and Considerations

### Existing Code Structure

- **`TwoPlayerInputRouter`** (input/two_player_input_router.rs): Tracks P2 join signal
  - Use to detect when to initialize two-player pause manager
  - START button detection already present

- **`TwoPlayerSession`** (session/two_player_session.rs): Manages game state
  - Query `state()` to know if both players active
  - Necessary for determining pause mode

- **Input Subsystem** (session/selected_production_input_owner.rs):
  - Needs ability to extract Player 2's frontend input
  - Currently only extracts Player 1's pause input

### Challenges

1. **Input Extraction for Player 2:**
   - Current code assumes single player
   - Need to refactor to support dual input paths
   - Each player's controller slot must map correctly

2. **Rendering Split Menus:**
   - UI system must support side-by-side menu rendering
   - May require viewport splitting at render time

3. **Settings Submenu:**
   - Should settings changes affect just the player or both?
   - Suggested: Per-player settings (audio volume, control mapping)

4. **State Persistence:**
   - If Player 2 joins mid-session, do we restore their menu state?
   - Suggested: Always start fresh (Waiting → BothActive initializes new manager)

---

## Testing Strategy

### Unit Tests

**TwoPlayerPauseMenuManager:**
- Both players open menus independently
- Navigation in one menu doesn't affect the other
- Unpause condition (both menus closed)
- Removal trigger detection
- Action confirmation

**Example test:**
```rust
#[test]
fn both_players_navigate_independently() {
    let mut manager = TwoPlayerPauseMenuManager::new();
    
    manager.open_menu(0);
    manager.open_menu(1);
    
    manager.player1_menu().navigate_down();
    assert_eq!(manager.player1_menu_ref().selected_option(), 
               PauseMenuOption::RemoveOtherPlayer);
    assert_eq!(manager.player2_menu_ref().selected_option(), 
               PauseMenuOption::Resume);
}

#[test]
fn unpause_requires_both_closed() {
    let mut manager = TwoPlayerPauseMenuManager::new();
    
    manager.open_menu(0);
    manager.open_menu(1);
    assert!(manager.is_paused());
    
    manager.close_menu(0);
    assert!(manager.is_paused());  // P2 still open
    
    manager.close_menu(1);
    assert!(!manager.is_paused());  // Both closed
}
```

### Integration Tests

1. Player 1 pauses alone (two-player mode not active)
   - Single-player menu logic unchanged

2. Player 2 joins mid-session
   - Two-player pause manager activated
   - Player 1 can press START to see menu (both players now affected)

3. Both players pause simultaneously
   - Both menus visible
   - Each menu tracks its own selection

4. One player closes menu while other navigates
   - First player's menu closes
   - Gameplay remains paused (other menu still open)

5. RemoveOtherPlayer action
   - Transition to single-player, remaining player continues

6. Quit confirmation
   - Both players see confirmation dialog
   - Selection by one player triggers exit

---

## References

### Key Files

- `crates/skate2-core/src/ui/two_player_pause_menu.rs` - TwoPlayerPauseMenuManager API
- `crates/skate2-core/src/ui/pause_menu.rs` - Single-player pause menu (reference)
- `crates/skate2-core/src/session/selected_session_public_facade.rs` - Session integration point
- `crates/skate2-core/src/session/selected_production_input_owner.rs` - Input extraction
- `crates/skate2-core/src/session/two_player_session.rs` - Two-player state management
- `crates/skate2-core/src/input/two_player_input_router.rs` - Player 2 join detection

### Related Docs

- `MULTIPLAYER_BUILD_SUMMARY.md` - Overview of two-player work
- `DS4_MULTIPLAYER_FIX.md` - Controller input handling

---

## Next Steps

1. **Input System Refactor:** Enable dual frontend input paths
2. **Session Integration:** Add two_player_pause_menu field and initialization
3. **Input Routing:** Implement player-specific pause menu input handling
4. **Pause State Logic:** Switch between single and dual pause modes
5. **Action Handling:** Implement RemoveOtherPlayer, QuitToMenu, Resume
6. **UI Rendering:** Implement split-screen menu rendering
7. **Testing:** Unit and integration tests
