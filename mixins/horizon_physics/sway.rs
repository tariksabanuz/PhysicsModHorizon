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
    MARKS.store(waves, std::sync::atomic::Ordering::Relaxed);
    let new = Sway { key, layers, waves, plants, wind };
    let changed = g.as_ref() != Some(&new);
    *g = Some(new);
    changed
}

pub fn current() -> Option<Sway> {
    SWAY.lock().ok().and_then(|g| g.clone())
}

/// What the built-in pack's material holds, so the patch knows the water shading is ours to do
/// (another pack's water is that pack's business: it only gets the waves).
pub const BUILTIN_MARK: &str = "HORIZON_PHYSICS_BUILTIN";

/// The built-in pack (`horizon-shaders.toml`, `material.wgsl`): vanilla's look, so the mod's
/// waves and sway run without a shader pack picked.
pub const BUILTIN_MANIFEST: &str = "schema = 1
id = \"horizon_physics_vanilla\"
name = \"Horizon Physics (vanilla look)\"
version = \"1.0.0\"
api = \"0.1\"
description = \"Vanilla's look, for Horizon Physics' waves, water and swaying plants.\"

[material]
file = \"material.wgsl\"
";

pub const BUILTIN_MATERIAL: &str = "// HORIZON_PHYSICS_BUILTIN: vanilla's material, albedo * tint * lightmap.
fn material(input: MaterialInput) -> Surface {
    return Surface(input.albedo * input.tint * vec4<f32>(input.lightmap, 1.0));
}
";

/// The WGSL for `s`: `horizon_physics_sway(pos, uv, layer)`, the displacement of a terrain
/// vertex (camera-relative `pos`, its UV inside its sprite), and `horizon_physics_surface`, the
/// water's look (only when `shade`: the built-in pack), both reading the pack's `frame`.
pub fn wgsl(s: &Sway) -> Option<String> {
    wgsl_with(s, false)
}

pub fn wgsl_with(s: &Sway, shade: bool) -> Option<String> {
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
    let _ = write!(w, "const HORIZON_PHYSICS_WIND: f32 = {wind:.3};\nconst HORIZON_PHYSICS_SHADE: bool = {shade};\n");
    w.push_str(&WGSL_BODY.replace("//SWELL//", &swell_wgsl()));
    Some(w)
}

/// The open sea's swell: (direction x, direction z, wavelength, amplitude, phase, pace).
pub const OCEAN: [[f64; 6]; 5] = [
    [0.83, 0.56, 34.0, 0.5, 0.0, 0.42],
    [0.45, 0.89, 21.0, 0.3, 1.7, 0.42],
    [0.97, -0.24, 13.0, 0.17, 3.4, 0.45],
    [0.62, 0.78, 8.5, 0.09, 5.1, 0.5],
    [-0.2, 0.98, 5.5, 0.045, 6.8, 0.5],
];
/// A lake's or river's lapping.
pub const LAKE: [[f64; 6]; 4] = [
    [0.83, 0.56, 14.0, 0.13, 0.0, 0.55],
    [0.39, 0.92, 9.0, 0.09, 1.7, 0.55],
    [0.97, -0.24, 6.4, 0.055, 3.4, 0.58],
    [0.60, 0.80, 4.2, 0.03, 5.1, 0.6],
];
/// How much of the swell moves the sea's surface sideways (Gerstner's steepness).
pub const CHOP: f64 = 0.35;

fn swell_wgsl() -> String {
    let calls = |name: &str, table: &[[f64; 6]], what: &str| {
        let mut f = format!("fn {name}(p: vec2<f32>, t: f32) -> {what} {{\n    var r = {what}(0.0);\n");
        for w in table {
            let call = if what == "vec3<f32>" { "horizon_physics_wave" } else { "horizon_physics_chop_wave" };
            let _ = writeln!(f, "    r += {call}(vec4({:?}, {:?}, {:?}, {:?}), p, t, {:?}, {:?});", w[0] as f32, w[1] as f32, w[2] as f32, w[3] as f32, w[4] as f32, w[5] as f32);
        }
        f.push_str("    return r;\n}\n");
        f
    };
    let mut out = String::new();
    out.push_str(&calls("horizon_physics_ocean", &OCEAN, "vec3<f32>"));
    out.push_str(&calls("horizon_physics_lake", &LAKE, "vec3<f32>"));
    out.push_str("// Gerstner's sideways motion of the sea's surface: the water gathers under the crests, which\n// turn sharp, and spreads in the troughs.\nfn horizon_physics_chop_wave(w: vec4<f32>, p: vec2<f32>, t: f32, seed: f32, pace: f32) -> vec2<f32> {\n    let k = 6.2831853 / w.z;\n    let ph = k * dot(w.xy, p) - sqrt(9.8 * k) * pace * t + seed;\n    return -w.xy * (w.w * cos(ph));\n}\n");
    out.push_str(&calls("horizon_physics_chop_sum", &OCEAN, "vec2<f32>"));
    let _ = writeln!(out, "fn horizon_physics_chop(p: vec2<f32>, t: f32, open: f32) -> vec2<f32> {{\n    return horizon_physics_chop_sum(p, t) * ({:?} * horizon_physics_storm() * horizon_physics_sea(open));\n}}", CHOP as f32);
    out
}

