//! PC extension, not in the Xbox 360 reference, which draws one local view per frame: local
//! two-player split screen (`docs/SPLIT_SCREEN_MULTIPLAYER.md`). Each half is one of the
//! stereo renderer's geometry passes (`stereoscopy`): every producer keeps the first player's
//! clip-from-local and the half applies one correction to it. The left half is the first
//! player's camera; the right half is the same camera rig carried to the second player.

use super::{
    RenderResult,
    stereoscopy::{self, Eye},
};

/// Each half's share of the target's width; the rest between them stays the scene's clear
/// colour as the divider.
pub(crate) const HALF_WIDTH: f32 = 0.498;

/// The second player's view: `world_remap` takes a world point near the second player to
/// where the first player's camera sees the same relative position, so `world_remap` maps
/// `camera` onto the first player's camera.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct SplitScreenView {
    pub world_remap: [f32; 16],
    pub camera: [f32; 3],
    /// The second camera's right, up and forward axes, w = 0.
    pub axes: [[f32; 4]; 3],
    /// The window clip x, in NDC, both halves centre their narrower field on.
    pub centre: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Half {
    Left,
    Right,
}

/// One half's geometry pass.
#[derive(Clone, Copy)]
pub(super) struct SplitPass {
    pub half: Half,
    pub eye: Eye,
    /// The view position and axes the map's globals, selection and DMOs take; the first
    /// player's on the left half.
    pub view: Option<([f32; 3], [[f32; 4]; 3])>,
    /// x, y, width and height in the target's pixels.
    pub viewport: [u32; 4],
}

impl SplitPass {
    /// Both halves of a target for the first player's window clip-from-world `clip`.
    pub(super) fn pair(
        clip: [f32; 16],
        view: &SplitScreenView,
        target: (u32, u32),
        planes: [f32; 2],
    ) -> RenderResult<[Self; 2]> {
        let [left, right] = viewports(target);
        let first = clip.map(f64::from);
        let left_squeeze = squeeze(target.0, left[2], view.centre);
        let right_clip = stereoscopy::multiply(
            stereoscopy::multiply(squeeze(target.0, right[2], view.centre), first),
            view.world_remap.map(f64::from),
        );
        let right_correction = stereoscopy::multiply(right_clip, stereoscopy::inverse(first)?);
        Ok([
            Self {
                half: Half::Left,
                eye: Eye::split_half(
                    left_squeeze,
                    narrow(stereoscopy::multiply(left_squeeze, first)),
                    planes,
                ),
                view: None,
                viewport: left,
            },
            Self {
                half: Half::Right,
                eye: Eye::split_half(right_correction, narrow(right_clip), planes),
                view: Some((view.camera, view.axes)),
                viewport: right,
            },
        ])
    }

    pub(super) fn set_viewport(&self, pass: &mut wgpu::RenderPass<'_>) {
        let [x, y, width, height] = self.viewport;
        pass.set_viewport(
            pixels(x),
            pixels(y),
            pixels(width),
            pixels(height),
            0.0,
            1.0,
        );
        pass.set_scissor_rect(x, y, width, height);
    }
}

/// Two column-major matrices' product `a * b`, in f64 as the halves' corrections.
pub(crate) fn multiply_clip(a: &[f32; 16], b: &[f32; 16]) -> [f32; 16] {
    narrow(stereoscopy::multiply(a.map(f64::from), b.map(f64::from)))
}

/// The halves' viewports in a target `width` x `height`.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "a rounded share of a target width"
)]
fn viewports((width, height): (u32, u32)) -> [[u32; 4]; 2] {
    let half = ((pixels(width) * HALF_WIDTH).round() as u32).clamp(1, width.max(1));
    [
        [0, 0, half, height],
        [width.saturating_sub(half), 0, half, height],
    ]
}

/// The clip's x widened so a viewport `viewport` pixels wide keeps the target's aspect: the
/// half shows the part of the first player's horizontal field around NDC x `centre`.
fn squeeze(target: u32, viewport: u32, centre: f32) -> [f64; 16] {
    let mut matrix = stereoscopy::IDENTITY;
    matrix[0] = f64::from(target) / f64::from(viewport.max(1));
    matrix[12] = -matrix[0] * f64::from(centre);
    matrix
}

