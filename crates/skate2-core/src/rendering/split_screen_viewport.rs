//! Split-screen viewport configuration and management.
//!
//! Handles rendering configuration for two players displayed side-by-side.
//! Player 1 uses the left half of the viewport, Player 2 uses the right half.

use std::fmt;

/// Screen dimensions and layout information.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ScreenDimensions {
    /// Total screen width in pixels.
    pub width: u32,
    /// Total screen height in pixels.
    pub height: u32,
}

impl ScreenDimensions {
    /// Create screen dimensions.
    pub fn new(width: u32, height: u32) -> Self {
        Self { width, height }
    }

    /// Get the aspect ratio (width / height).
    pub fn aspect_ratio(&self) -> f32 {
        self.width as f32 / self.height.max(1) as f32
    }
}

/// Viewport configuration for one player in split-screen.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PlayerViewport {
    /// Left edge in pixels (x).
    pub x: u32,
    /// Top edge in pixels (y).
    pub y: u32,
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
}

impl PlayerViewport {
    /// Create a viewport at the given position and size.
    pub fn new(x: u32, y: u32, width: u32, height: u32) -> Self {
        Self { x, y, width, height }
    }

    /// Get the aspect ratio of this viewport.
    pub fn aspect_ratio(&self) -> f32 {
        self.width as f32 / self.height.max(1) as f32
    }

    /// Check if a pixel coordinate (x, y) is within this viewport.
    pub fn contains_point(&self, px: u32, py: u32) -> bool {
        px >= self.x && px < (self.x + self.width) &&
        py >= self.y && py < (self.y + self.height)
    }

    /// Normalize pixel coordinates to viewport space (0.0 to 1.0).
    pub fn normalize_point(&self, px: u32, py: u32) -> Option<(f32, f32)> {
        if self.contains_point(px, py) {
            let x = (px - self.x) as f32 / self.width.max(1) as f32;
            let y = (py - self.y) as f32 / self.height.max(1) as f32;
            Some((x, y))
        } else {
            None
        }
    }
}

/// Split-screen layout mode.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SplitScreenLayout {
    /// Player 1 on left, Player 2 on right (50/50 split).
    LeftRight,
    /// Player 1 on top, Player 2 on bottom (50/50 split).
    TopBottom,
}

/// Manages viewport configuration for two-player split-screen.
pub struct SplitScreenViewportManager {
    /// Screen resolution.
    screen: ScreenDimensions,
    /// Layout mode (left/right vs. top/bottom).
    layout: SplitScreenLayout,
    /// Player 1 viewport.
    player1: PlayerViewport,
    /// Player 2 viewport.
    player2: PlayerViewport,
}

impl SplitScreenViewportManager {
    /// Create a new split-screen viewport manager.
    pub fn new(screen: ScreenDimensions, layout: SplitScreenLayout) -> Self {
        let (player1, player2) = Self::compute_viewports(screen, layout);
        Self {
            screen,
            layout,
            player1,
            player2,
        }
    }

    /// Compute viewport rectangles for both players given screen dimensions and layout.
    fn compute_viewports(screen: ScreenDimensions, layout: SplitScreenLayout) -> (PlayerViewport, PlayerViewport) {
        match layout {
            SplitScreenLayout::LeftRight => {
                let half_width = screen.width / 2;
                (
                    PlayerViewport::new(0, 0, half_width, screen.height),
                    PlayerViewport::new(half_width, 0, screen.width - half_width, screen.height),
                )
            }
            SplitScreenLayout::TopBottom => {
                let half_height = screen.height / 2;
                (
                    PlayerViewport::new(0, 0, screen.width, half_height),
                    PlayerViewport::new(0, half_height, screen.width, screen.height - half_height),
                )
            }
        }
    }

    /// Get player 1's viewport.
    pub fn player1_viewport(&self) -> PlayerViewport {
        self.player1
    }

    /// Get player 2's viewport.
    pub fn player2_viewport(&self) -> PlayerViewport {
        self.player2
    }

    /// Get the layout mode.
    pub fn layout(&self) -> SplitScreenLayout {
        self.layout
    }

    /// Get screen dimensions.
    pub fn screen(&self) -> ScreenDimensions {
        self.screen
    }

    /// Update screen dimensions (e.g., on window resize).
    /// Recomputes viewports for the new resolution.
    pub fn set_screen_dimensions(&mut self, screen: ScreenDimensions) {
        self.screen = screen;
        let (player1, player2) = Self::compute_viewports(screen, self.layout);
        self.player1 = player1;
        self.player2 = player2;
    }

