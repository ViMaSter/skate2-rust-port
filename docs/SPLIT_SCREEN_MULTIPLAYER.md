# Split-Screen Multiplayer Implementation Guide

## Overview

This document describes the split-screen co-op multiplayer feature for Skate 2 main story mode. Two players can play simultaneously in a shared world with independent cameras and viewports (left/right or top/bottom split).

## Architecture

### Core Components

1. **Two-Player Session Management** (`two_player_session.rs`)
   - Tracks two-player game state (waiting, both active, removing player 2)
   - Manages unified objective completion with per-player tracking
   - Handles objective types: standard (one player passes = all pass), timed activities
   - Failure conditions: both players must fail to fail
   - Player 2 lifecycle (join/remove)

2. **Input Router** (`two_player_input_router.rs`)
   - Detects player 2 START button press for join signal
   - Tracks controller connection state
   - Debounces START press (prevents repeated joins)
   - Routes inputs to correct player

3. **Viewport Management** (`split_screen_viewport.rs`)
   - Manages 50/50 split (left/right or top/bottom)
   - Tracks viewport dimensions for each player
   - Supports screen resize and layout switching
   - Provides point-in-viewport testing for input

4. **Dual Camera System** (`dual_camera_manager.rs`)
   - Independent cameras for each player
   - Smooth follow logic with adjustable smoothing
   - Forward vector and LookAt computation
   - Camera position/target tracking

5. **Pause Menu System** (`two_player_pause_menu.rs`)
   - Separate pause menus per player
   - Independent navigation (up/down/confirm)
   - Options: Resume, Remove Other Player, Quit, Settings
   - Either player can remove the other
   - Paused state: both players must close menus to unpause

### Objective Logic

**Standard Challenges:**
- Player 1 completes objective → Challenge passes for both
- Single player can achieve, single player can fail

**Timed Activities:**
- Timer ticks down regardless of player progress
- At timeout: check if `require_both_on_timeout` is set
  - If true: both must complete, otherwise fail
  - If false: standard logic (one completes = pass)

**Failure Conditions:**
- Only fail if **both** players trigger a failure condition
- Example: distance to NPC too far fails only if both are too far
- Single player can complete while other is in danger

## Integration Points

### 1. Session Initialization

When starting main story mode with potential two-player:

```rust
// Create session (player 1 only initially)
let mut two_player_session = TwoPlayerSession::new_waiting();

// Session has:
// - state: TwoPlayerState::Waiting
// - objective: TwoPlayerObjective::new_standard()
// - player2_character_id: None
```

### 2. Input Detection

Each frame, check for player 2 join signal:

```rust
router.advance_frame();
let player2_input = get_controller_2_input(); // 24-element f32 array
let check = router.check_player2_input(&player2_input);

if router.consume_join_request() {
    // Player 2 pressed START → trigger join sequence
    load_player2_character(&mut session);
    session.activate_player2(character_id);
}
```

### 3. Viewport & Camera Setup

Initialize split-screen rendering:

```rust
let screen = ScreenDimensions::new(1920, 1080);
let viewport_manager = SplitScreenViewportManager::new(
    screen,
    SplitScreenLayout::LeftRight,
);

let mut camera_manager = DualCameraManager::new();
camera_manager.set_smoothing(0.6);
```

### 4. Pause Menu Integration

Handle player pause input:

```rust
if player1_pressed_start {
    pause_manager.open_menu(0);
}

if player1_in_menu {
    if player1_pressed_up {
        pause_manager.player1_menu().navigate_up();
    }
    if player1_pressed_confirm {
        let action = pause_manager.player_confirm(0);
        match action {
            PauseMenuAction::RemoveOtherPlayer => {
                // Remove player 2
            }
            // ... handle other actions
        }
    }
}
```

### 5. Objective Update Loop

Each frame, update objectives:

```rust
// Update player 1 state
if player1_completed_objective {
    let completion = session.objective.mark_player_completed(0);
    match completion {
        ObjectiveCompletionState::Passed => challenge_complete(),
        _ => {}
    }
}

// Update player 2 if active
if session.is_player2_active() && player2_completed_objective {
    let completion = session.objective.mark_player_completed(1);
    match completion {
        ObjectiveCompletionState::Passed => challenge_complete(),
        _ => {}
    }
}

// For timed activities, advance time
if let Some(remaining) = session.objective.remaining_time() {
    let completion = session.objective.advance_time(dt);
    match completion {
        ObjectiveCompletionState::Passed => challenge_complete(),
        ObjectiveCompletionState::Failed => challenge_failed(),
        _ => {}
    }
}
```

## Data Flow

### Player 2 Join Sequence

```
User presses START on controller 2
    ↓
TwoPlayerInputRouter detects START
    ↓
consume_join_request() returns true
    ↓
Load player 2 character (async or sync)
    ↓
session.activate_player2(char_id)
    ↓
Transition to split-screen rendering
    ↓
Both cameras active
```

### Objective Completion

```
Player 1 completes goal
    ↓
objective.mark_player_completed(0)
    ↓
update_completion() checks states
    ↓
player1.player_completed = true
    ↓
One-player-pass rule → PASSED
    ↓
Challenge complete, resume/quit
```

### Timed Activity Timeout

```
Activity start time = now()
duration = 60.0 seconds
    ↓
Each frame: objective.advance_time(dt)
elapsed_time += dt
    ↓
At 60.0 seconds:
    if require_both_on_timeout:
        both_completed? → PASSED : FAILED
    else:
        any_completed? → PASSED : FAILED
    ↓
Challenge completes
```

## Implementation Checklist

- [x] **Foundation**: TwoPlayerSession, TwoPlayerInputRouter
- [x] **Rendering**: SplitScreenViewportManager, DualCameraManager
- [x] **UI**: TwoPlayerPauseMenuManager
- [ ] **Integration**: Connect to freeskate session
- [ ] **Career**: Update challenge system to track 2-player completion
- [ ] **Input routing**: Map controller 1/2 to player 0/1
- [ ] **Camera follow**: Implement per-player camera tracking
- [ ] **HUD adjustment**: Scale/reposition for split viewport
- [ ] **Testing**: Validate all objective types, edge cases
- [ ] **Performance**: Profile split-screen rendering

## Testing Approach

### Unit Tests
- Two-player objective logic (pass/fail conditions)
- Input router debouncing and state transitions
- Pause menu navigation and selection
- Viewport coordinate calculations
- Camera follow logic

### Integration Tests
- Player 2 join while player 1 is active
- Pause menu while playing
- Objective completion recognition
- Player 2 removal mid-game
- Career progression save

### Manual Tests (with real controllers)
- Press START on controller 2 mid-gameplay
- Both players navigate menus independently
- Remove player 2 from either pause menu
- Complete objectives with both configurations
- Timed activities with both players
- Player 1 only (player 2 not joined)

## Known Limitations

1. **Rendering**: Split-screen rendering not yet integrated with GPU renderer
2. **Physics**: Two skaters in same world may have interaction issues (investigate)
3. **UI Space**: Limited HUD space per player in split screen (design needed)
4. **Career**: Progression handling for 2-player completion (design needed)
5. **Network**: No online co-op support (local only, as intended)

## Future Enhancements

- 3-4 player split-screen (2x2 grid)
- Vertical and diagonal split options
- Per-player HUD customization
- Asymmetric objectives (cooperative but different goals)
- Spectator mode for waiting players

## References

- Original Party Play implementation: `docs/partyplay/README.md`
- Session architecture: `docs/CORE_LAYOUT.md`
- Input system: `docs/ARCHITECTURE.md`
