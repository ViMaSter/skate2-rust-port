# Skate 2 Multiplayer Co-Op Integration Roadmap

**Status**: ~50% Complete - Foundation solid, core integration underway

## Completed Components ✅

### Phase 1: Input Router (COMPLETE)
- TwoPlayerInputRouter detects START button on controller slot 1
- Integrated into `LiveSkatingClient::run_tick()` every frame
- Proper edge detection (debouncing, press/release distinction)
- Binary: `dist/skate2-pc-multiplayer-p2-activation`

### Phase 2: Two-Player Session (COMPLETE)
- `TwoPlayerSession` added to `SelectedLiveSession` struct
- Initialized in constructor with `TwoPlayerState::Waiting`
- Methods: `activate_player2(char_id)`, `is_player2_active()`, `begin_remove_player2()`
- Getter methods: `two_player()` and `two_player_mut()`
- State transitions: Waiting → BothActive on START press

### Phase 2.5: Rendering Infrastructure (COMPLETE)
- `DualCameraManager` added to `LiveSkatingView` struct
- `SplitScreenViewportManager` initialized with 1920x1080, LeftRight layout
- Imports and types properly wired
- Binary: `dist/skate2-pc-multiplayer-cameras-ready`

## Critical Blocking Tasks 🔴

### 1. Player 2 Character Instantiation (CRITICAL)
**Status**: Research ongoing by agent e82e67bd
**Blocking**: Can't proceed without character in world
**Approach**: 
- Study `create_party_players()` method in `selected_session_party_play.rs`
- Duplicate player 1's ActorConstruction for player 2
- Spawn at same or nearby location
- Assign to separate actor slot/input channel

**Expected outcome**: Player 2 character visible in game world
**Integration point**: `LiveSkatingClient::run_tick()` when `consume_join_request()` returns true

---

### 2. Camera Switching & Split-Screen Rendering
**Status**: Research complete (agent 09f9d0ec)
**Research doc**: `SPLIT_SCREEN_RENDERING.md` (626 lines)

**Key findings**:
- Current flow: `LiveSkatingClient::frame()` → `SelectedVisualSnapshot` → camera → `LiveSkatingView::draw()`
- Need: Extend `SelectedVisualSnapshot` with `player2_camera_*` fields
- Integration point: `LiveSkatingView::draw()` - insert dual-pass rendering logic
- Two passes: Camera P1 → viewport_left, Camera P2 → viewport_right

**Blockers identified**:
1. Player 2 camera data must come from simulation (need `SelectedVisualSnapshot` extension)
2. Shadow maps - MVP: reuse single global shadow map for both cameras
3. Post-FX - MVP: apply global, no per-viewport effects
4. HUD - MVP: single shared UI interface (not per-player)
5. Frames-between-ticks - currently only has P1 camera in `PresentationRestorer`

**Implementation phases**:
- Phase 1 (1-2h): Extend `SelectedVisualSnapshot` with P2 camera data
- Phase 2 (1-2h): Add matrix conversion to `PlayerCamera`
- Phase 3 (2-3h): Implement `SplitScreenRenderPass` in `LiveSkatingView`
- Phase 4 (3-4h): GPU rendering (two passes with scissor rects)
- Phase 5 (2-3h): Testing

---

### 3. Two-Player Pause Menu Integration
**Status**: Research complete (agent 3482fd9d)
**Research doc**: `TWO_PLAYER_PAUSE.md` (703 lines)

**Key findings**:
- Current: Single `PauseMenuController` in `SelectedLiveSession`
- Need: `TwoPlayerPauseMenuManager` with independent menus for P1 and P2
- Unpause only when BOTH players close menus (cooperative pause)

**Critical API methods**:
- `player1_menu()` and `player2_menu()` - get menu for each player
- `try_input(player_index)` - route button press to correct menu
- `is_paused()` - check if either menu is open
- `removal_triggered_by()` - detect RemoveOtherPlayer selection

