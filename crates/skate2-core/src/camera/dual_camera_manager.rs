//! Dual-camera system for split-screen two-player gameplay.
//!
//! Manages two independent cameras following each player separately.

use std::fmt;

/// Camera configuration for one player.
#[derive(Clone, Copy, Debug)]
pub struct PlayerCamera {
    /// Camera position (world coordinates, typically x/y on ground plane).
    pub position: [f32; 3],
    /// Where the camera is looking (target point).
    pub target: [f32; 3],
    /// Camera up vector (typically [0, 0, 1] for z-up world).
    pub up: [f32; 3],
    /// Vertical field of view in degrees.
    pub fov_degrees: f32,
    /// Near clipping plane distance.
    pub near_clip: f32,
    /// Far clipping plane distance.
    pub far_clip: f32,
}

impl PlayerCamera {
    /// Create a new player camera with default values.
    pub fn new() -> Self {
        Self {
            position: [0.0, 0.0, 5.0],
            target: [0.0, 0.0, 0.0],
            up: [0.0, 0.0, 1.0],
            fov_degrees: 60.0,
            near_clip: 0.1,
            far_clip: 1000.0,
        }
    }

    /// Update camera position to follow skater at given position with offset.
    /// `skater_pos`: world position of the skater (x, y, z).
    /// `offset`: relative offset from skater (typically behind and above).
    /// `target_offset`: offset for where camera looks relative to skater.
    pub fn follow_skater(&mut self, skater_pos: [f32; 3], offset: [f32; 3], target_offset: [f32; 3]) {
        self.position = [
            skater_pos[0] + offset[0],
            skater_pos[1] + offset[1],
            skater_pos[2] + offset[2],
        ];
        self.target = [
            skater_pos[0] + target_offset[0],
            skater_pos[1] + target_offset[1],
            skater_pos[2] + target_offset[2],
        ];
    }

    /// Smoothly interpolate camera position towards a target.
    /// `dt`: delta time in seconds.
    /// `smoothing`: smoothing factor (0.0-1.0, higher = faster).
    pub fn smooth_follow(&mut self, target_position: [f32; 3], dt: f32, smoothing: f32) {
        let alpha = (smoothing * dt).min(1.0);
        self.position[0] += (target_position[0] - self.position[0]) * alpha;
        self.position[1] += (target_position[1] - self.position[1]) * alpha;
        self.position[2] += (target_position[2] - self.position[2]) * alpha;
    }

    /// Get the forward vector (from position towards target).
    pub fn forward(&self) -> [f32; 3] {
        let dx = self.target[0] - self.position[0];
        let dy = self.target[1] - self.position[1];
        let dz = self.target[2] - self.position[2];
        let len = (dx * dx + dy * dy + dz * dz).sqrt();
        if len > 0.0001 {
            [dx / len, dy / len, dz / len]
        } else {
            [0.0, 1.0, 0.0]
        }
    }
}

impl Default for PlayerCamera {
    fn default() -> Self {
        Self::new()
    }
}

/// Manages two independent cameras for split-screen gameplay.
pub struct DualCameraManager {
    /// Player 1's camera.
    pub player1: PlayerCamera,
    /// Player 2's camera.
    pub player2: PlayerCamera,
    /// Global camera smoothing factor (0.0-1.0).
    pub smoothing: f32,
}

impl DualCameraManager {
    /// Create a new dual-camera manager.
    pub fn new() -> Self {
        Self {
            player1: PlayerCamera::new(),
            player2: PlayerCamera::new(),
            smoothing: 0.5,
        }
    }

    /// Update both cameras with delta time (for smoothing).
    pub fn advance(&mut self, dt: f32) {
        // Smoothing is applied in follow operations, not here.
        // This method is reserved for any global camera logic.
        let _ = dt;
    }

    /// Update player 1's camera to follow their skater.
    pub fn update_player1_follow(&mut self, skater_pos: [f32; 3], offset: [f32; 3], target_offset: [f32; 3]) {
        self.player1.follow_skater(skater_pos, offset, target_offset);
    }

    /// Update player 2's camera to follow their skater.
    pub fn update_player2_follow(&mut self, skater_pos: [f32; 3], offset: [f32; 3], target_offset: [f32; 3]) {
        self.player2.follow_skater(skater_pos, offset, target_offset);
    }

    /// Smoothly move player 1's camera.
    pub fn smooth_update_player1(&mut self, target_position: [f32; 3], dt: f32) {
        self.player1.smooth_follow(target_position, dt, self.smoothing);
    }

    /// Smoothly move player 2's camera.
    pub fn smooth_update_player2(&mut self, target_position: [f32; 3], dt: f32) {
        self.player2.smooth_follow(target_position, dt, self.smoothing);
    }

    /// Set global camera smoothing (0.0 = instant, 1.0 = very smooth).
    pub fn set_smoothing(&mut self, smoothing: f32) {
        self.smoothing = smoothing.clamp(0.0, 1.0);
    }

    /// Reset both cameras to default positions.
    pub fn reset(&mut self) {
        self.player1 = PlayerCamera::new();
        self.player2 = PlayerCamera::new();
    }
}

impl Default for DualCameraManager {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for DualCameraManager {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DualCameraManager")
            .field("player1_position", &self.player1.position)
            .field("player2_position", &self.player2.position)
            .field("smoothing", &self.smoothing)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn camera_follow_skater() {
        let mut cam = PlayerCamera::new();
        let skater_pos = [10.0, 20.0, 0.0];
        let offset = [0.0, -5.0, 3.0];
        let target_offset = [0.0, 2.0, 1.0];

        cam.follow_skater(skater_pos, offset, target_offset);

        assert_eq!(cam.position, [10.0, 15.0, 3.0]);
        assert_eq!(cam.target, [10.0, 22.0, 1.0]);
    }

    #[test]
    fn camera_smooth_follow() {
        let mut cam = PlayerCamera::new();
        cam.position = [0.0, 0.0, 0.0];

        cam.smooth_follow([10.0, 0.0, 0.0], 1.0, 0.1); // 10% per second
        assert!(cam.position[0] > 0.9 && cam.position[0] < 1.1); // ~1.0
    }

    #[test]
    fn camera_forward_vector() {
        let mut cam = PlayerCamera::new();
        cam.position = [0.0, 0.0, 0.0];
        cam.target = [1.0, 0.0, 0.0];

        let fwd = cam.forward();
        assert!((fwd[0] - 1.0).abs() < 0.01);
        assert!(fwd[1].abs() < 0.01);
        assert!(fwd[2].abs() < 0.01);
    }

    #[test]
    fn dual_camera_manager() {
        let mut manager = DualCameraManager::new();

        manager.update_player1_follow([5.0, 5.0, 0.0], [0.0, -5.0, 3.0], [0.0, 0.0, 1.0]);
        assert_eq!(manager.player1.position, [5.0, 0.0, 3.0]);

        manager.update_player2_follow([15.0, 15.0, 0.0], [0.0, -5.0, 3.0], [0.0, 0.0, 1.0]);
        assert_eq!(manager.player2.position, [15.0, 10.0, 3.0]);
    }

    #[test]
    fn smoothing_clamp() {
        let mut manager = DualCameraManager::new();
        manager.set_smoothing(2.0); // Over 1.0
        assert_eq!(manager.smoothing, 1.0);

        manager.set_smoothing(-0.5); // Below 0.0
        assert_eq!(manager.smoothing, 0.0);
    }
}
