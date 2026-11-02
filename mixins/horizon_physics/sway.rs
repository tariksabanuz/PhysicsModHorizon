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
    w.push_str(WGSL_BODY);
    Some(w)
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
// How rough the water is: the wind setting, more in rain, most in thunder.
fn horizon_physics_storm() -> f32 {
    return clamp(0.42 + HORIZON_PHYSICS_WIND * 0.3 + frame.rain * 0.35 + frame.thunder * 0.55, 0.15, 1.6);
}
// The swell at world point p (x, z): long waves carry most of the height, short ones ride on them.
fn horizon_physics_swell(p: vec2<f32>, t: f32) -> vec3<f32> {
    var r = horizon_physics_wave(vec4(0.83, 0.56, 15.0, 0.15), p, t, 0.0, 0.55);
    r += horizon_physics_wave(vec4(0.39, 0.92, 9.5, 0.095), p, t, 1.7, 0.55);
    r += horizon_physics_wave(vec4(0.97, -0.24, 6.8, 0.06), p, t, 3.4, 0.55);
    r += horizon_physics_wave(vec4(0.60, 0.80, 4.4, 0.035), p, t, 5.1, 0.55);
    r += horizon_physics_wave(vec4(-0.20, 0.98, 3.1, 0.02), p, t, 6.8, 0.55);
    return r * horizon_physics_storm();
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
    let strength = (0.25 + 0.45 * HORIZON_PHYSICS_WIND) * (1.0 + frame.rain * 0.6 + frame.thunder * 0.8);
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
fn horizon_physics_sway(pos: vec3<f32>, uv: vec2<f32>, layer: u32) -> vec3<f32> {
    let kind = horizon_physics_kind(layer);
    if (kind == 0u) { return vec3(0.0); }
    let dist = length(pos);
    // Only close by: far away it would not show, and it stops at the same distance for all.
    let near = 1.0 - smoothstep(48.0, 64.0, dist);
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
                    // The swell, around a level a little under the still surface.
                    let s = horizon_physics_swell(world.xz, t);
                    off = vec3(0.0, s.x - 0.04, 0.0);
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
// more at a glancing angle (Fresnel), the sun glitters on it. `color` is the material's.
fn horizon_physics_surface(layer: u32, pos: vec3<f32>, light: vec2<f32>, color: vec4<f32>) -> vec4<f32> {
    // The face's own normal, before any branch (derivatives need the whole quad).
    let geo = cross(dpdx(pos), dpdy(pos));
    if (!HORIZON_PHYSICS_SHADE || horizon_physics_kind(layer) != 5u) { return color; }
    let up = abs(geo.y) > 0.6 * length(geo);
    if (!up || frame.eye_in == 1u) { return color; }
    let world = pos + frame.camera_position;
    let t = frame.time;
    let s = horizon_physics_swell(world.xz, t);
    let r = horizon_physics_ripples(world.xz, t);
    let dist = length(pos);
    // Far away the small waves blur into one.
    let detail = 1.0 - smoothstep(24.0, 90.0, dist);
    let slope = s.yz + r * detail;
    var n = normalize(vec3(-slope.x, 1.0, -slope.y));
    let v = normalize(-pos);
    let cosv = clamp(dot(n, v), 0.0, 1.0);
    let fresnel = 0.02 + 0.98 * pow(1.0 - cosv, 5.0);
    let refl = reflect(-v, n);
    // The sky it mirrors: the horizon's haze below, the sky's colour above.
    let skylight = smoothstep(0.0, 1.0, light.y);
    let sky = mix(frame.fog_color, frame.sky_color, clamp(refl.y * 2.5, 0.0, 1.0)) * (0.25 + 0.75 * skylight);
    let sun = normalize(frame.sun_direction);
    let day = smoothstep(-0.05, 0.2, sun.y);
    let rs = max(dot(refl, sun), 0.0);
    let glitter = (pow(rs, 900.0) * 9.0 + pow(rs, 120.0) * 0.9 + pow(rs, 18.0) * 0.08) * day * skylight * (1.0 - frame.rain * 0.92);
    let moon = pow(max(dot(refl, normalize(frame.moon_direction)), 0.0), 400.0) * 1.5 * (1.0 - day) * skylight;
    // Body colour: the material's, a little deeper in the troughs and lighter on the crests.
    let crest = clamp(s.x / (0.32 * horizon_physics_storm()), -1.0, 1.0);
    var body = color.rgb * (0.88 + 0.16 * crest);
    var rgb = mix(body, sky, clamp(fresnel * 0.9, 0.0, 1.0));
    rgb += vec3(1.0, 0.93, 0.78) * glitter + vec3(0.75, 0.82, 1.0) * moon;
    // A thin froth on the highest crests in rough weather.
    let foam = smoothstep(0.78, 1.0, crest) * smoothstep(0.7, 1.3, horizon_physics_storm()) * 0.35;
    rgb = mix(rgb, vec3(0.92, 0.95, 0.97) * (0.3 + 0.7 * skylight), foam);
    let a = clamp(mix(color.a, 1.0, fresnel) + glitter + foam, 0.0, 1.0);
    return vec4(rgb, a);
}
"#;

/// The material stage's WGSL (`ENTRIES`) with the displacement and, for the built-in pack
/// (`source` its stage), the water's look; None when nothing sways or the text is not the one
/// this was written for.
pub fn patch(entries: &str, source: &str) -> Option<String> {
    const AT: &str = "let pos = position + origin;";
    const COLOR: &str = "return material(MaterialInput(";
    const COLOR_END: &str = ")).color;";
    if entries.matches(AT).count() != 1 {
        return None;
    }
    let s = current()?;
    let builtin = source.contains(BUILTIN_MARK);
    let shade = builtin && entries.matches(COLOR).count() == 1 && entries.matches(COLOR_END).count() == 1;
    let w = wgsl_with(&s, shade)?;
    let mut patched = entries.replace(AT, "let pos = position + origin + horizon_physics_sway(position + origin, uv, layer);");
    if shade {
        patched = patched
            .replace(COLOR, "return horizon_physics_surface(in.layer, in.position, in.light, material(MaterialInput(")
            .replace(COLOR_END, ")).color);");
    } else {
        // Called by nobody, but present so the text is the same shape either way.
        patched.push('\n');
    }
    Some(format!("{patched}\n// Horizon Physics: waves, water and swaying plants\n{w}"))
}
