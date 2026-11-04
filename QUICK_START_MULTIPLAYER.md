# Quick Start: Multiplayer Split-Screen

## One-Minute Setup

```bash
# Build
cd ~/skate2-rust-port && cargo build --release --bin skate2-pc

# Run
./target/release/skate2-pc

# Activate Player 2
Press START on Controller 2
```

## What You'll See ✨
- **Left half**: Player 1 skateboarding
- **Right half**: Player 2 skateboarding
- Both players with independent cameras
- Full split-screen rendering

## Current Status

### ✅ Working
- Player detection (START press activates P2)
- Character spawning (P2 appears in world)
- Camera computation (P2 camera follows independently)
- Dual-pass rendering (split-screen visible)
- Scissor rect configuration (clean left/right separation)

### ⏳ Coming Next (Phase 4)
- Controller 2 input routing (move P2 directly)
- Pause menu for 2 players
- HUD optimization

## Troubleshooting

**"Player 2 doesn't appear"**
- Check console for "Player 2 activated" message
- Try pressing START again with controller 2

**"No split-screen visible"**
- Verify both players spawned (look around with P1)
- Check GPU is rendering both passes
- Check for console errors

**"Game crashes"**
- Report the crash message
- Verify you're using the latest built binary

## Performance
- Target: 60 FPS
- Overhead: ~20-40% GPU usage for second pass

## File Locations
- **Binary**: `/Users/vincent.mahnke/skate2-rust-port/target/release/skate2-pc`
- **Source**: `/Users/vincent.mahnke/skate2-rust-port`
- **Test Guide**: `TESTING_SPLIT_SCREEN.md`

## Architecture Overview
```
Player 1 (Left Half)          Player 2 (Right Half)
├─ Controls: Controller 1      ├─ Controls: Controller 2 (WIP)
├─ Camera: Independent        ├─ Camera: Independent
├─ Scissor: x=0, w/2, h       ├─ Scissor: x=w/2, w/2, h
└─ Render Pass 1              └─ Render Pass 2
```

## What We Built This Session

### Phase 3a: Camera Math
- PlayerCamera.clip_from_world() computes matrices
- Matrix helpers: cross product, normalization, etc.

### Phase 3b: Data Pipeline  
- PlayerCameraSnapshot carries all GPU data
- GpuScene.player2_camera stores it
- LiveSkatingView populates from visual snapshot

### Phase 3c: Dual-Pass Rendering ⭐
- render_frame() detects split-screen mode
- Two render passes with different cameras
- Scissor rects split screen horizontally
- Depth buffer shared between passes

## Next Phase
- Extract controller 2 input
- Route to Player 2's character controller
- Verify independent movement

## Want More Details?
- **Technical Deep Dive**: `MULTIPLAYER_STATUS_COMPLETE.md`
- **Testing Procedures**: `TESTING_SPLIT_SCREEN.md`
- **Full Session Report**: `SESSION_SUMMARY.md`

---

**Status**: 🟢 Ready for Testing
**Last Updated**: November 4, 2024
**Binary**: Up-to-date (Release build)
