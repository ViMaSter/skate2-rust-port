# Split-Screen Rendering Integration Guide

**Status:** Research & Architecture Documentation  
**Date:** 2026-11-04  
**Scope:** Integrating `DualCameraManager` and `SplitScreenViewportManager` into the rendering pipeline

---

## 1. Current Single-Camera Rendering Flow

### 1.1 High-Level Flow: Game Tick → GPU Render

```
LiveSkatingClient::frame()
  ├─ Simulation tick (fixed 60 Hz, 16.666ms step)
  ├─ core_session.update() → produces SelectedVisualSnapshot
  ├─ LiveSkatingView::update() → stores camera in RetainedFrame
  ├─ LiveSkatingView::draw() → queries retained camera
  └─ GpuRenderer::present() → renders single viewport with one camera

```

### 1.2 Detailed Flow: Where Camera Is Acquired & Applied

#### Step 1: Simulation produces visual output
**File:** `apps/skate2-pc/src/live_skating_client.rs:1086` (LiveSkatingClient::frame)
```rust
let visual: SelectedVisualSnapshot = core_session.update_frame(...)?;
// visual contains:
//   - provisional_clip_from_world: [f32; 16]
//   - camera_world_position: [f32; 3]
//   - camera_right/up/forward: [f32; 3]
//   - camera_planes: [f32; 2] (near, far)
//   - camera_update_counter: u32
```

**Confidence:** Exact — visible in `SelectedVisualSnapshot` struct in core.

#### Step 2: View layer caches camera in RetainedFrame
**File:** `apps/skate2-pc/src/live_skating_view.rs:687-694`

```rust
self.retained = Some(RetainedFrame {
    camera: CameraView {
        clip_from_world: visual.provisional_clip_from_world,
        position: visual.camera_world_position,
        right: visual.camera_right,
        up: visual.camera_up,
        forward: visual.camera_forward,
        planes: visual.camera_planes,
        update_counter: visual.camera_update_counter,
    },
    // ... other frame state ...
});
```

**Confidence:** Exact — this is the single point where simulation camera meets retained state.

#### Step 3: Draw phase extracts and uses camera
**File:** `apps/skate2-pc/src/live_skating_view.rs:763` (draw method)

Two use patterns:
- **Frontend (menus):** `frame.camera` is a 4×4 matrix passed to `set_frontend_world_view()`
- **Gameplay:** `CameraRelativeClip::new(&clip, [frame.camera[12], camera[13], camera[14]])` creates camera-relative projection

```rust
let camera = CameraRelativeClip::new(&clip, [frame.camera[12], frame.camera[13], frame.camera[14]]);
// Used for:
//   - Character/skater rendering (visual.rs:505, 515, 545, 560)
//   - Shadow culling
//   - Particle camera setup (world_particles::ParticleCamera)
```

**Confidence:** High — pattern is consistent across all draw paths.

#### Step 4: GpuRenderer uses camera via GpuMap
**File:** `apps/skate2-pc/src/gpu_renderer.rs:176` (GpuScene struct)

```rust
pub struct GpuScene {
    pub(crate) map: Option<GpuMap>,  // ← contains camera_binding
    pub objects: Vec<GpuObject>,     // ← individual object transforms
    // ... 30+ other rendering state fields ...
}
```

**Confidence:** High — GpuMap holds the camera buffer; present() method applies it to all render passes.

#### Step 5: Present/render pass applies camera
**File:** `apps/skate2-pc/src/gpu_renderer.rs:1196`

```rust
self.queue.write_buffer(
    &self.camera,  // wgpu::Buffer
    0,
    &map.globals_for_eye(None, eye),  // includes clip_from_world
);
```

Then in render pass (line 1287–1355):
```rust
let mut pass = encoder.begin_render_pass(&RenderPassDescriptor { ... });
pass.set_bind_group(/* group 0 */, &self.camera_binding, &[]);
// All subsequent draws use this single camera
```

