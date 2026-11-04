# Multiplayer Phase 3: Rendering Integration Status

## Completed (This Phase)
- ✅ Extended SelectedVisualSnapshot with player2_camera_* optional fields
- ✅ Implemented PlayerCamera.clip_from_world() matrix computation
- ✅ Added camera vector extraction (forward, right, up)
- ✅ Integrated P2 camera update into LiveSkatingView.update()
- ✅ P2 camera now computed every frame from party_skaters[0] position
- ✅ All player 2 camera data flows through visual snapshot to rendering layer

## Current Status
**Camera computation pipeline complete. P2 camera matrices ready for GPU pipeline.**

All camera data is now available in RetainedFrame.player2_camera_* fields (when P2 active):
- `player2_camera_clip_from_world`: [f32; 16] view-projection matrix
- `player2_camera_world_position`: [f32; 3] camera position
- `player2_camera_forward/right/up`: [f32; 3] basis vectors
- `player2_camera_planes`: [f32; 2] near/far clip planes
- `player2_camera_update_counter`: sync counter with P1

## Next Steps (Phase 3 Continuation)
### Immediate: Dual-Pass Rendering Integration
1. **Update GpuRenderer camera binding for dual-pass**
   - Current: Single camera_binding in wgpu BindGroup
   - Needed: Ability to update camera matrix between passes
   - Approach: Modify GpuRenderer::render_frame() to support optional second camera
   
2. **Implement scissor rects for split-screen**
   - Scissor API already exists (set_scissor_rect used in crowd.rs, shadow_map.rs)
   - Left viewport: x=0, width=display_width/2
   - Right viewport: x=display_width/2, width=display_width/2
   - Height: full display_height for both

3. **Integrate into render_frame pipeline**
   - Check if scene contains P2 camera data
   - Render pass 1: P1 camera + left scissor rect
   - Render pass 2: P2 camera + right scissor rect
   - Maintain existing postfx/UI logic

### Design Decision Needed
**Camera Pipeline Option A: Dual Camera Binding**
- Create two wgpu BindGroups with different matrices
- Switch between them for each pass
- Pro: Minimal changes to existing render pipeline
- Con: Requires BindGroup management

**Camera Pipeline Option B: Dynamic Buffer Update**
- Update camera buffer between passes
- Re-bind with same BindGroup
- Pro: More efficient GPU resource usage
- Con: Synchronization complexity

### Known Constraints
- Scene stereo rendering already handles multiple eye passes
  - Location: render_frame() lines ~1060-1100
  - Could reuse this pattern for split-screen
- Map pipeline encodes on worker threads
  - Must ensure camera updates before worker task dispatch
- Particles and pedestrians updated per-camera
  - May need per-pass updates or shared frustum

## Testing Strategy
1. **P2 camera update verification**
   - ✅ Done: Console logging in LiveSkatingView.update()
   - Check: visual.player2_camera_clip_from_world is Some() when P2 active

2. **Split-screen rendering validation**
   - Left viewport: verify P1 renders correctly
   - Right viewport: verify P2 renders correctly
   - Both: check no overlap or visual artifacts

3. **Performance profiling**
   - GPU time: dual-pass rendering impact
   - CPU time: camera binding update overhead
   - Target: maintain 60 FPS @ 1080p split-screen

## File Changes This Phase
- `apps/skate2-pc/src/live_skating_view.rs`
  - Added P2 camera computation (lines ~675-708)
  - Computes matrices before party moved into RetainedFrame
  - Extracts camera basis vectors for rendering

- `crates/skate2-core/src/session/selected_session_public_facade.rs`
  - Extended SelectedVisualSnapshot with P2 camera fields (lines ~300-310, ~4615-4622)
  - All fields initialized as None, populated when P2 active

- `crates/skate2-core/src/camera/dual_camera_manager.rs`
  - PlayerCamera.clip_from_world() computes view-projection matrix
  - Matrix math helpers: cross(), dot(), normalize(), matrix_multiply()
  - Ready for integration into GPU pipeline

## Dependencies & Blockers
- None at core level
- GPU pipeline integration requires understanding wgpu camera binding strategy
- Recommend: Quick spike on render_frame() camera binding architecture

## Commits This Session
1. "Fix multiplayer input router integration" - Router in game loop
2. "Player 2 spawning now functional" - Character instantiation
3. "Session complete: multiplayer foundation ready for rendering" - Status update
4. "Add Player 2 camera matrix computation to rendering layer" - Matrix math
5. "Add Player 2 camera computation in LiveSkatingView" - Camera update integration
