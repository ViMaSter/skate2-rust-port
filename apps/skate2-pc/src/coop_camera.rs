//! PC extension, not in the Xbox 360 reference, which follows one local skater with one camera
//! rig: the split screen's second view (`docs/SPLIT_SCREEN_MULTIPLAYER.md`). The second player's
//! half shows the first player's rig carried to the second player: the first camera's offset
//! from the first player's hips, turned about the vertical from the first camera's ground
//! heading to the second player's travel heading, around the second player's hips. The hips are
//! the frame's drawn ones, so both halves move with the presentation's restore between ticks;
//! the heading eases toward the second player's root motion once per completed tick.
//!
//! Both halves also share a lens shift: the full view frames the first player off its centre
//! column, which a half's narrower field would double toward its edge, so each half centres on
//! half that eased screen offset and keeps the skater where the full view's composition has it.

use crate::gpu_renderer::SplitScreenView;
use skate2_core::{SelectedCoopPlayer2, presentation_restore::camera::CameraView};

/// Root travel per tick, in metres, below which the heading holds.
const MOVING: f32 = 0.015;
/// The share of the remaining turn the heading takes each tick.
const FOLLOW: f32 = 0.06;
/// Root travel per tick, in metres, past which the second player was placed (a respawn or a
/// regroup) and the heading starts again from the first camera's.
const PLACED: f32 = 5.0;
/// Ground distance, in metres, under which a heading is undefined.
const LEVEL: f32 = 1e-3;
/// The share of the remaining lens shift the halves take each tick.
const LENS_FOLLOW: f32 = 0.1;

/// The second player's camera heading and the halves' lens shift, kept across ticks.
#[derive(Default)]
pub(crate) struct CoopCamera {
    /// Yaw about +y, `atan2(x, z)`.
    heading: Option<f32>,
    root: Option<[f32; 3]>,
    /// The first player's hips off the first camera's axis: their right over forward distance.
    lens: Option<f32>,
}

impl CoopCamera {
    /// One completed tick: none without a co-op player; else the first camera's heading when the
    /// second player appears or was placed, then easing toward their travel.
    pub(crate) fn tick(&mut self, coop: Option<&SelectedCoopPlayer2>, camera: &CameraView) {
        let Some(coop) = coop else {
            *self = Self::default();
            return;
        };
        if let Some(target) = lens_tangent(camera, coop.player1_hips) {
            self.lens = Some(
                self.lens
                    .map_or(target, |lens| LENS_FOLLOW.mul_add(target - lens, lens)),
            );
        }
        let first = first_heading(camera, coop.player1_hips);
        let previous = self.root.replace(coop.root);
        let travel = previous.map(|previous| sub(coop.root, previous));
        match (self.heading, travel) {
            (Some(heading), Some(travel)) if length(travel) <= PLACED => {
                let ground = travel[0].hypot(travel[2]);
                if ground > MOVING {
                    let target = travel[0].atan2(travel[2]);
                    self.heading = Some(wrap(FOLLOW.mul_add(wrap(target - heading), heading)));
                }
            }
            _ => self.heading = first.or(self.heading),
        }
    }

    /// The second half's view under the first camera `camera` this frame, whose window
    /// clip-from-world is `clip_from_world`, with the first and second players' drawn hips.
    pub(crate) fn view(
        &self,
        camera: &CameraView,
        clip_from_world: &[f32; 16],
        [first, second]: [[f32; 3]; 2],
    ) -> Option<SplitScreenView> {
        let turn = self.heading? - first_heading(camera, first)?;
        let (sin, cos) = turn.sin_cos();
        let rotate = |v: [f32; 3]| {
            [
                cos.mul_add(v[0], sin * v[2]),
                v[1],
                (-sin).mul_add(v[0], cos * v[2]),
            ]
        };
        let unrotate = |v: [f32; 3]| {
            [
                cos.mul_add(v[0], -sin * v[2]),
                v[1],
                sin.mul_add(v[0], cos * v[2]),
            ]
        };
        let position = add(second, rotate(sub(camera.position, first)));
        let axes = [camera.right, camera.up, camera.forward].map(|axis| {
            let [x, y, z] = rotate(axis);
            [x, y, z, 0.0]
        });
        // The second player's hips unturned onto the first player's: T(a1) R⁻¹ T(-a2).
        let shift = sub(first, unrotate(second));
        #[rustfmt::skip]
        let world_remap = [
            cos, 0.0, sin, 0.0,
            0.0, 1.0, 0.0, 0.0,
            -sin, 0.0, cos, 0.0,
            shift[0], shift[1], shift[2], 1.0,
        ];
        let centre = self.lens.map_or(0.0, |lens| {
            let along = add(
                camera.position,
                add(camera.forward, scale(camera.right, lens)),
            );
            let [x, _, _, w] = project(clip_from_world, along);
            if w > LEVEL {
                0.5 * (x / w).clamp(-1.0, 1.0)
            } else {
                0.0
            }
        });
        Some(SplitScreenView {
            world_remap,
            camera: position,
            axes,
            centre,
        })
    }
}