**Integration points**:
1. Add `two_player_pause: Option<TwoPlayerPauseMenuManager>` to `SelectedLiveSession`
2. Activate when `TwoPlayerState::BothActive` detected
3. Route controller input: P1 START→menu1, P2 START→menu2
4. Pause logic: either menu open = simulation paused

**Menu options per player**:
- Resume (unpause self)
- Remove Other Player
- Quit to Menu
- Settings

---

## Remaining Work by Priority

### Priority 1: Character Spawning (MUST DO FIRST)
Waiting for agent research to complete. Once available:
1. Find how `create_party_players()` works
2. Adapt for story mode (not party play)
3. Implement `activate_player2_character()` method
4. Call from `LiveSkatingClient::run_tick()` on join signal

**Estimated**: 1-2 hours

### Priority 2: Rendering Pipeline  
With character spawned, need cameras and viewports:
1. Extend `SelectedVisualSnapshot` with player2_camera data
2. Modify `LiveSkatingClient::frame()` to compute P2 camera
3. Update `LiveSkatingView::draw()` to do two-pass rendering
4. Test with two controllers

**Estimated**: 4-6 hours

### Priority 3: Input Routing
Route controller slot 1 to player 2:
1. Extract controller 1 input in `run_tick()`
2. Pass to player 2's input handling (similar to how P1 gets slot 0)
3. Test: P2 controller should move P2 skater independently

**Estimated**: 1-2 hours

### Priority 4: Pause Menu
Implement two-player pause:
1. Add pause menu manager to session
2. Activate on first player 2 join
3. Route input to correct menu per player
4. Implement cooperative pause logic

**Estimated**: 2-3 hours

### Priority 5: HUD/UI Adjustments
For split-screen display:
1. Identify HUD elements that need repositioning
2. Create left/right variants or hide non-essential elements
3. Test readability on split screen

**Estimated**: 1-2 hours