**Confidence:** Exact — this is the only place camera is written to GPU.

---

## 2. Where to Insert Split-Screen Logic

### 2.1 Decision Tree: Single vs. Dual Camera

```
if two_player_session.is_player2_active() {
    // Use DualCameraManager
    //   - Render TWO passes with different viewports
    //   - Each pass has its own camera matrix
} else {
    // Use existing single-camera path (no change)
}
```

**Decision Point Location:** `LiveSkatingClient::frame()` or `LiveSkatingView::draw()`

### 2.2 Integration Points (Priority Order)

#### Option A: Insert at LiveSkatingView::draw (RECOMMENDED)
**File:** `apps/skate2-pc/src/live_skating_view.rs`  
**Method:** `draw()` at line 763  
**Advantage:** Clean separation — view layer owns viewport/camera splitting  
**Disadvantage:** Requires refactoring draw() to loop over viewports

**Pseudocode:**
```rust
fn draw(
    &mut self,
    renderer: &GpuRenderer,
    scene: &mut GpuScene,
    now: Instant,
    restore: Option<&PresentationRestorer>,
    tick: Option<&SelectedVisualSnapshot<'_>>,
) -> RenderResult<()> {
    let retained = self.retained.take().ok_or("frame without a completed tick")?;
    
    if is_two_player_mode {
        // Render twice: once for each viewport
        self.draw_viewport(renderer, scene, now, restore, tick, &retained, Viewport::Player1)?;
        self.draw_viewport(renderer, scene, now, restore, tick, &retained, Viewport::Player2)?;
    } else {
        // Existing single-pass path
        self.draw_retained(renderer, scene, now, restore, tick, &retained)?;
    }
    
    self.retained = Some(retained);
    Ok(())
}
```

#### Option B: Insert at GpuRenderer::present (ALTERNATIVE)
**File:** `apps/skate2-pc/src/gpu_renderer.rs`  
**Method:** `present()` at line ~1050  
**Advantage:** Centralized rendering logic  
**Disadvantage:** Scene preparation must know about dual cameras earlier

#### Option C: Insert at LiveSkatingClient::frame (LEAST INVASIVE)
**File:** `apps/skate2-pc/src/live_skating_client.rs`  
**Method:** `frame()` at line 1086  
**Advantage:** Minimal changes to existing render pipeline  
**Disadvantage:** Camera logic bleeds into input/simulation layer

---

## 3. Existing APIs: DualCameraManager & SplitScreenViewportManager

### 3.1 DualCameraManager

**File:** `crates/skate2-core/src/camera/dual_camera_manager.rs`

**Purpose:** Manages two independent `PlayerCamera` structs.

**Key Types:**
```rust
pub struct PlayerCamera {
    pub position: [f32; 3],        // World position
    pub target: [f32; 3],          // Look-at target
    pub up: [f32; 3],              // Up vector (typically [0, 0, 1])
    pub fov_degrees: f32,          // Vertical FOV
    pub near_clip: f32,            // Near plane
    pub far_clip: f32,             // Far plane
}

pub struct DualCameraManager {
    pub player1: PlayerCamera,
    pub player2: PlayerCamera,
    pub smoothing: f32,            // Global smoothing factor (0.0–1.0)
}
```

**Key Methods:**
- `new()` → creates manager with default cameras
- `update_player1_follow(skater_pos, offset, target_offset)` → updates player 1's position
- `update_player2_follow(skater_pos, offset, target_offset)` → updates player 2's position
- `smooth_update_player1/player2(target_position, dt)` → smoothly interpolates
- `set_smoothing(smoothing: f32)` → sets global smoothing (0.0–1.0)
- `reset()` → resets both cameras to defaults

**Current Limitation:** 
- ❌ Does NOT convert to clip-from-world matrices (view-projection)
- ❌ Does NOT integrate with wgpu/rendering backend
- ✓ Purely simulation-side camera data structure

**Confidence:** Exact — code is in repository.

