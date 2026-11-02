// Horizon Physics, in the renderer: water that waves and plants, leaves and vines that sway.
//
// Horizon's terrain is meshed once per chunk, so nothing on the CPU can move it every frame. A
// shader pack's terrain pipelines (Horizon's own packs, `gfx/native_material.rs`) get their
// vertex work from a short piece of WGSL around the pack's `material`; the bundle's hook adds a
// displacement there. Which vertex is what comes from its sprite layer: the game side hands this
// module the block atlas's sprite names (`set`), and the WGSL gets a table of those layers.
// Without a Horizon shader pack nothing here runs.

use std::fmt::Write as _;
use std::sync::Mutex;

/// What a sprite does: 0 nothing.
pub const LEAVES: u8 = 1;
/// A plant standing on the ground, its foot still (grass, flowers, crops, saplings).
pub const PLANT: u8 = 2;
/// The top half of a tall plant (its foot is the bottom half's top).
pub const PLANT_TOP: u8 = 3;
pub const VINES: u8 = 4;
pub const WATER: u8 = 5;
pub const SEAGRASS: u8 = 6;
pub const SEAGRASS_TOP: u8 = 7;
/// Stacked underwater plants (kelp): the whole column sways together.
pub const KELP: u8 = 8;
pub const LAVA: u8 = 9;

const FLOWERS: &[&str] = &[
    "dandelion", "poppy", "blue_orchid", "allium", "azure_bluet", "red_tulip", "orange_tulip",
    "white_tulip", "pink_tulip", "oxeye_daisy", "cornflower", "lily_of_the_valley", "wither_rose",
    "torchflower", "closed_eyeblossom", "open_eyeblossom", "short_grass", "fern", "dead_bush",
    "bush", "short_dry_grass", "tall_dry_grass", "firefly_bush",
    "sweet_berry_bush_stage0", "sweet_berry_bush_stage1", "sweet_berry_bush_stage2",
    "sweet_berry_bush_stage3", "nether_sprouts", "warped_roots", "crimson_roots",
];
const DOUBLES: &[&str] = &["tall_grass", "large_fern", "lilac", "rose_bush", "peony", "sunflower"];
const HANGING: &[&str] = &[
    "vine", "weeping_vines", "weeping_vines_plant", "twisting_vines", "twisting_vines_plant",
    "cave_vines", "cave_vines_lit", "cave_vines_plant", "cave_vines_plant_lit", "hanging_roots",
    "pale_hanging_moss", "pale_hanging_moss_tip", "spore_blossom",
];

/// The kind of the block atlas sprite `name` (`minecraft:block/oak_leaves`), 0 for none.
pub fn kind(name: &str) -> u8 {
    let Some(n) = name.strip_prefix("minecraft:block/").or_else(|| name.strip_prefix("block/")) else {
        return 0;
    };
    if n.ends_with("_leaves") {
        return LEAVES;
    }
    if FLOWERS.contains(&n) || n.ends_with("_sapling") || n.ends_with("_propagule") {
        return PLANT;
    }
    for crop in ["wheat_stage", "carrots_stage", "potatoes_stage", "beetroots_stage"] {
        if n.starts_with(crop) {
            return PLANT;
        }
    }
    for d in DOUBLES {
        if let Some(rest) = n.strip_prefix(d) {
            match rest {
                "_bottom" => return PLANT,
                "_top" | "_front" => return PLANT_TOP,
                _ => {}
            }
        }
    }
    if HANGING.contains(&n) {
        return VINES;
    }
    match n {
        "water_still" | "water_flow" => WATER,
        "lava_still" | "lava_flow" => LAVA,
        "seagrass" | "tall_seagrass_bottom" => SEAGRASS,
        "tall_seagrass_top" => SEAGRASS_TOP,
        "kelp" | "kelp_plant" => KELP,
        _ => 0,
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Sway {
    /// Which resources the table was made from (the game's render assets).
    pub key: usize,
    /// (layer, kind), sorted by layer.
    pub layers: Vec<(u32, u8)>,
    pub waves: bool,
    pub plants: bool,
    /// The wind setting: 0 calm .. 2.4 strong.
    pub wind: f32,
}

static SWAY: Mutex<Option<Sway>> = Mutex::new(None);

/// The game side's state: the resources `key` with their sprite layers (only read when `key`
/// changed), and the settings. Returns true when the shader must be built again.
pub fn set<'a>(key: usize, sprites: impl FnOnce() -> Vec<(&'a str, u32)>, waves: bool, plants: bool, wind: f32) -> bool {
    let mut g = SWAY.lock().unwrap_or_else(|e| e.into_inner());
    let layers = match g.as_ref() {
        Some(s) if s.key == key => s.layers.clone(),
        _ => {
            let mut l: Vec<(u32, u8)> = sprites().into_iter().filter_map(|(n, l)| match kind(n) {
                0 => None,
                k => Some((l, k)),
            }).collect();
            l.sort_unstable();
            l.dedup_by_key(|x| x.0);
            l
        }
    };
    let new = Sway { key, layers, waves, plants, wind };
    let changed = g.as_ref() != Some(&new);
    *g = Some(new);
    changed
}

pub fn current() -> Option<Sway> {
    SWAY.lock().ok().and_then(|g| g.clone())
}

