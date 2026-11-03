// Horizon Physics' quick setup, drawn on the title screen the first time the game starts with the
// mod (and again when asked for in the mod's settings): a page each for mob deaths, broken blocks,
// weather and collapsing structures, each choice a live preview of itself. The layout, the pages
// and the previews are the runtime's (`setup_ui`, `SetupScene`); this module draws them with the
// HUD's own quads and text.
use crate::hud::{HudAtlas, HudBatch, rgba};
use crate::native_horizon_physics as hp;

fn color(c: [u8; 4]) -> u32 {
    rgba(c[0], c[1], c[2], c[3])
}

fn unit(v: f32) -> u8 {
    (v.clamp(0.0, 1.0) * 255.0).round() as u8
}

/// Draws the quick setup over the whole screen, the pointer at `cursor`.
pub fn draw(atlas: &HudAtlas, batch: &mut HudBatch, cursor: (f32, f32)) {
    let (w, h) = (batch.width.ceil(), batch.height.ceil());
    let white = atlas.font.white();
    let flat = [(white.u0 + white.u1) * 0.5, (white.v0 + white.v1) * 0.5];
    let sprites: Vec<Option<[f32; 4]>> = hp::SETUP_SPRITES
        .iter()
        .map(|n| if n.is_empty() { None } else { atlas.sprites.get(*n).map(|r| [r.u0, r.v0, r.u1, r.v1]) })
        .collect();
    hp::with_setup(|s| {
        for item in hp::setup_ui(s.page, w, h, cursor) {
            match item {
                hp::SetupUi::Rect { x, y, w, h, color: c } => batch.fill(atlas, x, y, w, h, color(c)),
                hp::SetupUi::Gradient { x, y, w, h, top, bottom } => batch.fill_gradient(atlas, (x, y, w, h), color(top), color(bottom)),
                hp::SetupUi::Text { text, x, y, color: c } => {
                    let tw = HudBatch::text_width(atlas, &text);
                    batch.text(atlas, &text, (x - tw * 0.5).round(), y, color(c), true);
                }
                hp::SetupUi::Scene { index, x, y, w: sw, h: sh } => {
                    let Some(scene) = s.scenes.get(index) else {
                        continue;
                    };
                    let mut quads = scene.quads(x + sw * 0.5, y + sh * 0.5, sw, sh);
                    // The solid ones first, then the see-through ones (shadows, rain) over them.
                    quads.sort_by_key(|q| q.color[3] < 1.0);
                    let start = batch.vertices.len();
                    for q in quads {
                        let uv = match sprites.get(usize::from(q.sprite)).copied().flatten() {
                            Some(r) => q.uv.map(|t| [r[0] + (r[2] - r[0]) * t[0], r[1] + (r[3] - r[1]) * t[1]]),
                            None => [flat; 4],
                        };
                        let c = rgba(unit(q.color[0]), unit(q.color[1]), unit(q.color[2]), unit(q.color[3]));
                        batch.quad_3d(q.p, uv, c);
                    }
                    batch.depth_since(start, [x, y, x + sw, y + sh]);
                }
            }
        }
    });
}