fn smoothstep(a: f64, b: f64, x: f64) -> f64 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// The water's clock the shader uses, from the world's game time (ticks) and the partial tick.
pub fn water_time(game_time: i64, partial: f64) -> f64 {
    let f = ((game_time % 24000) as f32 + partial as f32) / 24000.0;
    f64::from(f) * 1200.0
}

/// The water surface's movement at world point (x, z), time `t` (`water_time`), openness `open`
/// (0..1) and wind setting `wind`, as the shader moves it: (up, sideways x, sideways z, dh/dx,
/// dh/dz), before the distance fade.
pub fn water_motion(x: f64, z: f64, t: f64, open: f64, wind: f64) -> [f64; 5] {
    let storm = (0.42 + wind * 0.3).clamp(0.15, 1.6);
    let sea = smoothstep(0.5, 0.9, open);
    let lake = (open * 3.0).clamp(0.0, 1.0) * (1.0 - sea);
    let mut r = [0.0; 5];
    for (table, k) in [(&OCEAN[..], sea), (&LAKE[..], lake)] {
        if k <= 0.0 {
            continue;
        }
        for w in table {
            let kk = std::f64::consts::TAU / w[2];
            let ph = kk * (w[0] * x + w[1] * z) - (9.8 * kk).sqrt() * w[5] * t + w[4];
            let c = w[3] * kk * ph.cos();
            r[0] += w[3] * ph.sin() * k * storm;
            r[3] += c * w[0] * k * storm;
            r[4] += c * w[1] * k * storm;
        }
    }
    for w in &OCEAN {
        let kk = std::f64::consts::TAU / w[2];
        let ph = kk * (w[0] * x + w[1] * z) - (9.8 * kk).sqrt() * w[5] * t + w[4];
        let s = -w[3] * ph.cos() * CHOP * storm * sea;
        r[1] += w[0] * s;
        r[2] += w[1] * s;
    }
    r[0] -= 0.04;
    r
}

/// The step of the mesher's mark, and the height vanilla gives a still surface's corners.
pub const MARK_STEP: f32 = 0.0001;
pub const STILL_TOP: f32 = 0.888_888_9 - 0.001;

/// How open the water is at a surface corner: the corner at section-relative (`cx`, `cz`) of
/// surface cells at height `y` (`water` says which cells hold water, the section's border
/// included). 0 at the shore (a corner touching anything but surface water), then by depth: 1
/// for a puddle up to 7 for deep water, at most 3 within two blocks of a shore.
pub fn water_code(cx: i32, y: i32, cz: i32, water: impl Fn(i32, i32, i32) -> bool, bottom: i32) -> u32 {
    let surface = |x: i32, z: i32| water(x, y, z) && !water(x, y + 1, z);
    let mut depth = u32::MAX;
    for (dx, dz) in [(-1, -1), (-1, 0), (0, -1), (0, 0)] {
        let (x, z) = (cx + dx, cz + dz);
        if !surface(x, z) {
            return 0;
        }
        let mut d = 0u32;
        while y - d as i32 >= bottom && water(x, y - d as i32, z) {
            d += 1;
        }
        depth = depth.min(d);
    }
    let mut code = match depth {
        0..=1 => 1,
        2 => 2,
        3 => 3,
        4 | 5 => 4,
        6..=8 => 5,
        9..=12 => 6,
        _ => 7,
    };
    for dx in -2..=1 {
        for dz in -2..=1 {
            if (-1..=0).contains(&dx) && (-1..=0).contains(&dz) {
                continue;
            }
            if !water(cx + dx, y, cz + dz) {
                code = code.min(3);
            }
        }
    }
    code
}

/// How far out the shader moves the water (`horizon_physics_reach`), at `dist` blocks.
pub fn water_reach(dist: f64) -> f64 {
    1.0 - smoothstep(72.0, 104.0, dist)
}

/// The wind setting the shader was built with.
pub fn water_wind() -> f64 {
    current().map_or(1.0, |s| f64::from(s.wind))
}

/// Whether the waves are on in the shader.
pub fn waves_on() -> bool {
    current().is_some_and(|s| s.waves)
}

/// Whether the mesher marks water corners (on unless the waves are off; the marks are far too
/// small to see, so marking a surface that will not wave costs nothing).
static MARKS: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(true);

pub fn marks_water() -> bool {
    MARKS.load(std::sync::atomic::Ordering::Relaxed)
}

