//! PC extension, not in the Xbox 360 reference: the split screen's co-op labels
//! (`docs/SPLIT_SCREEN_MULTIPLAYER.md`). Each half names the other player over their skater,
//! or at the half's edge toward them, with the players' distance; each half's outer top corner
//! holds its player's co-op score and the team's total sits over the divider. They draw on the
//! window over the scene and under the APT levels, in `helvetica_bold` as `online_names` and in
//! the players' `kOnlineColours`.

use super::assets::{Assets, Font};
use crate::gpu_renderer::{RenderResult, SPLIT_HALF_WIDTH as HALF, marker_ui::Batch};
use skate2_core::{
    texture::NativeTexture,
    two_player_session::{CoopPlayer, CoopScoreboard},
    ui::pause_menu::PauseMenuSnapshot,
};

/// The label's rise over the hips, as `online_names`' `kfSkaterLabelYAdjust`.
const LABEL_RISE: f32 = 1.3;
/// Text cap heights on a 720-line window; larger windows scale them.
const TAG_HEIGHT: f32 = 15.0;
const SCORE_HEIGHT: f32 = 17.0;
/// How far toward a half's edges an off-screen label stays, in the half's NDC.
const EDGE: [f32; 2] = [0.88, 0.82];
/// The panels' backing.
const BACKING: [f32; 4] = [0.0, 0.0, 0.0, 0.45];
/// Where a half's prompt to join a teleport sits, in the half's NDC y.
const PROMPT_Y: f32 = -0.78;

/// What the shared front end and the labels read of the co-op session, while the second player
/// is in the game.
#[derive(Clone, Copy, Debug)]
pub(crate) struct CoopFrontEnd {
    /// The second player's own pause menu.
    pub player2_menu: PauseMenuSnapshot,
    /// Whose half the Challenge Map fills: the player whose pad drives it.
    pub screen_owner: CoopPlayer,
    /// Each player's prompt to join the other's teleport (LB + D-pad right), while their chord
    /// would take it.
    pub join_prompts: [bool; 2],
}

/// One split-screen frame's labels.
pub(crate) struct Frame {
    /// Each half's window clip-from-world before the half's squeeze: the first player's on the
    /// left, the second player's view on the right.
    pub clips: [[f32; 16]; 2],
    /// The window NDC x both halves centre on (`SplitScreenView::centre`).
    pub centre: f32,
    /// The first and second players' hips.
    pub hips: [[f32; 3]; 2],
    pub scoreboard: CoopScoreboard,
    /// Which halves prompt their player to join the other's teleport.
    pub join_prompts: [bool; 2],
}

