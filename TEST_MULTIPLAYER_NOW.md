# Quick Test Guide: Skate 2 Multiplayer Right Now

## Prerequisites

- Two controllers connected:
  - Controller 0: DualShock 4 or DualSense (Player 1)
  - Controller 1: DualShock 4 or DualSense (Player 2)
- macOS (or run on Windows with `skate2-pc.exe`)
- Game folder configured in `config.json`

## Test Steps

### 1. Build Latest
```bash
cd /Users/vincent.mahnke/skate2-rust-port
~/.cargo/bin/cargo build --release --bin skate2-pc
```

Expected output:
```
Finished `release` profile [optimized + debuginfo] target(x) in 42.92s
```

### 2. Run Game
```bash
./dist/skate2-pc-multiplayer-p2-spawn-ready
# or after building:
./target/release/skate2-pc
```

### 3. Load into Game
- Navigate past menus
- Start a challenge or free skate location
- Wait for location to load (see skater on screen)

### 4. Join Player 2
- **Press START on Controller 1 (the second controller)**
- Watch console/terminal for:
  ```
  Skate 2: Player 2 START pressed - activating player 2!
  Skate 2: Player 2 activated in session
  Player 2 spawned successfully at [x, y, z]
  ```

### 5. What You Should See (Current Phase)

**✅ Working:**
- Console shows player 2 detection messages
- Game continues running normally
- No crashes or errors

**⏳ Not Yet Visible (Next Phase):**
- Second player character in the game world
- Split-screen camera view
- Controller 2 movement

## Detailed: What Each Message Means

| Console Output | Meaning | Status |
|---|---|---|
| `Player 2 START pressed` | Router detected START button | ✅ Working |
| `Player 2 activated in session` | Session state changed to BothActive | ✅ Working |
| `Player 2 spawned successfully at [x,y,z]` | Character created in world | ✅ Working |
| (no second player visible) | Rendering not yet integrated | ⏳ In Progress |

## Troubleshooting

### Issue: No console output when pressing START
**Cause:** Controller 2 not detected or wrong slot  
**Solution:**
- Verify both controllers are in SDL diagnostics (check console on startup)
- Ensure slot 0 = first controller, slot 1 = second controller
- Try pressing START on the second controller again (debounce window is 3 frames)

### Issue: Game crashes
**Cause:** Likely a rendering or physics issue  
**Solution:**
- Check terminal for error message
- Ensure game folder is set in config.json
- Try with a different location

### Issue: Player 2 message appears but nothing happens
**Cause:** Rendering integration not yet complete  
**Status:** This is expected! Next phase will show the second player

## Next Session: Rendering Integration

When the next session completes rendering integration, you should see:

1. Screen splits in half (left/right)
2. Player 1 visible on left side
3. Player 2 visible on right side (5m behind initially)
4. Both players movable with their controllers
5. Independent cameras following each player

**Expected completion:** 4-6 hours

---

## For Developers: Testing Checklist

Use this to verify the multiplayer foundation during development:

- [ ] Build completes without errors
- [ ] `cargo clippy` passes with `-D warnings`
- [ ] `cargo test` all pass
- [ ] Binary runs without crash on startup
- [ ] Console shows controller detection
- [ ] START press on controller 2 triggers console output
- [ ] Session state prints "BothActive"
- [ ] Spawn location is reasonable (not underground/out of bounds)

## File Manifest

| File | Purpose |
|------|---------|
| `dist/skate2-pc-multiplayer-p2-spawn-ready` | Latest test binary |
| `MULTIPLAYER_SESSION_STATUS.md` | Complete session report |
| `SPLIT_SCREEN_RENDERING.md` | Rendering architecture (next phase) |
| `TWO_PLAYER_PAUSE.md` | Pause menu integration (later phase) |

---

**Quick Summary:** The multiplayer foundation is complete and working. You can now press START on a second controller and see confirmation messages in the console. Rendering integration (making the second player visible) comes next.