thread_local! {
    static IN_FLUID: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// The fluid hook's guard against itself (it calls the function it hooks): true once.
pub fn fluid_enter() -> bool {
    IN_FLUID.with(|c| !c.replace(true))
}

pub fn fluid_leave() {
    IN_FLUID.with(|c| c.set(false));
}

const WGSL_BODY: &str = r#"
// One wave of the swell: direction (x, z), wavelength and amplitude in blocks; deep-water
// dispersion (omega = sqrt(g k)) gives each its own speed, slowed to a calm Minecraft sea.
// Returns (height, dh/dx, dh/dz).
fn horizon_physics_wave(w: vec4<f32>, p: vec2<f32>, t: f32, seed: f32, pace: f32) -> vec3<f32> {
    let k = 6.2831853 / w.z;
    let omega = sqrt(9.8 * k) * pace;
    let ph = k * dot(w.xy, p) - omega * t + seed;
    let c = w.w * k * cos(ph);
    return vec3(w.w * sin(ph), c * w.x, c * w.y);
}
// The water's clock: the world's time, so the game can follow the same waves (a boat on them).
fn horizon_physics_water_time() -> f32 {
    return frame.game_time * 1200.0;
}
// How rough the water is: the wind setting.
fn horizon_physics_storm() -> f32 {
    return clamp(0.42 + HORIZON_PHYSICS_WIND * 0.3, 0.15, 1.6);
}
//SWELL//
// The water at world point p (x, z) whose openness is `open` (0 at the shore or in a puddle, 1
// on deep open water): how much of the sea's swell and of a lake's lapping it has.
fn horizon_physics_sea(open: f32) -> f32 {
    return smoothstep(0.5, 0.9, open);
}
fn horizon_physics_swell(p: vec2<f32>, t: f32, open: f32) -> vec3<f32> {
    let sea = horizon_physics_sea(open);
    let lake = clamp(open * 3.0, 0.0, 1.0) * (1.0 - sea);
    var r = vec3(0.0);
    if (sea > 0.0) { r += horizon_physics_ocean(p, t) * sea; }
    if (lake > 0.0) { r += horizon_physics_lake(p, t) * lake; }
    return r * horizon_physics_storm();
}
// The mesher's mark on a water surface's corner (Horizon Physics' fluid hook): its height raised
// by `code` ten-thousandths of a block over vanilla's 8/9 - 0.001, code 0..7 its openness.
// Unmarked (another shape, flowing water): 0, still.
fn horizon_physics_open(position: vec3<f32>, layer: u32) -> f32 {
    if (horizon_physics_kind(layer) != 5u) { return 0.0; }
    let d = (fract(position.y) - 0.8878889) / 0.0001;
    if (d < -0.45 || d > 7.45) { return 0.0; }
    return round(d) / 7.0;
}
// How far out the water moves: all of it close by, none past the shape's reach.
fn horizon_physics_reach(dist: f32) -> f32 {
    return 1.0 - smoothstep(72.0, 104.0, dist);
}
// Ripples: only in the surface's light, never in its shape. Returns the slope.
fn horizon_physics_ripples(p: vec2<f32>, t: f32) -> vec2<f32> {
    var r = horizon_physics_wave(vec4(0.71, 0.70, 1.9, 0.012), p, t, 0.0, 0.7);
    r += horizon_physics_wave(vec4(-0.45, 0.89, 1.3, 0.008), p, t, 2.3, 0.7);
    r += horizon_physics_wave(vec4(0.95, 0.31, 0.95, 0.006), p, t, 4.6, 0.7);
    r += horizon_physics_wave(vec4(0.18, -0.98, 0.7, 0.004), p, t, 6.9, 0.7);
    r += horizon_physics_wave(vec4(-0.86, -0.51, 0.52, 0.003), p, t, 9.2, 0.7);
    r += horizon_physics_wave(vec4(0.55, -0.83, 0.37, 0.002), p, t, 11.5, 0.7);
    return r.yz * (0.6 + 0.6 * horizon_physics_storm());
}
// The wind over the land: a direction that turns slowly, gusts that travel across fields.
fn horizon_physics_wind_dir(t: f32) -> vec2<f32> {
    let a = 0.7 + t * 0.013 + 0.6 * sin(t * 0.071);
    return vec2(cos(a), sin(a));
}
fn horizon_physics_wind(world: vec3<f32>, t: f32) -> f32 {
    let dir = horizon_physics_wind_dir(t);
    // Along the wind, a gust is a band that sweeps over the grass.
    let along = dot(world.xz, dir);
    let gust = 0.55 + 0.3 * sin(along * 0.11 - t * 0.9) + 0.15 * sin(along * 0.37 - t * 1.7 + world.y * 0.2);
    let strength = 0.25 + 0.45 * HORIZON_PHYSICS_WIND;
    return max(gust, 0.05) * strength;
}
// A flutter in two directions at a world point (each plant its own phase).
fn horizon_physics_flutter(p: vec3<f32>, t: f32, rate: f32) -> vec2<f32> {
    let a = dot(p.xz, vec2(0.37, 0.23)) * 6.0 + p.y * 1.9;
    return vec2(sin(t * rate + a) + 0.35 * sin(t * rate * 2.7 + a * 1.3),
                cos(t * rate * 0.83 + a * 1.1) + 0.35 * cos(t * rate * 2.1 + a * 0.7));
}
// How a plant leans at height h (0 its foot, 1 its top) of one block: in the wind, plus flutter.
fn horizon_physics_lean(root: vec3<f32>, t: f32, h: f32, stiff: f32) -> vec3<f32> {
    let dir = horizon_physics_wind_dir(t);
    let w = horizon_physics_wind(root, t);
    let f = horizon_physics_flutter(root, t, 1.1 + 0.6 * w);
    let lean = (dir * (0.5 + 0.5 * w) * w + f * (0.12 + 0.2 * w)) * stiff;
    // A bent stalk: the top moves most, and comes down a little as it bends.
    let x = lean * h * h;
    return vec3(x.x, -dot(x, x) * 0.35, x.y);
}
fn horizon_physics_sway(pos: vec3<f32>, raw: vec3<f32>, uv: vec2<f32>, layer: u32) -> vec3<f32> {
    let kind = horizon_physics_kind(layer);
    if (kind == 0u) { return vec3(0.0); }
    let dist = length(pos);
    // Only close by: far away it would not show, and it stops at the same distance for all.
    let near = select(1.0 - smoothstep(48.0, 64.0, dist), horizon_physics_reach(dist), kind == 5u);
    if (near <= 0.0) { return vec3(0.0); }
    let world = pos + frame.camera_position;
    let t = frame.time;
    // How high up the sprite the vertex is (its foot 0).
    let h = clamp(1.0 - uv.y, 0.0, 1.0);
    let root = vec3(floor(world.x) + 0.5, floor(world.y), floor(world.z) + 0.5);
    var off = vec3(0.0);
    switch (kind) {
        case 1u: {
            // Leaves: every corner rustles on its own, more in the wind.
            let w = horizon_physics_wind(world, t);
            let f = horizon_physics_flutter(world * 1.7, t, 1.4 + w * 0.8);
            let dir = horizon_physics_wind_dir(t);
            off = vec3(f.x * 0.016 + dir.x * 0.02 * w, f.y * 0.01, f.y * 0.016 + dir.y * 0.02 * w) * (0.5 + w);
        }
        case 2u: { off = horizon_physics_lean(root, t, h, 0.16); }
        case 3u: {
            // The top half of a tall plant: its foot is where the bottom half's top went.
            let below = vec3(root.x, root.y - 1.0, root.z);
            off = horizon_physics_lean(below, t, (1.0 + h) * 0.5, 0.16) * 4.0;
        }
        case 4u: {
            // Hanging vines: a slow swing, its phase running down the vine, and the wind.
            let dir = horizon_physics_wind_dir(t);
            let w = horizon_physics_wind(world, t);
            let ph = dot(floor(world.xz), vec2(0.71, 0.53)) - world.y * 0.45;
            let swing = vec2(sin(t * 0.7 + ph), cos(t * 0.55 + ph * 1.3)) * 0.045;
            let x = swing * (0.6 + 0.6 * w) + dir * 0.05 * w;
            off = vec3(x.x, 0.0, x.y);
        }
        case 5u, 9u: {
            // Only the surface: corners at whole blocks (the bed, full water) stay.
            let f = fract(world.y);
            if (f > 0.02 && f < 0.98) {
                if (kind == 5u) {
                    // As open as the mesher marked it: the shore and puddles stay put.
                    let open = horizon_physics_open(raw, layer);
                    if (open > 0.0) {
                        let tw = horizon_physics_water_time();
                        let s = horizon_physics_swell(world.xz, tw, open);
                        let side = horizon_physics_chop(world.xz, tw, open);
                        off = vec3(side.x, s.x - 0.04, side.y);
                    }
                } else {
                    // Lava: a slow, heavy heave.
                    let tt = t * 0.3;
                    let wave = sin(world.x * 0.7 + tt * 1.3) * 0.5 + sin(world.z * 0.9 - tt * 1.7) * 0.35;
                    off = vec3(0.0, wave * 0.04, 0.0);
                }
            }
        }
        case 6u: {
            // Seagrass: the current, slow and smooth.
            let f = horizon_physics_flutter(root, t * 0.45, 1.2);
            off = vec3(f.x, 0.0, f.y) * 0.13 * h * h;
        }
        case 7u: {
            let below = vec3(root.x, root.y - 1.0, root.z);
            let f = horizon_physics_flutter(below, t * 0.45, 1.2);
            let hh = (1.0 + h) * 0.5;
            off = vec3(f.x, 0.0, f.y) * 0.52 * hh * hh;
        }
        case 8u: {
            // Kelp: the whole stalk in the current, more higher up.
            let f = horizon_physics_flutter(vec3(root.x, 0.0, root.z), t * 0.4, 1.0);
            let bend = sin(t * 0.6 + world.y * 0.35 + dot(root.xz, vec2(0.3, 0.7)));
            off = vec3(f.x + bend * 0.5, 0.0, f.y - bend * 0.3) * 0.08;
        }
        default: {}
    }
    return off * near;
}
// The water's surface, lit as water: the swell and ripples bend the light, the sky is mirrored
// more at a glancing angle (Fresnel), the sun glitters on it, light shines through the crests.
// `color` is the material's; `lightmap` vanilla's light colour here.
fn horizon_physics_surface(layer: u32, pos: vec3<f32>, light: vec2<f32>, lightmap: vec3<f32>, open: f32, color: vec4<f32>) -> vec4<f32> {
    // The face's own normal, before any branch (derivatives need the whole quad).
    let geo = cross(dpdx(pos), dpdy(pos));
    if (!HORIZON_PHYSICS_SHADE || horizon_physics_kind(layer) != 5u) { return color; }
    let up = abs(geo.y) > 0.6 * length(geo);
    // Seen from under the water: vanilla's look.
    if (!up || pos.y > 0.0) { return color; }
    let world = pos + frame.camera_position;
    let tw = horizon_physics_water_time();
    let dist = length(pos);
    let reach = horizon_physics_reach(dist);
    let sea = horizon_physics_sea(open);
    let s = horizon_physics_swell(world.xz, tw, open);
    let r = horizon_physics_ripples(world.xz, tw);
    // Far away the small waves blur into one.
    let detail = 1.0 - smoothstep(24.0, 110.0, dist);
    let slope = s.yz * max(reach, 0.25) + r * detail * (0.35 + 0.65 * clamp(open * 3.0, 0.0, 1.0));
    let n = normalize(vec3(-slope.x, 1.0, -slope.y));
    let v = normalize(-pos);
    let cosv = clamp(dot(n, v), 0.0, 1.0);
    let fresnel = 0.02 + 0.98 * pow(1.0 - cosv, 5.0);
    let refl = reflect(-v, n);
    // How bright it is here: the light colour of the sky light (dark at night, under trees).
    let bright = max(max(lightmap.r, lightmap.g), lightmap.b);
    let skylight = smoothstep(0.0, 1.0, light.y);
    // The sky it mirrors: the horizon's haze below, the sky's colour above.
    let sky = mix(frame.fog_color, frame.sky_color, clamp(refl.y * 2.5, 0.0, 1.0)) * (0.25 + 0.75 * skylight);
    let sun = normalize(frame.sun_direction);
    let day = smoothstep(-0.05, 0.2, sun.y);
    let rs = max(dot(refl, sun), 0.0);
    let glitter = (pow(rs, 900.0) * 9.0 + pow(rs, 120.0) * 0.9 + pow(rs, 18.0) * 0.08) * day * skylight;
    let moon = pow(max(dot(refl, normalize(frame.moon_direction)), 0.0), 400.0) * 1.5 * (1.0 - day) * skylight;
    let height = clamp(s.x / (0.75 * horizon_physics_storm()), -1.0, 1.0);
    var body = color.rgb * (0.9 + 0.12 * height);
    if (sea > 0.0) {
        // The open sea's own colour: deep blue-green in the troughs, clear teal where the light
        // comes through a crest from behind (towards the sun).
        let deep = vec3(0.01, 0.055, 0.09);
        let shallow = vec3(0.04, 0.27, 0.3);
        let tint = color.rgb / max(max(color.r, max(color.g, color.b)), 0.001);
        var sea_rgb = mix(deep, shallow, 0.45 + 0.45 * height);
        let behind = pow(clamp(dot(-v, vec3(sun.x, 0.0, sun.z)) * 0.5 + 0.5, 0.0, 1.0), 2.0);
        let through = smoothstep(0.1, 0.9, height) * behind * day * 0.35;
        sea_rgb += vec3(0.08, 0.42, 0.4) * through;
        sea_rgb *= mix(vec3(1.0), tint, 0.25);
        // The sun lights the slopes that face it: the waves' shapes show.
        let lit = 0.6 + 0.6 * max(dot(n, sun), 0.0) * day + 0.15 * (1.0 - day);
        body = mix(body, sea_rgb * (0.35 + 0.65 * bright) * 1.35 * lit, 0.9 * sea);
    }
    var rgb = mix(body, sky, clamp(fresnel * mix(0.9, 0.75, sea), 0.0, 1.0));
    rgb += vec3(1.0, 0.93, 0.78) * glitter + vec3(0.75, 0.82, 1.0) * moon;
    // Froth on the highest crests of open water, more in strong wind.
    let foam = smoothstep(0.82, 1.0, height) * smoothstep(0.5, 1.2, horizon_physics_storm()) * 0.45 * sea;
    rgb = mix(rgb, vec3(0.92, 0.95, 0.97) * (0.3 + 0.7 * skylight), foam);
    let opaque = mix(color.a, 0.92, sea);
    let a = clamp(mix(opaque, 1.0, fresnel) + glitter + foam, 0.0, 1.0);
    return vec4(rgb, a);
}
"#;

/// The material stage's WGSL (`ENTRIES`) with the displacement and, for the built-in pack
/// (`source` its stage), the water's look; None when nothing sways or the text is not the one
/// this was written for.
pub fn patch(entries: &str, source: &str) -> Option<String> {
    const AT: &str = "let pos = position + origin;";
    const FIELD: &str = "    @location(7) cylindrical: f32,\n}";
    const SET: &str = "    out.cylindrical = max(length(pos.xz), abs(pos.y));";
    const COLOR: &str = "return material(MaterialInput(";
    const COLOR_END: &str = ")).color;";
    for anchor in [AT, FIELD, SET] {
        if entries.matches(anchor).count() != 1 {
            return None;
        }
    }
    let s = current()?;
    let builtin = source.contains(BUILTIN_MARK);
    let shade = builtin && entries.matches(COLOR).count() == 1 && entries.matches(COLOR_END).count() == 1;
    let w = wgsl_with(&s, shade)?;
    let mut patched = entries
        .replace(AT, "let pos = position + origin + horizon_physics_sway(position + origin, position, uv, layer);")
        .replace(FIELD, "    @location(7) cylindrical: f32,\n    @location(8) hp_open: f32,\n}")
        .replace(SET, &format!("{SET}\n    out.hp_open = horizon_physics_open(position, layer);"));
    if shade {
        patched = patched
            .replace(COLOR, "return horizon_physics_surface(in.layer, in.position, in.light, in.lightmap, in.hp_open, material(MaterialInput(")
            .replace(COLOR_END, ")).color);");
    }
    Some(format!("{patched}\n// Horizon Physics: waves, water and swaying plants\n{w}"))
}

// ---------------------------------------------------------------------------------------------
// Snow that gives way (PhysicsMod's snow terrain deformation): where the player and the mobs
// walk, the snow's top is pressed down into a trough with a lip of pushed-up snow along it. The
// game side presses (`snow_press`) into a height map kept per block (`SNOW_GRID` cells a side);
// the sections whose blocks changed are meshed again (`snow_take_dirty`), and the mesher hook
// swaps each pressed block's flat top for a fine grid at those heights (`snow_top`), shaded like
// the game shades its faces. The prints stay as long as the blocks are loaded, up to `SNOW_MAX`
// blocks (the oldest go first).

/// Cells along a block's side.
pub const SNOW_GRID: usize = 8;
const SNOW_PTS: usize = SNOW_GRID + 1;
/// Pressed blocks kept at most.
pub const SNOW_MAX: usize = 3000;

/// One pressed block: how far each grid point is pressed down (negative: pushed up), its snow's
/// top (in the block, 0..1) and when it was last pressed.
#[derive(Clone, Debug)]
pub struct SnowDent {
    pub depth: [f32; SNOW_PTS * SNOW_PTS],
    pub top: f32,
    stamp: u64,
}

#[derive(Default)]
struct Snow {
    world: u64,
    min_y: i32,
    blocks: std::collections::HashMap<(i32, i32, i32), SnowDent>,
    dirty: std::collections::HashSet<(i32, i32, i32)>,
    clock: u64,
}

static SNOW: Mutex<Option<Snow>> = Mutex::new(None);

fn snow_section(min_y: i32, b: (i32, i32, i32)) -> (i32, i32, i32) {
    (b.0 >> 4, (b.1 - min_y) >> 4, b.2 >> 4)
}

/// The world the prints belong to (`key`: the dimension, `min_y` its bottom). Another world
/// forgets them.
pub fn snow_world(key: u64, min_y: i32) {
    let mut g = SNOW.lock().unwrap_or_else(|e| e.into_inner());
    let s = g.get_or_insert_with(Snow::default);
    if s.world != key || s.min_y != min_y {
        // The old world's sections are not this world's: nothing to mesh again for them.
        s.blocks.clear();
        s.dirty.clear();
        s.world = key;
        s.min_y = min_y;
    }
}

/// Forgets every print, and meshes their blocks again flat.
pub fn snow_clear() {
    let mut g = SNOW.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(s) = g.as_mut()
        && !s.blocks.is_empty()
    {
        let min_y = s.min_y;
        let sections: Vec<(i32, i32, i32)> = s.blocks.keys().map(|b| snow_section(min_y, *b)).collect();
        s.dirty.extend(sections);
        s.blocks.clear();
    }
}

/// How much a press at (`cx`, `cz`) (radius `r`, depth `d`, drawn out `stretch` times along the
/// unit direction (`fx`, `fz`)) moves the snow at (`x`, `z`): down inside, a lip pushed up just
/// outside, nothing further out.
pub fn snow_profile(x: f64, z: f64, c: [f64; 4], r: f64, d: f64) -> f64 {
    let (cx, cz, fx, fz) = (c[0], c[1], c[2], c[3]);
    let (dx, dz) = (x - cx, z - cz);
    let along = (dx * fx + dz * fz) / 1.25;
    let across = -dx * fz + dz * fx;
    let q = (along * along + across * across).sqrt() / r;
    if q < 0.3 {
        d
    } else if q < 1.0 {
        // A rounded wall from the floor up to the surface.
        let t = (q - 0.3) / 0.7;
        d * (1.0 - t * t * (3.0 - 2.0 * t))
    } else if q < 1.6 {
        // The lip: the snow pushed aside, a little above the surface.
        let t = (q - 1.0) / 0.6;
        -d * 0.18 * (std::f64::consts::PI * t).sin()
    } else {
        0.0
    }
}

/// Presses the snow at `feet` (x, top of the snow, z), a body `width` wide moving along
/// (`fx`, `fz`). `top` gives a block's snow top (its block y plus its height), None for a block
/// that is not snow. Returns whether anything changed.
pub fn snow_press(feet: [f64; 3], width: f64, dir: [f64; 2], top: impl Fn(i32, i32, i32) -> Option<f64>) -> bool {
    let surface = feet[1];
    let by = (surface - 1e-3).floor() as i32;
    let height = surface - f64::from(by);
    if !(0.05..=1.0001).contains(&height) {
        return false;
    }
    // As deep as a foot sinks in vanilla: a layer of snow stands an eighth of a block above what
    // the feet stand on (the thinnest layer: down to almost the ground).
    let deepest = (height - 1.0 / 32.0).min(0.125);
    let depth = deepest;
    if depth <= 0.005 {
        return false;
    }
    let r = (width * 0.75).clamp(0.3, 1.1);
    let len = (dir[0] * dir[0] + dir[1] * dir[1]).sqrt();
    let (fx, fz) = if len > 1e-6 { (dir[0] / len, dir[1] / len) } else { (1.0, 0.0) };
    let shape = [feet[0], feet[2], fx, fz];
    let same = |x: i32, z: i32| top(x, by, z).is_some_and(|t| (t - surface).abs() < 1e-3);
    let reach = r * 1.6 * 1.25 + 0.01;
    let (x0, x1) = ((feet[0] - reach).floor() as i32, (feet[0] + reach).floor() as i32);
    let (z0, z1) = ((feet[2] - reach).floor() as i32, (feet[2] + reach).floor() as i32);
    let mut g = SNOW.lock().unwrap_or_else(|e| e.into_inner());
    let s = g.get_or_insert_with(Snow::default);
    s.clock += 1;
    let clock = s.clock;
    let min_y = s.min_y;
    let mut changed = false;
    for bx in x0..=x1 {
        for bz in z0..=z1 {
            if !same(bx, bz) {
                continue;
            }
            let mut dent: Option<[f32; SNOW_PTS * SNOW_PTS]> = None;
            for i in 0..SNOW_PTS {
                for j in 0..SNOW_PTS {
                    let x = f64::from(bx) + i as f64 / SNOW_GRID as f64;
                    let z = f64::from(bz) + j as f64 / SNOW_GRID as f64;
                    let mut v = snow_profile(x, z, shape, r, depth);
                    if v.abs() < 1e-4 {
                        continue;
                    }
                    // A point on the block's edge is shared with the next block: it moves only
                    // if every block it touches is the same snow, so the surface stays whole.
                    let xs: &[i32] = if i == 0 { &[-1, 0] } else if i == SNOW_GRID { &[0, 1] } else { &[0] };
                    let zs: &[i32] = if j == 0 { &[-1, 0] } else if j == SNOW_GRID { &[0, 1] } else { &[0] };
                    if !xs.iter().all(|dx| zs.iter().all(|dz| same(bx + dx, bz + dz))) {
                        v = 0.0;
                    }
                    if v == 0.0 {
                        continue;
                    }
                    let d = dent.get_or_insert_with(|| {
                        s.blocks.get(&(bx, by, bz)).map_or([0.0; SNOW_PTS * SNOW_PTS], |b| b.depth)
                    });
                    let k = i * SNOW_PTS + j;
                    let old = f64::from(d[k]);
                    // Pressing wins over the lip; a lip only rises where nothing was pressed.
                    let new = if v > 0.0 { old.max(v) } else if old <= 0.0 { old.min(v) } else { old };
                    if (new - old).abs() > 0.004 {
                        d[k] = new as f32;
                        changed = true;
                    }
                }
            }
            if let Some(d) = dent {
                if d.iter().all(|v| v.abs() < 1e-4) {
                    continue;
                }
                let key = (bx, by, bz);
                let fresh = s.blocks.get(&key).is_none_or(|b| b.depth != d);
                s.blocks.insert(key, SnowDent { depth: d, top: height as f32, stamp: clock });
                if fresh {
                    s.dirty.insert(snow_section(min_y, key));
                }
            }
        }
    }
    if s.blocks.len() > SNOW_MAX {
        let mut ages: Vec<((i32, i32, i32), u64)> = s.blocks.iter().map(|(k, b)| (*k, b.stamp)).collect();
        ages.sort_unstable_by_key(|a| a.1);
        for (k, _) in ages.iter().take(s.blocks.len() - SNOW_MAX) {
            s.blocks.remove(k);
            s.dirty.insert(snow_section(min_y, *k));
        }
    }
    changed
}

/// The sections to mesh again (chunk x, section index from the bottom, chunk z).
/// At most ten times a second: a walk through snow would otherwise mesh its sections every frame.
pub fn snow_take_dirty() -> Vec<(i32, i32, i32)> {
    static LAST: Mutex<Option<std::time::Instant>> = Mutex::new(None);
    if let Ok(mut last) = LAST.lock() {
        let now = std::time::Instant::now();
        if last.is_some_and(|t| now.duration_since(t) < std::time::Duration::from_millis(100)) {
            return Vec::new();
        }
        *last = Some(now);
    }
    let mut g = SNOW.lock().unwrap_or_else(|e| e.into_inner());
    g.as_mut().map(|s| s.dirty.drain().collect()).unwrap_or_default()
}

/// The pressed blocks of the section (chunk `cx`, index `sy`, chunk `cz`), each with its
/// section-relative position.
pub fn snow_in_section(cx: i32, sy: i32, cz: i32) -> Vec<([i32; 3], SnowDent)> {
    let g = SNOW.lock().unwrap_or_else(|e| e.into_inner());
    let Some(s) = g.as_ref() else {
        return Vec::new();
    };
    if s.blocks.is_empty() {
        return Vec::new();
    }
    let base = [cx * 16, s.min_y + sy * 16, cz * 16];
    let mut out = Vec::new();
    for (k, d) in &s.blocks {
        if snow_section(s.min_y, *k) == (cx, sy, cz) {
            out.push(([k.0 - base[0], k.1 - base[1], k.2 - base[2]], d.clone()));
        }
    }
    out
}

/// One corner of a flat top: where it is in the block (0 or 1 along x and z) and what the game
/// gave it: its sprite position, light (sky low byte, block high byte) and colour.
#[derive(Clone, Copy, Debug)]
pub struct SnowCorner {
    pub at: [f32; 2],
    pub uv: [f32; 2],
    pub light: u16,
    pub color: u32,
}

/// The pressed top of a block whose flat top had `corners` at height `y` (section-relative), its
/// cells as quads (four corners each, counter-clockwise seen from above): position, sprite
/// position, light, colour. The colour takes the game's face shading by the slope, and the
/// bottom of a print is a little darker, as the light reaches it less.
pub fn snow_top(origin: [f32; 3], y: f32, dent: &SnowDent, corners: &[SnowCorner; 4]) -> Vec<([f32; 3], [f32; 2], u16, u32)> {
    let n = SNOW_GRID;
    let step = 1.0 / n as f32;
    let h = |i: usize, j: usize| -dent.depth[i.min(n) * SNOW_PTS + j.min(n)];
    // Bilinear in the four corners, by where in the block.
    let lerp = |fx: f32, fz: f32, f: &dyn Fn(&SnowCorner) -> f32| -> f32 {
        let mut acc = 0.0;
        for c in corners {
            let wx = if c.at[0] > 0.5 { fx } else { 1.0 - fx };
            let wz = if c.at[1] > 0.5 { fz } else { 1.0 - fz };
            acc += wx * wz * f(c);
        }
        acc
    };
    let byte = |v: u32, s: u32| ((v >> s) & 255) as f32;
    let point = |i: usize, j: usize| -> ([f32; 3], [f32; 2], u16, u32) {
        let (fx, fz) = (i as f32 * step, j as f32 * step);
        let uv = [lerp(fx, fz, &|c| c.uv[0]), lerp(fx, fz, &|c| c.uv[1])];
        let sky = lerp(fx, fz, &|c| f32::from(c.light & 255)).round().clamp(0.0, 255.0) as u16;
        let block = lerp(fx, fz, &|c| f32::from(c.light >> 8)).round().clamp(0.0, 255.0) as u16;
        // The slope: Minecraft's face shading (up 1, north and south 0.8, east and west 0.6).
        let dx = (h(i + 1, j) - h(i.saturating_sub(1), j)) / (step * if i == 0 || i == n { 1.0 } else { 2.0 });
        let dz = (h(i, j + 1) - h(i, j.saturating_sub(1))) / (step * if j == 0 || j == n { 1.0 } else { 2.0 });
        let len = (dx * dx + 1.0 + dz * dz).sqrt();
        let (nx, ny, nz) = (-dx / len, 1.0 / len, -dz / len);
        let shade = ny * ny + nx * nx * 0.6 + nz * nz * 0.8;
        let deep = (dent.depth[i * SNOW_PTS + j]).max(0.0);
        let k = (shade * (1.0 - deep * 0.55)).clamp(0.35, 1.0);
        let mut color = 0u32;
        for s in [0u32, 8, 16] {
            let v = lerp(fx, fz, &|c| byte(c.color, s)) * k;
            color |= (v.round().clamp(0.0, 255.0) as u32) << s;
        }
        color |= corners[0].color & 0xff00_0000;
        let pos = [origin[0] + fx, y + h(i, j), origin[2] + fz];
        (pos, uv, sky | block << 8, color)
    };
    let mut out = Vec::with_capacity(n * n * 4);
    for i in 0..n {
        for j in 0..n {
            out.push(point(i, j));
            out.push(point(i, j + 1));
            out.push(point(i + 1, j + 1));
            out.push(point(i + 1, j));
        }
    }
    out
}