/// The first player's hips off the first camera's axis, right over forward, while in front.
fn lens_tangent(camera: &CameraView, hips: [f32; 3]) -> Option<f32> {
    let offset = sub(hips, camera.position);
    let forward = dot(offset, camera.forward);
    (forward > LEVEL).then(|| dot(offset, camera.right) / forward)
}

/// A column-major clip-from-world applied to a point.
fn project(clip: &[f32; 16], point: [f32; 3]) -> [f32; 4] {
    std::array::from_fn(|row| {
        (0..3).fold(clip[12 + row], |sum, axis| {
            clip[axis * 4 + row].mul_add(point[axis], sum)
        })
    })
}

/// The first camera's ground heading toward the first player's hips, else along its forward.
fn first_heading(camera: &CameraView, hips: [f32; 3]) -> Option<f32> {
    [sub(hips, camera.position), camera.forward]
        .into_iter()
        .find(|direction| direction[0].hypot(direction[2]) > LEVEL)
        .map(|direction| direction[0].atan2(direction[2]))
}

fn wrap(angle: f32) -> f32 {
    use std::f32::consts::{PI, TAU};
    (angle + PI).rem_euclid(TAU) - PI
}

fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn add(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

fn scale(v: [f32; 3], by: f32) -> [f32; 3] {
    [v[0] * by, v[1] * by, v[2] * by]
}

fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0].mul_add(b[0], a[1].mul_add(b[1], a[2] * b[2]))
}