### 3.2 SplitScreenViewportManager

**File:** `crates/skate2-core/src/rendering/split_screen_viewport.rs`

**Purpose:** Manages viewport rectangles for two players.

**Key Types:**
```rust
pub struct ScreenDimensions {
    pub width: u32,
    pub height: u32,
}

pub struct PlayerViewport {
    pub x: u32,              // Left edge in pixels
    pub y: u32,              // Top edge in pixels
    pub width: u32,          // Width in pixels
    pub height: u32,         // Height in pixels
}

pub enum SplitScreenLayout {
    LeftRight,               // 50/50 split, player 1 left
    TopBottom,               // 50/50 split, player 1 top
}

pub struct SplitScreenViewportManager {
    screen: ScreenDimensions,
    layout: SplitScreenLayout,
    player1: PlayerViewport,
    player2: PlayerViewport,
}
```

**Key Methods:**
- `new(screen, layout)` → creates manager, computes viewports
- `player1_viewport() → PlayerViewport`
- `player2_viewport() → PlayerViewport`
- `layout() → SplitScreenLayout`
- `screen() → ScreenDimensions`
- `set_screen_dimensions(screen)` → recalculates on window resize

**Current Limitation:**
- ❌ Does NOT have wgpu render pass setup
- ✓ Purely geometric viewport calculation

**Confidence:** Exact — code is in repository.

---

## 4. Implementation Strategy: Minimal Changes

### 4.1 Camera Transformation Pipeline (MISSING PIECE)

Currently:
```
SelectedVisualSnapshot.camera_* → RetainedFrame.camera → CameraView → draws
                                                           ↓
                                                    [4x4 matrix in GpuMap]
```

**New requirement:**
```
SelectedVisualSnapshot.camera_* 
    ├─ Player 1 camera → clip_from_world_p1 [4x4]
    └─ Player 2 camera → clip_from_world_p2 [4x4]
         (when is_player2_active() == true)
```

**Who computes clip_from_world?**
- Currently: `PresentationRestorer::clip_from_world()` or `provisional_clip_from_world` from visual
- **For dual cameras:** Need to compute TWO matrices from ONE visual's camera data + player 2's override

### 4.2 Architecture: Add Split-Screen Support to LiveSkatingView

**New struct in LiveSkatingView:**
```rust
struct SplitScreenRenderPass {
    camera: DualCameraManager,
    viewports: SplitScreenViewportManager,
    active: bool,  // true when is_player2_active()
}
```

**Initialization (in LiveSkatingView::new or initialize):**
```rust
if core_session.two_player().is_player2_active() {
    let screen = ScreenDimensions::new(renderer.dimensions().0, renderer.dimensions().1);
    self.split_screen = Some(SplitScreenRenderPass {
        camera: DualCameraManager::new(),
        viewports: SplitScreenViewportManager::new(screen, SplitScreenLayout::LeftRight),
        active: true,
    });
}
```

### 4.3 Update Dual Cameras During Tick

**Location:** `LiveSkatingView::update()` or new method called from `LiveSkatingClient::frame()`

```rust
if let Some(split) = &mut self.split_screen {
    // Query player 1 and player 2 camera follow positions from core
    let player1_pos = visual.selected_skater(0).hips_position;  // hypothetical
    let player2_pos = visual.selected_skater(1).hips_position;  // hypothetical
    
    split.camera.update_player1_follow(
        player1_pos,
        DEFAULT_CAMERA_OFFSET,    // [0, -5, 3] typically
        DEFAULT_TARGET_OFFSET,    // [0, 2, 1] typically
    );
    split.camera.update_player2_follow(
        player2_pos,
        DEFAULT_CAMERA_OFFSET,
        DEFAULT_TARGET_OFFSET,
    );
}
```

**Challenge:** Core visual currently provides only ONE camera. Need to query BOTH player positions.

### 4.4 Dual-Pass Rendering in GpuRenderer