/// The WGSL for `s`: `horizon_physics_sway(pos, uv, layer)`, the displacement of a terrain
/// vertex (camera-relative `pos`, its UV inside its sprite), reading the pack's `frame`.
pub fn wgsl(s: &Sway) -> Option<String> {
    let wanted = |k: u8| if k == WATER || k == LAVA { s.waves } else { s.plants };
    let mut by_kind: [Vec<u32>; 10] = Default::default();
    for &(l, k) in &s.layers {
        if wanted(k) && (k as usize) < by_kind.len() {
            by_kind[k as usize].push(l);
        }
    }
    if by_kind.iter().all(|v| v.is_empty()) {
        return None;
    }
    let mut w = String::new();
    w.push_str("fn horizon_physics_kind(layer: u32) -> u32 {\n    switch (layer & 0x7fffu) {\n");
    for (k, layers) in by_kind.iter().enumerate() {
        if layers.is_empty() {
            continue;
        }
        let cases: Vec<String> = layers.iter().map(|l| format!("{l}u")).collect();
        let _ = writeln!(w, "        case {}: {{ return {k}u; }}", cases.join(", "));
    }
    w.push_str("        default: { return 0u; }\n    }\n}\n");
    let wind = s.wind.max(0.0);
    let _ = write!(
        w,
        "
const HORIZON_PHYSICS_WIND: f32 = {wind:.3};
// A smooth wobble in two directions at a world point, for time t.
fn horizon_physics_wobble(p: vec3<f32>, t: f32, rate: f32) -> vec3<f32> {{
    let a = dot(p.xz, vec2(0.37, 0.23)) + p.y * 0.19;
    return vec3(sin(t * rate + a) + 0.4 * sin(t * rate * 2.3 + a * 1.7),
                0.25 * sin(t * rate * 1.3 + a * 0.7),
                cos(t * rate * 0.9 + a * 1.1) + 0.4 * cos(t * rate * 1.9 + a * 2.1));
}}
fn horizon_physics_sway(pos: vec3<f32>, uv: vec2<f32>, layer: u32) -> vec3<f32> {{
    let kind = horizon_physics_kind(layer);
    if (kind == 0u) {{ return vec3(0.0); }}
    // Only close by: far away it would not show, and it stops at the same distance for all.
    let near = 1.0 - smoothstep(40.0, 56.0, length(pos));
    if (near <= 0.0) {{ return vec3(0.0); }}
    let world = pos + frame.camera_position;
    let t = frame.time;
    // Wind: the setting, more in rain and thunder, in gusts.
    let gust = 0.65 + 0.35 * sin(t * 0.41 + world.x * 0.021 + world.z * 0.017);
    let wind = (0.45 + HORIZON_PHYSICS_WIND) * (1.0 + frame.rain * 0.7 + frame.thunder * 0.6) * gust;
    // How high up the sprite the vertex is (its foot 0).
    let h = clamp(1.0 - uv.y, 0.0, 1.0);
    var off = vec3(0.0);
    switch (kind) {{
        case 1u: {{ off = horizon_physics_wobble(world, t, 1.7) * 0.022 * wind; }}
        case 2u: {{ off = horizon_physics_wobble(vec3(floor(world.x), 0.0, floor(world.z)), t, 2.1) * vec3(1.0, 0.0, 1.0) * 0.07 * wind * h * h; }}
        case 3u: {{ off = horizon_physics_wobble(vec3(floor(world.x), 0.0, floor(world.z)), t, 2.1) * vec3(1.0, 0.0, 1.0) * 0.07 * wind * (1.0 + h) * (1.0 + h); }}
        case 4u: {{ off = horizon_physics_wobble(world, t, 1.4) * vec3(1.0, 0.0, 1.0) * 0.03 * wind; }}
        case 5u, 9u: {{
            // Only the surface: corners at whole blocks (the bed, full water) stay.
            let f = fract(world.y);
            if (f > 0.02 && f < 0.98) {{
                let slow = select(1.0, 0.35, kind == 9u);
                let tt = t * slow;
                let wave = sin(world.x * 0.9 + tt * 1.3) * 0.5 + sin(world.z * 1.13 - tt * 1.7) * 0.35
                    + sin((world.x + world.z) * 2.3 + tt * 2.9) * 0.15;
                off = vec3(0.0, wave * 0.035 * (0.6 + 0.4 * wind) * slow, 0.0);
            }}
        }}
        case 6u: {{ off = horizon_physics_wobble(vec3(floor(world.x), 0.0, floor(world.z)), t, 1.1) * vec3(1.0, 0.0, 1.0) * 0.1 * h * h; }}
        case 7u: {{ off = horizon_physics_wobble(vec3(floor(world.x), 0.0, floor(world.z)), t, 1.1) * vec3(1.0, 0.0, 1.0) * 0.1 * (1.0 + h) * (1.0 + h); }}
        case 8u: {{ off = horizon_physics_wobble(vec3(world.x, world.y * 0.3, world.z), t, 1.0) * vec3(1.0, 0.0, 1.0) * 0.07; }}
        default: {{}}
    }}
    return off * near;
}}
"
    );
    Some(w)
}

/// The material stage's vertex WGSL (`ENTRIES`) with the displacement, or None when nothing
/// sways or the text is not the one this was written for.
pub fn patch(entries: &str) -> Option<String> {
    const AT: &str = "let pos = position + origin;";
    if entries.matches(AT).count() != 1 {
        return None;
    }
    let s = current()?;
    let w = wgsl(&s)?;
    let patched = entries.replace(AT, "let pos = position + origin + horizon_physics_sway(position + origin, uv, layer);");
    Some(format!("{patched}\n// Horizon Physics: waves and swaying plants\n{w}"))
}
