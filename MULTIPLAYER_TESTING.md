# Multiplayer Co-Op Testing Guide

## Current Status

**Build**: dist/skate2-pc-multiplayer-p2-activation (52 MB)
**Completion**: ~40% (Router + Session Foundation Complete)

## Quick Start Test

### Prerequisites
- Two controllers (DualShock 4, DualSense, Xbox, or similar)
- Controller 1: Connected and verified working
- Controller 2: Connected via Bluetooth or USB

### Test Procedure

1. **Start the game**:
   ```bash
   ./dist/skate2-pc-multiplayer-p2-activation
   ```

2. **Navigate to a skating challenge** (main story or free skate)

3. **Press START on Controller 2** while in the game
   - Watch console output
   - Expected: `"Skate 2: Player 2 START pressed - activating player 2!"`
   - Expected: `"Skate 2: Player 2 activated in session"`

4. **Current visible behavior**:
   - Game continues normally (single player visible)
   - Only controller 1 responds to input
   - [PENDING] Player 2 character spawn
   - [PENDING] Split-screen camera/viewport
   - [PENDING] Controller 2 input routing

## Console Output Diagnostics

### Successful Detection
```
Skate 2: Player 2 START pressed - activating player 2!
Skate 2: Player 2 activated in session
```

### What This Means
- ✅ Input router is running every frame
- ✅ START button detected on controller slot 1
- ✅ Edge detection working (no repeated signals)
- ✅ Session state transitions to `TwoPlayerState::BothActive`
- ✅ Character ID stored (1u64 placeholder)

## Next Phases

### Phase 3: Character Spawning
- Create duplicate player 2 character at spawn location
- P2 skater appears in game world
- P1 and P2 independent but same location initially

### Phase 4: Camera & Rendering
- DualCameraManager activated when `is_player2_active()`
- Split-screen viewport (left/right 50/50 split)
- Each player sees their own 1st-person camera view

### Phase 5: Input Routing
- Controller slot 1 input → Player 2 character
- Independent trick/movement inputs
- Both players active simultaneously

### Phase 6: Pause Menus
- P1 presses START again → Pause menu
- P2 can independently pause with their START
- Cooperative pause (both must close to resume)

### Phase 7: Objectives & Completion
- Shared objective tracking
- Per-player completion states
- Challenge passes when logic complete

## Troubleshooting

### Game doesn't start
```
Error: game folder not configured
Solution: First launch asks for game folder with .xex. Point to your game install.
```

### Controllers not detected
```
Check: Controller connected?
  - PS4/PS5: USB or Bluetooth pairing required
  - macOS: System Preferences > Bluetooth > pair controller
Check: SDL3 recognizes it?
  - Look for console output when controllers connect
```

### START press not detected
```
Check: Used REAL START button, not Menu/Share
  - Different button on different controller models
Check: In-game and not in pause menu
  - Pause menu currently single-player only
Check: Controller slot 1 selected
  - Controller detection should be automatic if slot 0 (P1) is active
```

## Architecture Reference

```
LiveSkatingClient.run_tick()
  ├─ two_player_router.advance_frame()
  ├─ two_player_router.check_player2_input(pad1_sample)
  │   └─ Returns Player2InputCheck::StartPressed when START pressed
  └─ If consume_join_request() → true:
      └─ session.two_player_mut().activate_player2(char_id)
          └─ Sets TwoPlayerSession.state = BothActive
              └─ [PENDING] Triggers character spawn
                  └─ [PENDING] Activates split-screen rendering
```

## Files Modified

- `crates/skate2-core/src/session/selected_session_public_facade.rs`
  - Added `two_player: TwoPlayerSession` field
  - Added getter methods `two_player()` and `two_player_mut()`

- `apps/skate2-pc/src/live_skating_client.rs`
  - Integrated `TwoPlayerInputRouter` into `run_tick()`
  - Added join signal handling
  - Calls `session.two_player_mut().activate_player2()`

- `crates/skate2-core/src/input/two_player_input_router.rs`
  - Already fixed (from previous session)
  - Proper START button detection (0x0010 bit)
  - Edge detection for press/release

## Known Limitations

- [x] Currently using placeholder character_id = 1u64
- [x] No character selection for player 2 yet
- [ ] No player 2 character spawn yet (IN PROGRESS)
- [ ] No split-screen rendering yet (IN PROGRESS)
- [ ] No controller input routing to P2 yet
- [ ] No pause menu for player 2 yet
- [ ] No separate cameras yet

## See Also

- `/docs/SPLIT_SCREEN_MULTIPLAYER.md` - Architecture guide
- `/crates/skate2-core/src/session/two_player_session.rs` - Session logic
- `/crates/skate2-core/src/input/two_player_input_router.rs` - Input detection
- `/crates/skate2-core/src/camera/dual_camera_manager.rs` - Camera systems
- `/crates/skate2-core/src/rendering/split_screen_viewport.rs` - Viewport mgmt