**Concept:** Render the same scene twice with different camera matrices and render targets.

**New method in GpuRenderer:**
```rust
fn present_split_screen(
    &mut self,
    scene: &mut GpuScene,
    camera_p1: &DualCameraManager.player1,
    viewport_p1: &PlayerViewport,
    camera_p2: &DualCameraManager.player2,
    viewport_p2: &PlayerViewport,
) -> Result<(), Box<dyn Error>> {
    let surface = self.surface.get_current_texture()?;
    let view = surface.texture.create_view(&Default::default());
    let mut encoder = self.device.create_command_encoder(&Default::default());
    
    // Pass 1: Player 1 viewport
    let clip_p1 = compute_clip_from_world(&camera_p1);
    self.render_viewport_pass(&mut encoder, &view, scene, &clip_p1, &viewport_p1)?;
    
    // Pass 2: Player 2 viewport
    let clip_p2 = compute_clip_from_world(&camera_p2);
    self.render_viewport_pass(&mut encoder, &view, scene, &clip_p2, &viewport_p2)?;
    
    self.queue.submit(std::iter::once(encoder.finish()));
    surface.present();
    Ok(())
}
```

**Required GPU changes:**
- ✓ Two camera buffers (one per player)
- ✓ Two render passes (one per viewport)
- ✓ Scissor test or viewport stencil to isolate each player's framebuffer region
- ❓ Separate shadow passes per camera? (Research needed)

---

## 5. Data Flow: Single vs. Split-Screen

### 5.1 Single-Player (Current)

```
LiveSkatingClient::frame()
  │
  ├─ core_session.update() 
  │   └─ produces SelectedVisualSnapshot {camera_*}
  │
  ├─ LiveSkatingView::update()
  │   └─ RetainedFrame {camera: CameraView}
  │
  ├─ LiveSkatingView::draw()
  │   └─ GpuScene {map: GpuMap}
  │
  └─ GpuRenderer::present()
      └─ [ONE render pass]
          └─ write_buffer(self.camera, clip_from_world)
```

### 5.2 Split-Screen (Proposed)

```
LiveSkatingClient::frame()
  │
  ├─ core_session.update() 
  │   └─ produces SelectedVisualSnapshot {camera_*, player1_pos, player2_pos}
  │
  ├─ LiveSkatingView::update()
  │   ├─ if is_player2_active()
  │   │   └─ DualCameraManager.update_player{1,2}_follow()
  │   └─ RetainedFrame {camera: CameraView}  ← still single, for frames between ticks?
  │
  ├─ LiveSkatingView::draw()
  │   ├─ if is_player2_active()
  │   │   ├─ viewport_p1, camera_p1 = split_screen.get_pass(Player1)
  │   │   ├─ viewport_p2, camera_p2 = split_screen.get_pass(Player2)
  │   │   └─ [TWO draw passes]
  │   └─ GpuScene {map: GpuMap}
  │
  └─ GpuRenderer::present_split_screen()
      ├─ [Pass 1] write_buffer(self.camera, clip_p1) → viewport_p1 rect
      └─ [Pass 2] write_buffer(self.camera, clip_p2) → viewport_p2 rect
```

---

## 6. Blockers & Open Questions

### 6.1 Critical Questions

**Q1: How are player 2's position/camera data provided in the visual frame?**
- ✓ `SelectedVisualSnapshot` currently provides only player 1 camera
- ❓ Need to extend visual frame to include `player2_camera_*` fields?
- ❓ Or query player 2 directly from `core_session` during draw phase?

**Status:** BLOCKER — must coordinate with core rendering to ensure player 2 pose is available.

---

**Q2: What about frames between ticks (when presentation is restored)?**
- ✓ `PresentationRestorer` currently restores player 1 camera only
- ❓ Does it restore two cameras when in split-screen mode?
- ❓ Or do we disable "draw between ticks" in split-screen?

**Status:** MEDIUM — affects smoothness but not correctness.

---