/// The labels' quads in the window's clip space.
pub(super) fn batches(
    assets: &Assets,
    frame: &Frame,
    size: (u32, u32),
) -> RenderResult<Vec<Batch>> {
    let font = assets.font_by_file(super::online_names::FONT)?;
    let atlas = assets
        .textures
        .get(&font.texture)
        .ok_or("co-op font atlas missing")?;
    let mut pen = Pen {
        font,
        atlas,
        size,
        scale: ui_scale(size),
        text: Vec::new(),
        backing: Vec::new(),
    };
    let distance = frame.hips[0]
        .iter()
        .zip(frame.hips[1])
        .map(|(a, b)| (a - b) * (a - b))
        .sum::<f32>()
        .sqrt();
    for half in 0..2 {
        let other = 1 - half;
        let hips = frame.hips[other];
        let anchor = label_anchor(
            &frame.clips[half],
            frame.centre,
            [hips[0], hips[1] + LABEL_RISE, hips[2]],
        );
        let ([x, y], clamped) = anchor;
        let window = [half_origin(half) + HALF * x, y];
        let name = format!("P{} {distance:.0} m", other + 1);
        let label = match clamped {
            Some(Side::Left) => format!("< {name}"),
            Some(Side::Right) => format!("{name} >"),
            None => name,
        };
        let colour = player_colour(other);
        let height = TAG_HEIGHT * pen.scale;
        let width = pen.width(&label, height);
        let [px, py] = pen.pixels(window);
        pen.panel(
            [px - width * 0.5, py - height * 1.4],
            [width, height],
            colour,
            &label,
        );
    }
    let height = SCORE_HEIGHT * pen.scale;
    let margin = 12.0 * pen.scale;
    for (player, score) in frame.scoreboard.players.iter().enumerate() {
        let line = if score.current_line > 0 {
            format!(
                "P{} {}  +{}",
                player + 1,
                grouped(score.banked),
                grouped(u64::from(score.current_line))
            )
        } else {
            format!("P{} {}", player + 1, grouped(score.banked))
        };
        let width = pen.width(&line, height);
        // The outer corners, clear of the team total over the divider.
        let [left, top] = if player == 0 {
            let [left, top] = pen.pixels([half_origin(0) - HALF, 1.0]);
            [left + margin, top]
        } else {
            let [right, top] = pen.pixels([half_origin(1) + HALF, 1.0]);
            [right - margin - width, top]
        };
        pen.panel(
            [left, top + margin],
            [width, height],
            player_colour(player),
            &line,
        );
    }
    let team = format!("TEAM {}", grouped(frame.scoreboard.team_total()));
    let width = pen.width(&team, height);
    let [centre, top] = pen.pixels([0.0, 1.0]);
    pen.panel(
        [centre - width * 0.5, top + margin],
        [width, height],
        [1.0; 4],
        &team,
    );
    // The offer to join the other player's teleport, which LB + D-pad right takes.
    for half in (0..2).filter(|&half| frame.join_prompts[half]) {
        let teleporter = 1 - half;
        let prompt = format!("LB + RIGHT: join P{}'s teleport", teleporter + 1);
        let width = pen.width(&prompt, height);
        let [centre, row] = pen.pixels([half_origin(half), PROMPT_Y]);
        pen.panel(
            [centre - width * 0.5, row],
            [width, height],
            player_colour(teleporter),
            &prompt,
        );
    }
    let mut batches = Vec::new();
    if !pen.backing.is_empty() {
        batches.push(Batch {
            texture: None,
            wrapped: false,
            vertices: pen.backing,
        });
    }
    if !pen.text.is_empty() {
        batches.push(Batch {
            texture: Some(font.texture.clone()),
            wrapped: false,
            vertices: pen.text,
        });
    }
    Ok(batches)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Side {
    Left,
    Right,
}

/// A half's centre in window NDC x.
fn half_origin(half: usize) -> f32 {
    if half == 0 { HALF - 1.0 } else { 1.0 - HALF }
}

/// The window's text scale against a 720-line, 16:9 window.
#[expect(clippy::cast_precision_loss, reason = "window dimensions")]
fn ui_scale((width, height): (u32, u32)) -> f32 {
    let aspect = width as f32 / height.max(1) as f32;
    height as f32 / 720.0 * (aspect / (16.0 / 9.0)).min(1.0)
}

/// Where a world point labels in a half: the half's NDC, kept inside `EDGE`, and the side it
/// was pushed to when it lies outside the half or behind the camera.
fn label_anchor(
    clip_from_world: &[f32; 16],
    centre: f32,
    point: [f32; 3],
) -> ([f32; 2], Option<Side>) {
    let clip = super::online_names::project(clip_from_world, point);
    let (x, y) = if clip[3] > 1e-4 {
        ((clip[0] / clip[3] - centre) / HALF, clip[1] / clip[3])
    } else {
        // Behind the camera: the bottom corner on the point's side.
        let side = if clip[0] >= 0.0 { EDGE[0] } else { -EDGE[0] };
        (side * 2.0, -EDGE[1] * 2.0)
    };
    let reach = (x.abs() / EDGE[0]).max(y.abs() / EDGE[1]);
    if reach <= 1.0 {
        return ([x, y], None);
    }
    let side = if x < 0.0 { Side::Left } else { Side::Right };
    ([x / reach, y / reach], Some(side))
}

fn player_colour(player: usize) -> [f32; 4] {
    let [red, green, blue] = super::online_names::colour(u8::try_from(player).unwrap_or(u8::MAX));
    [red, green, blue, 1.0]
}

/// `value` with thousands separators.
fn grouped(value: u64) -> String {
    let digits = value.to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (index, digit) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index) % 3 == 0 {
            out.push(',');
        }
        out.push(digit);
    }
    out
}

/// Text and its backing in window pixels, emitted in window clip space.
struct Pen<'a> {
    font: &'a Font,
    atlas: &'a NativeTexture,
    size: (u32, u32),
    scale: f32,
    text: Vec<[f32; 10]>,
    backing: Vec<[f32; 10]>,
}

