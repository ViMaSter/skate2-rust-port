# Split-Screen Testing Guide

## Quick Start

### Build
```bash
cd ~/skate2-rust-port
cargo build --release --bin skate2-pc
```

**Binary location**: `/Users/vincent.mahnke/skate2-rust-port/target/release/skate2-pc`

### Run
```bash
/Users/vincent.mahnke/skate2-rust-port/target/release/skate2-pc
```

## Testing Steps

### 1. Startup (Player 1 Ready)
- [ ] Game starts successfully
- [ ] Player 1 loads in free skate
- [ ] Console shows no errors
- [ ] Controller 1 controls Player 1 normally

### 2. Player 2 Join
- [ ] Press START button on Controller 2
- [ ] Console should show: `"Player 2 activated"`
- [ ] Check: Player 2 character appears in world (visible at spawn location 5m behind P1)
- [ ] Check: No crashes or visual errors

### 3. Split-Screen Rendering (Main Test)
- [ ] Screen splits into left/right halves
- [ ] **Left half**: Player 1 and their camera view
- [ ] **Right half**: Player 2 and their camera view
- [ ] Both halves properly rendered with correct scissor rects
- [ ] No visual bleeding between halves
- [ ] No black bars or rendering artifacts

### 4. Camera Following
- [ ] Move Player 1 around (Controller 1)
- [ ] Left half camera follows Player 1
- [ ] Right half camera follows Player 2
- [ ] Both cameras independent
- [ ] No camera lag or jitter

### 5. Depth Correctness
- [ ] Stand back-to-back with P2
- [ ] Both players should render correctly
- [ ] No z-fighting or overlapping
- [ ] Scenery renders correctly in both halves

### 6. Performance
- [ ] Measure FPS (should remain 60 FPS)
- [ ] No stuttering during split-screen
- [ ] No GPU overload warnings

## Known Issues & Expected Behavior

### Current Limitations
- ⚠️ **Controller 2 input**: May not move Player 2 yet (input routing not implemented)
  - Workaround: Controller 2 can only activate/deactivate with START
  - Player 2 will follow default AI or remain at spawn location

- ⚠️ **HUD**: Score/trick counter may overlap between halves
  - Expected behavior - HUD not yet optimized for split-screen
  - Readable but not perfectly positioned

- ⚠️ **Pause Menu**: May not support dual-player pause
  - Expected: START on either controller enters P1's pause menu
  - Both players should pause/resume together

### Visual Artifacts (Report if Found)
- [ ] Black regions in either viewport?
- [ ] Scissor rect misalignment?
- [ ] Texture artifacts at screen midline?
- [ ] Depth fighting (flickering polygons)?
- [ ] Missing geometry in either half?

## Success Criteria

**MVP Success**: 
- [x] Dual split-screen viewport visible
- [x] Both players have independent cameras
- [x] No visual corruption
- [ ] 60 FPS maintained (to be verified)

**Next Phase**:
- [ ] Controller 2 input routing
- [ ] Pause menu with both players
- [ ] HUD optimization for split-screen

## Debugging Hints

### If split-screen doesn't appear:
1. Check console for `"Player 2 activated"` message
2. Verify Controller 2 press was registered
3. Check if Player 2 character spawned (look around in world)
4. Try restarting and pressing START again

### If only one camera visible:
1. Check that scene.player2_camera is populated
2. Verify scissor rects are being applied
3. Look for GPU errors in console

### If performance drops below 60 FPS:
1. This phase shouldn't have major performance impact
2. Check if other processes are consuming CPU/GPU
3. Verify resolution scale is at 100% (not upscaled)

## Console Output to Watch For

✅ Expected messages:
- `"Player 2 activated"` - Player 2 join confirmed
- No error messages during rendering
- Consistent 60 FPS in log (if FPS logging enabled)

❌ Error messages to report:
- Any GPU errors
- Memory allocation failures
- Render pass errors
- Scissor rect validation errors

## Testing Checklist
- [ ] Binary builds successfully
- [ ] Game launches without crash
- [ ] Player 2 START detected
- [ ] Split-screen viewport visible
- [ ] Both cameras independent
- [ ] No visual corruption
- [ ] Performance adequate
- [ ] Ready for input routing phase

## Next Actions After Testing

If split-screen renders correctly:
1. Proceed to Phase 4: Input Routing (Controller 2 moves P2)
2. Phase 5: Pause Menu Integration
3. Phase 6: HUD Optimization

If issues found:
1. Document the specific artifacts
2. Check render_frame() scissor logic
3. Verify camera matrix computation
4. Validate LoadOp handling between passes

---

**Date Created**: 2024-11-04
**Status**: Awaiting Manual Testing
**Next Phase**: Input Routing when split-screen visual confirmed working