**Q3: Shadow maps and post-FX: one pass or two?**
- ✓ Shadow map is computed once per frame (global to all cameras)
- ❓ Can we reuse it for both player viewports, or must we recompute?
- ❓ Post-FX (bloom, motion blur): per-viewport or global?

**Status:** OPTIMIZATION — correctness is unaffected (global shadow/postfx acceptable for MVP).

---

**Q4: UI/HUD: single or split?**
- ✓ Current marker UI renders once globally
- ❓ Should each player see their own HUD in their viewport?
- ❓ Or is a shared global UI acceptable?

**Status:** DESIGN QUESTION — defer to gameplay designer.

---

### 6.2 Known Limitations

1. **No nested render passes in wgpu**
   - Can't automatically clip viewport; must use scissor test or stencil
   - Solution: Set `wgpu::RenderPass::set_scissor_rect()` before each pass

2. **DualCameraManager doesn't produce clip matrices**
   - Must write math: `[f32; 16]` from camera position/target/fov
   - Recommend: Add `to_clip_from_world(aspect_ratio: f32) → [f32; 16]` method to `PlayerCamera`

3. **Single RetainedFrame stores one camera**
   - In split-screen, need to store TWO cameras or reconstruct on-demand
   - Solution: Cache both player cameras in split-screen state, OR skip frames-between-ticks

4. **ParticleCamera, ShadowCasterInputs expect single camera position**
   - These structs hardcoded for one camera
   - May need refactoring; defer for now (use player 1 camera as fallback)

---

## 7. Recommended Implementation Path (MVP)

### Phase 1: Prepare Core Visual Data (1–2 hours)
- [ ] Extend `SelectedVisualSnapshot` to include `player2_camera_*` fields
- [ ] Update simulation tick to populate player 2 camera when `is_player2_active()`
- [ ] **Confidence:** High — clear extension point

### Phase 2: Add Camera Matrix Conversion (1–2 hours)
- [ ] Add `PlayerCamera::to_clip_from_world(aspect: f32) → [f32; 16]` method to `DualCameraManager`
- [ ] Use existing math from `PresentationRestorer::clip_from_world()` as reference
- [ ] **Confidence:** High — math is unchanged

### Phase 3: Integrate SplitScreenRenderPass into LiveSkatingView (2–3 hours)
- [ ] Add `Option<SplitScreenRenderPass>` field to `LiveSkatingView`
- [ ] Initialize in `initialize()` if `is_player2_active()`
- [ ] Update in `update()` tick → feeds player positions to `DualCameraManager`
- [ ] **Confidence:** Medium — need to route player 2 position data

### Phase 4: Dual-Pass Rendering in GpuRenderer (3–4 hours)
- [ ] Add `present_split_screen()` method
- [ ] Implement per-viewport render passes with scissor rect
- [ ] Test with synthetic scene (no shadow/postfx complexity)
- [ ] **Confidence:** High — wgpu documentation is clear

### Phase 5: Integration & Testing (2–3 hours)
- [ ] Wire up `LiveSkatingClient::frame()` to call `present_split_screen()` when needed
- [ ] Manual split-screen test: two controllers, verify both viewports render
- [ ] Verify camera follows, UI doesn't duplicate, no GPU errors
- [ ] **Confidence:** Medium — integration points need validation

### Phase 6 (Optional): Refinements
- [ ] Shadow map optimization (reuse or per-camera)
- [ ] Post-FX per-viewport handling
- [ ] Frames-between-ticks support (two restored cameras)
- [ ] HUD split (mirror current UI in each viewport)

---

## 8. File Changes Summary

### Create New Files
- None (all integration into existing files)

### Modify Files