### Priority 6: End-to-End Testing
Comprehensive validation:
1. Build automated test harness (ControlsDemo extension)
2. Two-controller manual testing
3. Check objective completion logic (both players' states)
4. Validate save/load with co-op save data

**Estimated**: 2-3 hours

---

## Data Structures Ready to Use

### DualCameraManager (crates/skate2-core/src/camera/dual_camera_manager.rs)
```rust
pub struct PlayerCamera {
    position: [f32; 3],
    target: [f32; 3],
    up: [f32; 3],
    fov_degrees: f32,
    near_clip: f32,
    far_clip: f32,
}

pub struct DualCameraManager {
    player1: PlayerCamera,
    player2: PlayerCamera,
    smoothing: f32,
}

// Methods:
- update_player1_follow(skater_pos, offset, target_offset)
- update_player2_follow(skater_pos, offset, target_offset)
- smooth_update_player1(dt) / smooth_update_player2(dt)
- set_smoothing(factor)
```

### SplitScreenViewportManager (crates/skate2-core/src/rendering/split_screen_viewport.rs)
```rust
pub struct PlayerViewport {
    x: u32, y: u32, width: u32, height: u32,
}

pub enum SplitScreenLayout { LeftRight, TopBottom }

pub struct SplitScreenViewportManager { /* ... */ }

// Methods:
- player1_viewport() -> PlayerViewport
- player2_viewport() -> PlayerViewport
- set_screen_dimensions(width, height)
```

### TwoPlayerSession (crates/skate2-core/src/session/two_player_session.rs)
```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TwoPlayerState { Waiting, BothActive, RemovingPlayer2 }

pub struct TwoPlayerSession {
    state: TwoPlayerState,
    objective: TwoPlayerObjective,
    player2_character_id: Option<u64>,
}

// Methods:
- new_waiting() -> Self
- activate_player2(character_id)
- is_player2_active() -> bool
- objective methods for tracking completion
```

---

## Files Modified So Far

### Core Multiplayer Integration
- `crates/skate2-core/src/session/selected_session_public_facade.rs`
  - Added `two_player: TwoPlayerSession` field
  - Added `two_player()` and `two_player_mut()` getters
  - Initialized in `new()`

### Game Loop Integration
- `apps/skate2-pc/src/live_skating_client.rs`
  - Added `TwoPlayerInputRouter` field
  - Integrated router detection in `run_tick()`
  - Call `session.two_player_mut().activate_player2()` on join signal
  - Added `PadPlatform` import

### Rendering Infrastructure
- `apps/skate2-pc/src/live_skating_view.rs`
  - Added `DualCameraManager` field
  - Added `SplitScreenViewportManager` field
  - Added proper imports
  - Initialized with 1920x1080 LeftRight layout

### Input Router (Previous Session)
- `crates/skate2-core/src/input/two_player_input_router.rs`
  - Fixed button detection (0x0010 START bit)
  - Added edge detection
  - Proper debouncing (3-frame window)

---

## Testing Strategy

### Unit Tests (Per Component)
- TwoPlayerSession state transitions
- DualCameraManager matrix calculations
- SplitScreenViewportManager rectangle calculations
- TwoPlayerPauseMenu menu state

### Integration Tests
- Character spawn on P2 join
- Camera updates with P2 position
- Viewport calculation matches screen size
- Pause menu opens/closes correctly

### Manual Tests (Two Controllers)
1. Start game, load challenge
2. Press START on controller 2
   - Console: "Player 2 activated"
   - Visual: Two players visible
3. Move both sticks independently
   - P1 responds to controller 1
   - P2 responds to controller 2
4. Press START on P1 → pause menu
5. Press START on P2 → separate pause menu
6. Verify objectives track per-player
7. Complete challenge
8. Test "Remove Other Player" option

---

## Architecture Summary

```
Input Layer:
  Controller Slot 0 (P1) ──────┐
  Controller Slot 1 (P2) ──→ Router ──┐
                                       │
                                       ↓
                            TwoPlayerSession
                            (state machine)
                                       │
                    ┌──────────────────┼──────────────────┐
                    ↓                  ↓                  ↓
            Character Spawning     Camera Management   Pause Menu
                    │                  │                  │
                    ├─→ ActorPool ──┐  ├─→ Matrix math   ├─→ Menu 1
                    │               │  │                 └─→ Menu 2
                    ├─→ Input 0 ────┤  └─→ Viewport calc
                    └─→ Input 1 ────┤
                                    ↓
                            Render two cameras
                            (split-screen)
                                    ↓
                            Output two viewports
```

---

## Known Issues & Workarounds

1. **Character ID Selection**: Currently using placeholder ID 1 for both P1 and P2
   - Workaround: Duplicate P1's character until character selection implemented
   - Future: Add character selection before co-op session starts

2. **Shadow Maps**: Only one global shadow map
   - MVP Approach: Share single shadow map (shadows from P1 camera only)
   - Better: Compute separate shadow maps per camera (future optimization)

3. **HUD on Split-Screen**: UI designed for single full screen
   - MVP: Single unified HUD visible on both halves
   - Future: Per-player HUD variants or minimized layouts

4. **Frames-Between-Ticks**: Animation smooth frames only have P1 camera
   - MVP: Disable interpolation when P2 active
   - Future: Extend PresentationRestorer to support dual cameras

---

## Quick Reference

**Binary to test**: `dist/skate2-pc-multiplayer-cameras-ready`
**Build command**: `~/.cargo/bin/cargo build --release --bin skate2-pc`
**Run game**: `./dist/skate2-pc-multiplayer-cameras-ready`

**Research documents**:
- `SPLIT_SCREEN_RENDERING.md` - Complete rendering architecture
- `TWO_PLAYER_PAUSE.md` - Pause menu integration guide
- [Awaiting character spawning doc from agent]

**Key files to understand**:
- `crates/skate2-core/src/session/selected_session_party_play.rs` - Multi-player reference
- `crates/skate2-core/src/session/two_player_session.rs` - Session logic
- `apps/skate2-pc/src/live_skating_client.rs` - Main game loop