impl Pen<'_> {
    #[expect(clippy::cast_precision_loss, reason = "window dimensions")]
    fn pixels(&self, [x, y]: [f32; 2]) -> [f32; 2] {
        [
            f32::midpoint(x, 1.0) * self.size.0 as f32,
            (1.0 - y) * 0.5 * self.size.1 as f32,
        ]
    }

    #[expect(clippy::cast_precision_loss, reason = "window dimensions")]
    fn clip(&self, [x, y]: [f32; 2]) -> [f32; 2] {
        [
            x * 2.0 / self.size.0.max(1) as f32 - 1.0,
            1.0 - y * 2.0 / self.size.1.max(1) as f32,
        ]
    }

    /// The glyph scale for a cap height, as `fps_batch` sizes its counter.
    fn glyph_scale(&self, height: f32) -> Option<[f32; 2]> {
        let cap = self.font.metrics.mapped_glyph(u16::from(b'P'))?;
        let sy = height / cap.height;
        Some([sy * self.font.scale[0] / self.font.scale[1], sy])
    }

    fn width(&self, text: &str, height: f32) -> f32 {
        let Some(scale) = self.glyph_scale(height) else {
            return 0.0;
        };
        text.encode_utf16()
            .filter_map(|unit| self.font.metrics.mapped_glyph(unit))
            .map(|glyph| glyph.advance * scale[0])
            .sum()
    }

    /// `text` with its cap's top-left at `at`, over a backing.
    fn panel(&mut self, at: [f32; 2], [width, height]: [f32; 2], colour: [f32; 4], text: &str) {
        let pad = height * 0.35;
        self.quad(
            [
                at[0] - pad,
                at[1] - pad,
                at[0] + width + pad,
                at[1] + height + pad,
            ],
            [0.0; 4],
            BACKING,
            false,
        );
        let Some(scale) = self.glyph_scale(height) else {
            return;
        };
        let Some(cap) = self.font.metrics.mapped_glyph(u16::from(b'P')) else {
            return;
        };
        let origin = [
            cap.x_offset.mul_add(-scale[0], at[0]),
            (cap.y_offset + self.font.metrics.ascent).mul_add(-scale[1], at[1]),
        ];
        let units: Vec<u16> = text.encode_utf16().collect();
        let quads: Vec<_> =
            super::glyph_quads(self.font, self.atlas, &units, scale, origin).collect();
        for (bounds, uv) in quads {
            self.quad(bounds, uv, colour, true);
        }
    }

    fn quad(&mut self, bounds: [f32; 4], uv: [f32; 4], colour: [f32; 4], text: bool) {
        let [left, top] = self.clip([bounds[0], bounds[1]]);
        let [right, bottom] = self.clip([bounds[2], bounds[3]]);
        let corners = [
            [left, top, uv[0], uv[1]],
            [right, top, uv[2], uv[1]],
            [right, bottom, uv[2], uv[3]],
            [left, bottom, uv[0], uv[3]],
        ];
        let out = if text {
            &mut self.text
        } else {
            &mut self.backing
        };
        out.extend([0, 1, 2, 0, 2, 3].map(|index| {
            let [x, y, u, v] = corners[index];
            [
                x, y, 0.0, 1.0, u, v, colour[0], colour[1], colour[2], colour[3],
            ]
        }));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scores_group_their_thousands() {
        assert_eq!(grouped(0), "0");
        assert_eq!(grouped(999), "999");
        assert_eq!(grouped(1000), "1,000");
        assert_eq!(grouped(1_234_567), "1,234,567");
    }

    #[test]
    fn labels_inside_a_half_stay_and_others_keep_to_its_edge() {
        let mut identity = [0.0; 16];
        for index in [0, 5, 10, 15] {
            identity[index] = 1.0;
        }
        let ([x, y], side) = label_anchor(&identity, 0.0, [0.1, 0.2, 0.5]);
        assert!(side.is_none());
        assert!((x - 0.1 / HALF).abs() < 1e-6 && (y - 0.2).abs() < 1e-6);
        // The halves' lens shift moves the point with the geometry.
        let ([x, _], side) = label_anchor(&identity, 0.1, [0.1, 0.2, 0.5]);
        assert!(side.is_none() && x.abs() < 1e-6);
        let ([x, _], side) = label_anchor(&identity, 0.0, [0.9, 0.0, 0.5]);
        assert_eq!(side, Some(Side::Right));
        assert!((x - EDGE[0]).abs() < 1e-6);
        // w comes from the identity's last row, 1: a point behind needs a perspective clip.
        let mut behind = identity;
        behind[15] = 0.0;
        behind[11] = 1.0;
        let (_, side) = label_anchor(&behind, 0.0, [-0.5, 0.0, -1.0]);
        assert_eq!(side, Some(Side::Left));
    }

    #[test]
    fn each_half_centres_on_its_side_of_the_divider() {
        // The left half spans [-1, 2 HALF - 1] and the right [1 - 2 HALF, 1].
        assert!((half_origin(0) - HALF + 1.0).abs() < 1e-6);
        assert!((half_origin(1) + HALF - 1.0).abs() < 1e-6);
        assert!(half_origin(0) + HALF < half_origin(1) - HALF);
    }
}