| File | Changes | Scope |
|------|---------|-------|
| `crates/skate2-core/src/camera/dual_camera_manager.rs` | Add `to_clip_from_world()` method | 20 lines |
| `apps/skate2-pc/src/live_skating_view.rs` | Add `SplitScreenRenderPass` field; update `update()` and `draw()` | 100–150 lines |
| `apps/skate2-pc/src/gpu_renderer.rs` | Add `present_split_screen()` method | 100–200 lines |
| `apps/skate2-pc/src/live_skating_client.rs` | Call `present_split_screen()` in `frame()` | 5–10 lines |
| **Core visual** (TBD) | Extend `SelectedVisualSnapshot` with player 2 camera data | ~30 lines |

### No Changes Required
- `crates/skate2-core/src/rendering/split_screen_viewport.rs` (complete, no rendering backend)
- Window/input layers (work unchanged)

---

## 9. Testing Strategy

### Unit Tests (Per-Component)
```rust
#[test]
fn dual_camera_manager_updates_player_positions() { }

#[test]
fn split_screen_viewport_computes_left_right_50_50() { }

#[test]
fn split_screen_viewport_recomputes_on_resize() { }
```

### Integration Tests (Full Path)
```rust
#[test]
fn split_screen_render_both_players() {
    // Setup: Two controllers, spawn player 2
    // Tick frame: verify visual includes player2_camera_*
    // Render frame: verify two render passes, two camera matrices
    // Assert: scene.map.camera buffers differ by player 2 offset
}
```

### Manual Tests (Visual)
- [ ] Start game, spawn 2 players
- [ ] Verify left/right viewports show both skaters at correct positions
- [ ] Move player 1 left → verify only left viewport updates
- [ ] Move player 2 right → verify only right viewport updates
- [ ] Pause/resume → verify cameras stay in sync
- [ ] Camera collision test → both cameras avoid obstacles independently

---

## 10. Reference: Existing Camera Code

**Single camera extraction:**
- `apps/skate2-pc/src/live_skating_view.rs:436` — `let camera = frame.camera;`
- `apps/skate2-pc/src/live_skating_view.rs:488` — `CameraRelativeClip::new(&clip, camera_position)`

**Camera in GPU:**
- `apps/skate2-pc/src/gpu_renderer.rs:302–320` — `CameraRelativeClip` struct
- `apps/skate2-pc/src/gpu_renderer.rs:424–425` — camera wgpu::Buffer
- `apps/skate2-pc/src/gpu_renderer.rs:1196` — camera write_buffer()

**Viewport setup:**
- `apps/skate2-pc/src/gpu_renderer.rs:1287–1355` — render pass setup (set_bind_group for camera)

**Window dimensions:**
- `apps/skate2-pc/src/gpu_renderer.rs` — `pub fn dimensions() → (u32, u32)`

---

## Checklist for Integration Engineer

- [ ] Read this document in full
- [ ] Run `cargo test --package skate2-core --lib camera::dual_camera_manager` to verify manager is sound
- [ ] Run `cargo test --package skate2-core --lib rendering::split_screen_viewport` to verify viewport math
- [ ] Locate where player 2 camera data flows from simulation to view layer
- [ ] Decide: frames-between-ticks support (optional for MVP)
- [ ] Decide: shadow/postfx per-camera or global (recommended: global MVP)
- [ ] Implement Phase 1 (core visual extension)
- [ ] Implement Phase 2 (camera matrix conversion)
- [ ] Implement Phase 3–4 (view & renderer integration)
- [ ] Run full test suite: `cargo test --workspace`
- [ ] Manual split-screen test with real controllers
- [ ] Commit & integrate

---

## Summary

**Current State:**
- ✓ `DualCameraManager` and `SplitScreenViewportManager` exist but are unused
- ✓ Single-camera rendering fully integrated end-to-end
- ❌ No dual-camera path exists in rendering pipeline

**Key Insight:**
Split-screen rendering requires **two independent render passes**, each with its own camera matrix and viewport rect. The GPU already supports this; the integration is a straightforward dual-pass loop.

**Estimated Effort:** 12–18 hours (5 phases, including testing).

**Risk:** Low — changes are isolated to view and renderer layers; core simulation is unchanged.