    /// Change the split-screen layout mode.
    /// Recomputes viewports for the new layout.
    pub fn set_layout(&mut self, layout: SplitScreenLayout) {
        self.layout = layout;
        let (player1, player2) = Self::compute_viewports(self.screen, layout);
        self.player1 = player1;
        self.player2 = player2;
    }

    /// Determine which player's viewport contains a given screen coordinate.
    pub fn get_player_at_point(&self, px: u32, py: u32) -> Option<u32> {
        if self.player1.contains_point(px, py) {
            Some(0)
        } else if self.player2.contains_point(px, py) {
            Some(1)
        } else {
            None
        }
    }
}

impl fmt::Debug for SplitScreenViewportManager {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SplitScreenViewportManager")
            .field("screen", &self.screen)
            .field("layout", &self.layout)
            .field("player1", &self.player1)
            .field("player2", &self.player2)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn left_right_split_1920x1080() {
        let screen = ScreenDimensions::new(1920, 1080);
        let manager = SplitScreenViewportManager::new(screen, SplitScreenLayout::LeftRight);

        let p1 = manager.player1_viewport();
        assert_eq!(p1.x, 0);
        assert_eq!(p1.width, 960); // Half of 1920
        assert_eq!(p1.height, 1080); // Full height

        let p2 = manager.player2_viewport();
        assert_eq!(p2.x, 960);
        assert_eq!(p2.width, 960);
        assert_eq!(p2.height, 1080);
    }

    #[test]
    fn top_bottom_split_1920x1080() {
        let screen = ScreenDimensions::new(1920, 1080);
        let manager = SplitScreenViewportManager::new(screen, SplitScreenLayout::TopBottom);

        let p1 = manager.player1_viewport();
        assert_eq!(p1.x, 0);
        assert_eq!(p1.y, 0);
        assert_eq!(p1.width, 1920);
        assert_eq!(p1.height, 540); // Half of 1080

        let p2 = manager.player2_viewport();
        assert_eq!(p2.x, 0);
        assert_eq!(p2.y, 540);
        assert_eq!(p2.width, 1920);
        assert_eq!(p2.height, 540);
    }

    #[test]
    fn player_at_point_left_right() {
        let screen = ScreenDimensions::new(1920, 1080);
        let manager = SplitScreenViewportManager::new(screen, SplitScreenLayout::LeftRight);

        // Point in player 1's viewport
        assert_eq!(manager.get_player_at_point(100, 100), Some(0));

        // Point in player 2's viewport
        assert_eq!(manager.get_player_at_point(1500, 100), Some(1));

        // Point outside both (shouldn't happen normally)
        // (no such point exists in perfect 50/50 split)
    }

    #[test]
    fn viewport_contains_point() {
        let vp = PlayerViewport::new(100, 100, 200, 200);

        assert!(vp.contains_point(150, 150));
        assert!(vp.contains_point(100, 100)); // Left edge
        assert!(!vp.contains_point(99, 100)); // Just outside
        assert!(!vp.contains_point(300, 300)); // Way outside
    }

    #[test]
    fn viewport_normalize_point() {
        let vp = PlayerViewport::new(100, 100, 200, 200);

        let norm = vp.normalize_point(150, 150);
        assert_eq!(norm, Some((0.25, 0.25))); // 50 pixels in a 200-pixel viewport = 0.25

        let norm = vp.normalize_point(300, 300); // Outside
        assert_eq!(norm, None);
    }

    #[test]
    fn screen_resize_updates_viewports() {
        let mut manager = SplitScreenViewportManager::new(
            ScreenDimensions::new(1920, 1080),
            SplitScreenLayout::LeftRight,
        );

        manager.set_screen_dimensions(ScreenDimensions::new(1280, 720));

        let p1 = manager.player1_viewport();
        assert_eq!(p1.width, 640); // Half of 1280
        assert_eq!(p1.height, 720);
    }

    #[test]
    fn layout_switch_updates_viewports() {
        let mut manager = SplitScreenViewportManager::new(
            ScreenDimensions::new(1920, 1080),
            SplitScreenLayout::LeftRight,
        );

        manager.set_layout(SplitScreenLayout::TopBottom);

        let p1 = manager.player1_viewport();
        assert_eq!(p1.width, 1920);
        assert_eq!(p1.height, 540); // Half of 1080
    }
}