fn length(v: [f32; 3]) -> f32 {
    dot(v, v).sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;
    use skate2_core::two_player_session::CoopScoreboard;

    fn camera(position: [f32; 3], forward: [f32; 3]) -> CameraView {
        CameraView {
            clip_from_world: [0.0; 16],
            position,
            right: [forward[2], 0.0, -forward[0]],
            up: [0.0, 1.0, 0.0],
            forward,
            planes: [0.1, 1000.0],
            update_counter: 0,
        }
    }

    fn coop(player1_root: [f32; 3], root: [f32; 3]) -> SelectedCoopPlayer2 {
        SelectedCoopPlayer2 {
            player1_root,
            player1_hips: player1_root,
            root,
            hips: root,
            scoreboard: CoopScoreboard::default(),
        }
    }

    fn apply(matrix: &[f32; 16], point: [f32; 3]) -> [f32; 3] {
        std::array::from_fn(|row| {
            (0..3).fold(matrix[12 + row], |sum, axis| {
                matrix[axis * 4 + row].mul_add(point[axis], sum)
            })
        })
    }

    fn close(a: [f32; 3], b: [f32; 3]) -> bool {
        length(sub(a, b)) < 1e-3
    }

    #[test]
    fn the_second_view_starts_as_the_first_carried_to_the_second_player() {
        // The first camera 4 m behind (-z) and 2 m over the first player, looking along +z.
        let first = camera([0.0, 2.0, -4.0], [0.0, 0.0, 1.0]);
        let players = coop([0.0, 0.0, 0.0], [30.0, 1.0, 10.0]);
        let mut rig = CoopCamera::default();
        rig.tick(Some(&players), &first);
        let view = rig
            .view(
                &first,
                &first.clip_from_world,
                [players.player1_hips, players.hips],
            )
            .unwrap();
        assert!(close(view.camera, [30.0, 3.0, 6.0]), "{:?}", view.camera);
        assert!(close(apply(&view.world_remap, view.camera), first.position));
        assert!(close(
            apply(&view.world_remap, players.root),
            players.player1_root
        ));
    }

    #[test]
    fn the_heading_eases_toward_the_second_players_travel() {
        let first = camera([0.0, 2.0, -4.0], [0.0, 0.0, 1.0]);
        let mut rig = CoopCamera::default();
        let mut root = [30.0, 0.0, 10.0];
        rig.tick(Some(&coop([0.0; 3], root)), &first);
        // Skating along +x for four seconds.
        for _ in 0..240 {
            root[0] += 0.1;
            rig.tick(Some(&coop([0.0; 3], root)), &first);
        }
        let heading = rig.heading.unwrap();
        assert!(
            (heading - std::f32::consts::FRAC_PI_2).abs() < 1e-2,
            "{heading}"
        );
        let players = coop([0.0; 3], root);
        let view = rig
            .view(
                &first,
                &first.clip_from_world,
                [players.player1_hips, players.hips],
            )
            .unwrap();
        // Behind the second player is now -x, the forward axis +x.
        assert!(
            close(view.camera, [root[0] - 4.0, 2.0, root[2]]),
            "{:?}",
            view.camera
        );
        assert!(close(
            [view.axes[2][0], view.axes[2][1], view.axes[2][2]],
            [1.0, 0.0, 0.0]
        ));
        assert!(close(apply(&view.world_remap, view.camera), first.position));
        // Standing still holds it; a respawn far away restarts from the first camera's.
        rig.tick(Some(&players), &first);
        assert!((rig.heading.unwrap() - heading).abs() < 1e-6);
        rig.tick(Some(&coop([0.0; 3], [500.0, 0.0, 0.0])), &first);
        assert!(rig.heading.unwrap().abs() < 1e-6);
        rig.tick(None, &first);
        assert!(rig.heading.is_none());
    }

    #[test]
    fn both_halves_centre_on_half_the_first_players_eased_screen_offset() {
        // Looking along +z from 4 m behind the origin with a unit-focal clip: x/w is x/z.
        let mut first = camera([0.0, 0.0, -4.0], [0.0, 0.0, 1.0]);
        first.right = [1.0, 0.0, 0.0];
        let mut clip = [0.0; 16];
        clip[0] = 1.0;
        clip[5] = 1.0;
        clip[11] = 1.0;
        clip[15] = 4.0;
        // The first player 1 m right of the axis: x/w = 0.25 on the full view.
        let mut players = coop([1.0, 0.0, 0.0], [30.0, 0.0, 10.0]);
        let mut rig = CoopCamera::default();
        rig.tick(Some(&players), &first);
        let view = rig
            .view(&first, &clip, [players.player1_hips, players.hips])
            .unwrap();
        assert!((view.centre - 0.125).abs() < 1e-5, "{}", view.centre);
        // Moving back onto the axis eases the shift rather than jumping.
        players.player1_hips = [0.0, 0.0, 0.0];
        rig.tick(Some(&players), &first);
        let view = rig
            .view(&first, &clip, [players.player1_hips, players.hips])
            .unwrap();
        assert!((view.centre - 0.9 * 0.125).abs() < 1e-5, "{}", view.centre);
        // Behind the camera the shift holds.
        players.player1_hips = [0.0, 0.0, -10.0];
        rig.tick(Some(&players), &first);
        let held = rig
            .view(&first, &clip, [players.player1_hips, players.hips])
            .unwrap();
        assert!((held.centre - view.centre).abs() < 1e-6);
    }

    #[test]
    fn headings_wrap_into_a_half_turn_either_way() {
        use std::f32::consts::PI;
        assert!((wrap(1.5 * PI) + 0.5 * PI).abs() < 1e-5);
        assert!((wrap(-1.5 * PI) - 0.5 * PI).abs() < 1e-5);
        assert!((wrap(0.25) - 0.25).abs() < 1e-6);
    }
}