#[expect(
    clippy::cast_possible_truncation,
    reason = "the f64 product narrows to the f32 uniform"
)]
fn narrow(matrix: [f64; 16]) -> [f32; 16] {
    matrix.map(|value| value as f32)
}

#[expect(clippy::cast_precision_loss, reason = "a target dimension")]
fn pixels(value: u32) -> f32 {
    value as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    fn apply(matrix: &[f32; 16], point: [f32; 3]) -> [f32; 4] {
        std::array::from_fn(|row| {
            (0..3).fold(matrix[12 + row], |sum, axis| {
                matrix[axis * 4 + row].mul_add(point[axis], sum)
            })
        })
    }

    /// A perspective looking down -z from `eye`.
    fn camera(eye: [f32; 3]) -> [f32; 16] {
        let (near, far) = (0.1_f32, 100.0_f32);
        let mut clip = [0.0; 16];
        clip[0] = 1.0;
        clip[5] = 16.0 / 9.0;
        clip[10] = far / (near - far);
        clip[11] = -1.0;
        clip[14] = near * far / (near - far);
        clip[12] = -eye[0];
        clip[13] = -eye[1] * clip[5];
        clip[14] += clip[10] * -eye[2];
        clip[15] = eye[2];
        clip
    }

    #[test]
    fn halves_split_the_width_around_a_divider() {
        let [left, right] = viewports((1920, 1080));
        assert_eq!(left, [0, 0, 956, 1080]);
        assert_eq!(right, [964, 0, 956, 1080]);
        assert_eq!(viewports((1, 1)), [[0, 0, 1, 1], [0, 0, 1, 1]]);
    }

    #[test]
    fn the_right_half_sees_the_second_player_where_the_left_sees_the_first() {
        let first_camera = [10.0, 2.0, 30.0];
        let clip = camera(first_camera);
        // A pure translation: the second player 40 m along x of the first.
        let mut world_remap = [0.0; 16];
        for index in [0, 5, 10, 15] {
            world_remap[index] = 1.0;
        }
        world_remap[12] = -40.0;
        let view = SplitScreenView {
            world_remap,
            camera: [first_camera[0] + 40.0, first_camera[1], first_camera[2]],
            axes: [
                [1.0, 0.0, 0.0, 0.0],
                [0.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
            ],
            centre: 0.0,
        };
        let [left, right] = SplitPass::pair(clip, &view, (1920, 1080), [0.1, 100.0]).unwrap();
        assert_eq!(left.half, Half::Left);
        assert_eq!(right.view.map(|(camera, _)| camera), Some(view.camera));
        let first_target = [10.5, 1.0, 20.0];
        let second_target = [50.5, 1.0, 20.0];
        let seen_left = apply(&left.eye.clip, first_target);
        let seen_right = apply(&right.eye.clip, second_target);
        for lane in 0..4 {
            assert!(
                (seen_left[lane] - seen_right[lane]).abs() < 1e-3,
                "{seen_left:?} {seen_right:?}"
            );
        }
        // The squeeze doubles x (less the divider) and keeps y.
        let original = apply(&clip, first_target);
        assert!((seen_left[0] - original[0] * 1920.0 / 956.0).abs() < 1e-4);
        assert!((seen_left[1] - original[1]).abs() < 1e-6);
        // The correction takes a first-player clip-from-world to the second half's.
        let corrected = right.eye.transform(clip);
        for (a, b) in corrected.iter().zip(right.eye.clip) {
            assert!((a - b).abs() < 1e-3);
        }
        // A lens shift centres both halves on that window x: a point there lands mid-half.
        let shifted = SplitScreenView {
            centre: original[0] / original[3],
            ..view
        };
        let [left, right] = SplitPass::pair(clip, &shifted, (1920, 1080), [0.1, 100.0]).unwrap();
        for seen in [
            apply(&left.eye.clip, first_target),
            apply(&right.eye.clip, second_target),
        ] {
            assert!((seen[0] / seen[3]).abs() < 1e-5, "{seen:?}");
            assert!((seen[1] - original[1]).abs() < 1e-5);
        }
    }
}
