// Horizon Physics: crumbling blocks, ragdolls, mobs that fall apart and item physics.
// Written from scratch for Horizon. Nothing here touches gameplay: every piece is a client-side
// decoration, drawn with the entity pipeline and simulated with a small rigid-body solver
// (position-based dynamics with substeps) against the blocks of the world.
//
// The hooks of the bundle call into this module:
// - a block breaking hands over its sprite and shape (`spawn_block`);
// - a mob's first frame of dying is drawn into a buffer, cut into its model parts and turned into
//   bodies joined at the joints (`capture_*`, `spawn_mob`);
// - once a frame the world is stepped and drawn (`frame`);
// - dropped items ask for their orientation (`item_rotation`).

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::mesher::Vertex;

pub const ID: &str = "horizon_physics";
pub const NAME: &str = "Horizon Physics";
pub const VERSION: &str = "0.3.0";
pub const DESCRIPTION: &str =
    "Blocks crumble and blow apart, ragdolls, items, smoke, ropes, capes, liquids, waves and swaying plants.";

// ---------------------------------------------------------------------------------------------
// Settings
// ---------------------------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Settings {
    pub blocks: bool,
    /// 0 off, 1 ragdoll, 2 fall apart, 3 mixed.
    pub deaths: u8,
    pub items: bool,
    /// Blocks blown apart, fireballs, sparks and smoke.
    pub explosions: bool,
    /// Smoke rising from fire, campfires and lava.
    pub smoke: bool,
    /// Footprints in snow.
    pub snow: bool,
    /// Leads and fishing lines that sag and swing.
    pub ropes: bool,
    /// Capes as cloth.
    pub capes: bool,
    /// Water that waves (needs a Horizon shader pack).
    pub waves: bool,
    /// Leaves, plants, vines and seagrass that sway (needs a Horizon shader pack).
    pub plants: bool,
    /// Placed water and lava splash as droplets; splashes when you fall in (beta).
    pub liquids: bool,
    /// 0 calm, 1 breeze, 2 strong.
    pub wind: u8,
    /// 0 Earth, 1 Moon, 2 Mars, 3 Pluto, 4 zero.
    pub gravity: u8,
    /// 0 few, 1 normal, 2 many.
    pub pieces: u8,
    /// 0 10 s, 1 30 s, 2 60 s, 3 5 min.
    pub lifetime: u8,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            blocks: true,
            deaths: 1,
            items: true,
            explosions: true,
            smoke: true,
            snow: true,
            ropes: true,
            capes: true,
            waves: true,
            plants: true,
            liquids: true,
            wind: 1,
            gravity: 0,
            pieces: 1,
            lifetime: 1,
        }
    }
}

const DEATHS: [&str; 4] = ["Off", "Ragdoll", "Fall apart", "Mixed"];
const GRAVITY: [(&str, f64); 5] =
    [("Earth", 1.0), ("Moon", 0.165), ("Mars", 0.38), ("Pluto", 0.063), ("Zero", 0.0)];
const PIECES: [(&str, usize); 3] = [("Few", 4), ("Normal", 8), ("Many", 16)];
const LIFETIME: [(&str, f64); 4] = [("10 s", 10.0), ("30 s", 30.0), ("60 s", 60.0), ("5 min", 300.0)];
const WIND: [(&str, f64); 3] = [("Calm", 0.0), ("Breeze", 1.0), ("Strong", 2.4)];
/// Earth's pull on the pieces, in blocks per second squared. Real gravity (9.81) looks slow at
/// the scale of blocks; twice that reads as a real fall.
const EARTH: f64 = 20.0;

fn on_off(b: bool) -> &'static str {
    if b { "On" } else { "Off" }
}

impl Settings {
    pub fn labels(&self) -> Vec<String> {
        vec![
            format!("Crumbling blocks: {}", on_off(self.blocks)),
            format!("Mob deaths: {}", DEATHS[self.deaths as usize % 4]),
            format!("Item physics: {}", on_off(self.items)),
            format!("Explosions: {}", on_off(self.explosions)),
            format!("Smoke and fire: {}", on_off(self.smoke)),
            format!("Snow footprints: {}", on_off(self.snow)),
            format!("Leads and fishing lines: {}", on_off(self.ropes)),
            format!("Cloth capes: {}", on_off(self.capes)),
            format!("Water waves (shader pack): {}", on_off(self.waves)),
            format!("Swaying plants (shader pack): {}", on_off(self.plants)),
            format!("Liquids (beta): {}", on_off(self.liquids)),
            format!("Wind: {}", WIND[self.wind as usize % 3].0),
            format!("Gravity: {}", GRAVITY[self.gravity as usize % 5].0),
            format!("Pieces per block: {}", PIECES[self.pieces as usize % 3].0),
            format!("Pieces stay for: {}", LIFETIME[self.lifetime as usize % 4].0),
            "Clear all pieces".to_string(),
        ]
    }

    /// Button `i` was clicked. Returns true when the pieces should be cleared.
    pub fn click(&mut self, i: usize) -> bool {
        match i {
            0 => self.blocks = !self.blocks,
            1 => self.deaths = (self.deaths + 1) % 4,
            2 => self.items = !self.items,
            3 => self.explosions = !self.explosions,
            4 => self.smoke = !self.smoke,
            5 => self.snow = !self.snow,
            6 => self.ropes = !self.ropes,
            7 => self.capes = !self.capes,
            8 => self.waves = !self.waves,
            9 => self.plants = !self.plants,
            10 => self.liquids = !self.liquids,
            11 => self.wind = (self.wind + 1) % 3,
            12 => self.gravity = (self.gravity + 1) % 5,
            13 => self.pieces = (self.pieces + 1) % 3,
            14 => self.lifetime = (self.lifetime + 1) % 4,
            15 => return true,
            _ => {}
        }
        false
    }

    pub fn to_text(&self) -> String {
        format!(
            "blocks={}\ndeaths={}\nitems={}\nexplosions={}\nsmoke={}\nsnow={}\nropes={}\ncapes={}\nwaves={}\nplants={}\nliquids={}\nwind={}\ngravity={}\npieces={}\nlifetime={}\n",
            self.blocks as u8,
            self.deaths,
            self.items as u8,
            self.explosions as u8,
            self.smoke as u8,
            self.snow as u8,
            self.ropes as u8,
            self.capes as u8,
            self.waves as u8,
            self.plants as u8,
            self.liquids as u8,
            self.wind,
            self.gravity,
            self.pieces,
            self.lifetime
        )
    }

    pub fn from_text(text: &str) -> Self {
        let mut s = Self::default();
        for line in text.lines() {
            let Some((k, v)) = line.split_once('=') else { continue };
            let Ok(n) = v.trim().parse::<u8>() else { continue };
            match k.trim() {
                "blocks" => s.blocks = n != 0,
                "deaths" => s.deaths = n % 4,
                "items" => s.items = n != 0,
                "explosions" => s.explosions = n != 0,
                "smoke" => s.smoke = n != 0,
                "snow" => s.snow = n != 0,
                "ropes" => s.ropes = n != 0,
                "capes" => s.capes = n != 0,
                "waves" => s.waves = n != 0,
                "plants" => s.plants = n != 0,
                "liquids" => s.liquids = n != 0,
                "wind" => s.wind = n % 3,
                "gravity" => s.gravity = n % 5,
                "pieces" => s.pieces = n % 3,
                "lifetime" => s.lifetime = n % 4,
                _ => {}
            }
        }
        s
    }

    pub fn gravity(&self) -> f64 {
        EARTH * GRAVITY[self.gravity as usize % 5].1
    }

    pub fn wind(&self) -> f64 {
        WIND[self.wind as usize % 3].1
    }

    pub fn pieces_per_block(&self) -> usize {
        PIECES[self.pieces as usize % 3].1
    }

    pub fn lifetime(&self) -> f64 {
        LIFETIME[self.lifetime as usize % 4].1
    }
}

static SETTINGS: Mutex<Option<(Settings, Option<PathBuf>)>> = Mutex::new(None);

/// Reads the settings file once (the folder comes from the game's native config dir).
pub fn load(dir: Option<PathBuf>) {
    let mut g = SETTINGS.lock().unwrap_or_else(|e| e.into_inner());
    if g.is_some() {
        return;
    }
    let path = dir.map(|d| d.join("horizon_physics.txt"));
    let s = path
        .as_ref()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .map(|t| Settings::from_text(&t))
        .unwrap_or_default();
    *g = Some((s, path));
}

pub fn loaded() -> bool {
    SETTINGS.lock().map(|g| g.is_some()).unwrap_or(true)
}

pub fn settings() -> Settings {
    SETTINGS.lock().ok().and_then(|g| g.as_ref().map(|s| s.0)).unwrap_or_default()
}

pub fn setting_labels() -> Vec<String> {
    settings().labels()
}

pub fn setting_click(i: usize) {
    let clear_all = {
        let mut g = SETTINGS.lock().unwrap_or_else(|e| e.into_inner());
        let entry = g.get_or_insert_with(|| (Settings::default(), None));
        let clear_all = entry.0.click(i);
        if let Some(p) = &entry.1 {
            if let Some(parent) = p.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let _ = std::fs::write(p, entry.0.to_text());
        }
        clear_all
    };
    if clear_all {
        clear();
    }
}

// ---------------------------------------------------------------------------------------------
// Math
// ---------------------------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct V3 {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

pub const fn v3(x: f64, y: f64, z: f64) -> V3 {
    V3 { x, y, z }
}

impl std::ops::Add for V3 {
    type Output = V3;
    fn add(self, o: V3) -> V3 {
        v3(self.x + o.x, self.y + o.y, self.z + o.z)
    }
}
impl std::ops::Sub for V3 {
    type Output = V3;
    fn sub(self, o: V3) -> V3 {
        v3(self.x - o.x, self.y - o.y, self.z - o.z)
    }
}
impl std::ops::Mul<f64> for V3 {
    type Output = V3;
    fn mul(self, k: f64) -> V3 {
        v3(self.x * k, self.y * k, self.z * k)
    }
}
impl std::ops::Neg for V3 {
    type Output = V3;
    fn neg(self) -> V3 {
        v3(-self.x, -self.y, -self.z)
    }
}

impl V3 {
    pub fn dot(self, o: V3) -> f64 {
        self.x * o.x + self.y * o.y + self.z * o.z
    }
    pub fn cross(self, o: V3) -> V3 {
        v3(self.y * o.z - self.z * o.y, self.z * o.x - self.x * o.z, self.x * o.y - self.y * o.x)
    }
    pub fn len(self) -> f64 {
        self.dot(self).sqrt()
    }
    pub fn norm(self) -> V3 {
        let l = self.len();
        if l > 1e-12 { self * (1.0 / l) } else { V3::default() }
    }
    pub fn mul_each(self, o: V3) -> V3 {
        v3(self.x * o.x, self.y * o.y, self.z * o.z)
    }
    pub fn get(self, k: usize) -> f64 {
        match k {
            0 => self.x,
            1 => self.y,
            _ => self.z,
        }
    }
    pub fn set(&mut self, k: usize, v: f64) {
        match k {
            0 => self.x = v,
            1 => self.y = v,
            _ => self.z = v,
        }
    }
    pub fn axis(k: usize, s: f64) -> V3 {
        let mut a = V3::default();
        a.set(k, s);
        a
    }
    fn from_f32(a: [f32; 3]) -> V3 {
        v3(f64::from(a[0]), f64::from(a[1]), f64::from(a[2]))
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Quat {
    pub w: f64,
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

impl Default for Quat {
    fn default() -> Self {
        Quat::IDENTITY
    }
}

impl Quat {
    pub const IDENTITY: Quat = Quat { w: 1.0, x: 0.0, y: 0.0, z: 0.0 };

    pub fn mul(self, o: Quat) -> Quat {
        Quat {
            w: self.w * o.w - self.x * o.x - self.y * o.y - self.z * o.z,
            x: self.w * o.x + self.x * o.w + self.y * o.z - self.z * o.y,
            y: self.w * o.y - self.x * o.z + self.y * o.w + self.z * o.x,
            z: self.w * o.z + self.x * o.y - self.y * o.x + self.z * o.w,
        }
    }
    pub fn conj(self) -> Quat {
        Quat { w: self.w, x: -self.x, y: -self.y, z: -self.z }
    }
    pub fn normalize(self) -> Quat {
        let l = (self.w * self.w + self.x * self.x + self.y * self.y + self.z * self.z).sqrt();
        if l < 1e-12 {
            return Quat::IDENTITY;
        }
        Quat { w: self.w / l, x: self.x / l, y: self.y / l, z: self.z / l }
    }
    pub fn rotate(self, v: V3) -> V3 {
        let u = v3(self.x, self.y, self.z);
        let t = u.cross(v) * 2.0;
        v + t * self.w + u.cross(t)
    }
    pub fn axis_angle(axis: V3, angle: f64) -> Quat {
        let a = axis.norm();
        let (s, c) = (angle * 0.5).sin_cos();
        Quat { w: c, x: a.x * s, y: a.y * s, z: a.z * s }
    }
    /// The rotation whose matrix has these orthonormal columns.
    pub fn from_axes(a: V3, b: V3, c: V3) -> Quat {
        let (m00, m11, m22) = (a.x, b.y, c.z);
        let tr = m00 + m11 + m22;
        let q = if tr > 0.0 {
            let s = (tr + 1.0).sqrt() * 2.0;
            Quat { w: 0.25 * s, x: (b.z - c.y) / s, y: (c.x - a.z) / s, z: (a.y - b.x) / s }
        } else if m00 > m11 && m00 > m22 {
            let s = (1.0 + m00 - m11 - m22).sqrt() * 2.0;
            Quat { w: (b.z - c.y) / s, x: 0.25 * s, y: (b.x + a.y) / s, z: (c.x + a.z) / s }
        } else if m11 > m22 {
            let s = (1.0 + m11 - m00 - m22).sqrt() * 2.0;
            Quat { w: (c.x - a.z) / s, x: (b.x + a.y) / s, y: 0.25 * s, z: (c.y + b.z) / s }
        } else {
            let s = (1.0 + m22 - m00 - m11).sqrt() * 2.0;
            Quat { w: (a.y - b.x) / s, x: (c.x + a.z) / s, y: (c.y + b.z) / s, z: 0.25 * s }
        };
        q.normalize()
    }
    /// The shortest rotation taking unit vector `from` to unit vector `to`.
    pub fn between(from: V3, to: V3) -> Quat {
        let d = from.dot(to).clamp(-1.0, 1.0);
        if d > 0.999_999 {
            return Quat::IDENTITY;
        }
        if d < -0.999_999 {
            let mut axis = v3(1.0, 0.0, 0.0).cross(from);
            if axis.len() < 1e-6 {
                axis = v3(0.0, 0.0, 1.0).cross(from);
            }
            return Quat::axis_angle(axis, std::f64::consts::PI);
        }
        let c = from.cross(to);
        Quat { w: 1.0 + d, x: c.x, y: c.y, z: c.z }.normalize()
    }
    pub fn slerp(self, mut o: Quat, t: f64) -> Quat {
        let mut d = self.w * o.w + self.x * o.x + self.y * o.y + self.z * o.z;
        if d < 0.0 {
            o = Quat { w: -o.w, x: -o.x, y: -o.y, z: -o.z };
            d = -d;
        }
        if d > 0.9995 {
            return Quat {
                w: self.w + (o.w - self.w) * t,
                x: self.x + (o.x - self.x) * t,
                y: self.y + (o.y - self.y) * t,
                z: self.z + (o.z - self.z) * t,
            }
            .normalize();
        }
        let th = d.acos();
        let s = th.sin();
        let (a, b) = (((1.0 - t) * th).sin() / s, (t * th).sin() / s);
        Quat {
            w: self.w * a + o.w * b,
            x: self.x * a + o.x * b,
            y: self.y * a + o.y * b,
            z: self.z * a + o.z * b,
        }
    }
    /// Turned by angular velocity `w` for `h` seconds.
    fn integrate(self, w: V3, h: f64) -> Quat {
        let dq = Quat { w: 0.0, x: w.x, y: w.y, z: w.z }.mul(self);
        Quat {
            w: self.w + 0.5 * h * dq.w,
            x: self.x + 0.5 * h * dq.x,
            y: self.y + 0.5 * h * dq.y,
            z: self.z + 0.5 * h * dq.z,
        }
        .normalize()
    }
}

/// A small, fast random source (xorshift64*); the look needs no more.
#[derive(Clone, Debug)]
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1)
    }
    pub fn next(&mut self) -> f64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        (self.0.wrapping_mul(0x2545_F491_4F6C_DD1D) >> 11) as f64 / (1u64 << 53) as f64
    }
    pub fn range(&mut self, lo: f64, hi: f64) -> f64 {
        lo + (hi - lo) * self.next()
    }
    pub fn unit(&mut self) -> V3 {
        v3(self.range(-1.0, 1.0), self.range(-1.0, 1.0), self.range(-1.0, 1.0)).norm()
    }
}

// ---------------------------------------------------------------------------------------------
// The world, as the solver sees it
// ---------------------------------------------------------------------------------------------

/// What fills one block cell.
#[derive(Clone, Debug)]
pub enum Cell {
    Air,
    /// Water or lava: the pieces sink slowly.
    Fluid,
    Full,
    /// Partial collision boxes in block space (`[x0, y0, z0, x1, y1, z1]`), as slabs and stairs.
    Boxes(std::sync::Arc<[[f64; 6]]>),
}

pub trait World {
    fn cell(&self, x: i32, y: i32, z: i32) -> Cell;
    /// Packed light as entity vertices take it: sky * 16 in bits 0..8, block * 16 in 8..16.
    fn light(&self, x: i32, y: i32, z: i32) -> u32;
}

/// A world made of two functions, for the hooks (which can hold only a function each).
pub struct FnWorld<C, L>(pub C, pub L);

impl<C, L> World for FnWorld<C, L>
where
    C: Fn(i32, i32, i32) -> Cell,
    L: Fn(i32, i32, i32) -> u32,
{
    fn cell(&self, x: i32, y: i32, z: i32) -> Cell {
        (self.0)(x, y, z)
    }
    fn light(&self, x: i32, y: i32, z: i32) -> u32 {
        (self.1)(x, y, z)
    }
}

fn floor(v: f64) -> i32 {
    v.floor() as i32
}

/// Where point `p` (which was at `prev` a moment before) is inside a block: the way out and how
/// deep it is.
fn penetration(world: &dyn World, p: V3, prev: V3, ignore: Option<[i32; 3]>) -> Option<(V3, f64)> {
    let cell = [floor(p.x), floor(p.y), floor(p.z)];
    if ignore == Some(cell) {
        return None;
    }
    let o = v3(f64::from(cell[0]), f64::from(cell[1]), f64::from(cell[2]));
    let full: [[f64; 6]; 1] = [[0.0, 0.0, 0.0, 1.0, 1.0, 1.0]];
    let c = world.cell(cell[0], cell[1], cell[2]);
    let boxes: &[[f64; 6]] = match &c {
        Cell::Air | Cell::Fluid => return None,
        Cell::Full => &full,
        Cell::Boxes(b) => b,
    };
    let is_full = matches!(c, Cell::Full);
    for b in boxes {
        let lo = o + v3(b[0], b[1], b[2]);
        let hi = o + v3(b[3], b[4], b[5]);
        let inside = (0..3).all(|k| p.get(k) >= lo.get(k) && p.get(k) < hi.get(k));
        if !inside {
            continue;
        }
        // The face the point came in through: of the faces it crossed, the last one crossed.
        let mut best: Option<(V3, f64, f64)> = None;
        for k in 0..3 {
            let (a, b0, c0) = (prev.get(k), lo.get(k), hi.get(k));
            let d = p.get(k) - a;
            if a <= b0 && d.abs() > 1e-12 {
                let t = (b0 - a) / d;
                if best.is_none_or(|x| t > x.2) {
                    best = Some((V3::axis(k, -1.0), p.get(k) - b0, t));
                }
            }
            if a >= c0 && d.abs() > 1e-12 {
                let t = (c0 - a) / d;
                if best.is_none_or(|x| t > x.2) {
                    best = Some((V3::axis(k, 1.0), c0 - p.get(k), t));
                }
            }
        }
        if let Some((n, depth, _)) = best {
            return Some((n, depth + 1e-4));
        }
        // Already inside before: the shallowest way out that does not lead into another block.
        let mut out: Option<(V3, f64)> = None;
        for k in 0..3 {
            for s in [-1.0, 1.0] {
                let depth = if s > 0.0 { hi.get(k) - p.get(k) } else { p.get(k) - lo.get(k) };
                if is_full {
                    let mut nb = cell;
                    nb[k] += s as i32;
                    if matches!(world.cell(nb[0], nb[1], nb[2]), Cell::Full) {
                        continue;
                    }
                }
                if out.is_none_or(|x| depth < x.1) {
                    out = Some((V3::axis(k, s), depth));
                }
            }
        }
        let (n, depth) = out.unwrap_or((v3(0.0, 1.0, 0.0), hi.y - p.y));
        return Some((n, depth + 1e-4));
    }
    None
}

// ---------------------------------------------------------------------------------------------
// Bodies
// ---------------------------------------------------------------------------------------------

/// One vertex of a piece's mesh, in the piece's own frame.
#[derive(Clone, Copy, Debug)]
pub struct MeshVertex {
    pub pos: [f32; 3],
    pub normal: [f32; 3],
    pub normal_w: i8,
    pub uv: [f32; 2],
    pub layer: u32,
    pub color: u32,
    pub glint_uv: [f32; 2],
}

#[derive(Clone, Copy, Debug)]
struct Contact {
    /// The touching corner, in the body's frame.
    local: V3,
    n: V3,
    lambda: f64,
    vn_before: f64,
}

#[derive(Clone, Debug)]
pub struct Body {
    pub x: V3,
    pub q: Quat,
    pub v: V3,
    pub w: V3,
    px: V3,
    pq: Quat,
    pub half: V3,
    inv_mass: f64,
    inv_inertia: V3,
    pub verts: Vec<MeshVertex>,
    contacts: Vec<Contact>,
}

impl Body {
    pub fn new(x: V3, q: Quat, half: V3, verts: Vec<MeshVertex>) -> Body {
        let h = v3(half.x.max(0.02), half.y.max(0.02), half.z.max(0.02));
        // Density 1, a box; small pieces are not made heavier than they are.
        let mass = (8.0 * h.x * h.y * h.z).max(1e-4);
        let (a, b, c) = (2.0 * h.x, 2.0 * h.y, 2.0 * h.z);
        let inertia = v3(
            mass / 12.0 * (b * b + c * c),
            mass / 12.0 * (a * a + c * c),
            mass / 12.0 * (a * a + b * b),
        );
        Body {
            x,
            q,
            v: V3::default(),
            w: V3::default(),
            px: x,
            pq: q,
            half: h,
            inv_mass: 1.0 / mass,
            inv_inertia: v3(1.0 / inertia.x, 1.0 / inertia.y, 1.0 / inertia.z),
            verts,
            contacts: Vec::new(),
        }
    }

    fn inv_inertia_world(&self, v: V3) -> V3 {
        self.q.rotate(self.q.conj().rotate(v).mul_each(self.inv_inertia))
    }

    /// The inverse mass felt at offset `r` along `n`.
    fn gen_inv_mass(&self, r: V3, n: V3) -> f64 {
        let rn = r.cross(n);
        self.inv_mass + rn.dot(self.inv_inertia_world(rn))
    }

    fn move_by(&mut self, p: V3, r: V3) {
        self.x = self.x + p * self.inv_mass;
        let dw = self.inv_inertia_world(r.cross(p));
        let dq = Quat { w: 0.0, x: dw.x, y: dw.y, z: dw.z }.mul(self.q);
        self.q = Quat {
            w: self.q.w + 0.5 * dq.w,
            x: self.q.x + 0.5 * dq.x,
            y: self.q.y + 0.5 * dq.y,
            z: self.q.z + 0.5 * dq.z,
        }
        .normalize();
    }

    fn push(&mut self, p: V3, r: V3) {
        self.v = self.v + p * self.inv_mass;
        self.w = self.w + self.inv_inertia_world(r.cross(p));
    }

    fn corners(&self) -> [V3; 8] {
        let h = self.half * 0.999;
        std::array::from_fn(|i| {
            v3(
                if i & 1 == 0 { -h.x } else { h.x },
                if i & 2 == 0 { -h.y } else { h.y },
                if i & 4 == 0 { -h.z } else { h.z },
            )
        })
    }

    fn radius(&self) -> f64 {
        self.half.len()
    }
}

/// Moves `a`'s point at `ra` by `corr` (and `b`'s at `rb` the other way), split by how easily
/// each moves there. Returns the size of the correction (the constraint's lambda).
fn correct(a: &mut Body, b: Option<(&mut Body, V3)>, corr: V3, ra: V3) -> f64 {
    let c = corr.len();
    if c < 1e-9 {
        return 0.0;
    }
    let n = corr * (1.0 / c);
    let wa = a.gen_inv_mass(ra, n);
    let wb = b.as_ref().map_or(0.0, |(b, rb)| b.gen_inv_mass(*rb, n));
    if wa + wb < 1e-12 {
        return 0.0;
    }
    let dl = c / (wa + wb);
    let p = n * dl;
    a.move_by(p, ra);
    if let Some((b, rb)) = b {
        b.move_by(-p, rb);
    }
    dl
}

#[derive(Clone, Copy, Debug)]
pub struct Joint {
    pub a: usize,
    pub b: usize,
    pub ra: V3,
    pub rb: V3,
}

#[derive(Clone, Debug)]
pub struct Object {
    pub first: usize,
    pub count: usize,
    pub joints: Vec<Joint>,
    pub age: f64,
    pub life: f64,
    /// The block a crumbling block's pieces start in: ignored until the block is gone.
    ignore: Option<[i32; 3]>,
    still: f64,
    pub sleeping: bool,
    check: f64,
}

const SUBSTEPS: usize = 4;
const STATIC_FRICTION: f64 = 0.6;
const DYNAMIC_FRICTION: f64 = 0.45;
const RESTITUTION: f64 = 0.25;
const MAX_BODIES: usize = 1200;

pub struct Physics {
    pub bodies: Vec<Body>,
    pub objects: Vec<Object>,
    rng: Rng,
    time: f64,
}

impl Default for Physics {
    fn default() -> Self {
        Physics { bodies: Vec::new(), objects: Vec::new(), rng: Rng::new(0x5EED), time: 0.0 }
    }
}

fn two(bodies: &mut [Body], i: usize, j: usize) -> (&mut Body, &mut Body) {
    if i < j {
        let (a, b) = bodies.split_at_mut(j);
        (&mut a[i], &mut b[0])
    } else {
        let (a, b) = bodies.split_at_mut(i);
        (&mut b[0], &mut a[j])
    }
}

impl Physics {
    pub fn len(&self) -> usize {
        self.bodies.len()
    }

    pub fn is_empty(&self) -> bool {
        self.bodies.is_empty()
    }

    pub fn clear(&mut self) {
        self.bodies.clear();
        self.objects.clear();
    }

    pub fn add(&mut self, bodies: Vec<Body>, joints: Vec<Joint>, life: f64, ignore: Option<[i32; 3]>) {
        if bodies.is_empty() {
            return;
        }
        // Too many pieces: the oldest go first.
        while self.bodies.len() + bodies.len() > MAX_BODIES && !self.objects.is_empty() {
            let mut keep = vec![true; self.objects.len()];
            keep[0] = false;
            self.retain(&keep);
        }
        let first = self.bodies.len();
        let count = bodies.len();
        self.bodies.extend(bodies);
        self.objects.push(Object {
            first,
            count,
            joints,
            age: 0.0,
            life,
            ignore,
            still: 0.0,
            sleeping: false,
            check: 0.0,
        });
    }

    fn retain(&mut self, keep: &[bool]) {
        let mut bodies = Vec::with_capacity(self.bodies.len());
        let mut objects = Vec::with_capacity(self.objects.len());
        let old = std::mem::take(&mut self.bodies);
        let mut old: Vec<Option<Body>> = old.into_iter().map(Some).collect();
        for (o, &k) in std::mem::take(&mut self.objects).into_iter().zip(keep) {
            if !k {
                continue;
            }
            let first = bodies.len();
            for i in o.first..o.first + o.count {
                if let Some(b) = old[i].take() {
                    bodies.push(b);
                }
            }
            objects.push(Object { first, ..o });
        }
        self.bodies = bodies;
        self.objects = objects;
    }

    /// Advances the world by `dt` seconds.
    pub fn step(&mut self, dt: f64, gravity: f64, world: &dyn World) {
        let dt = dt.clamp(0.0, 0.05);
        if dt <= 0.0 || self.objects.is_empty() {
            return;
        }
        self.time += dt;
        let h = dt / SUBSTEPS as f64;
        // Which body belongs to which object, and whether it moves.
        let mut owner = vec![0usize; self.bodies.len()];
        for (oi, o) in self.objects.iter().enumerate() {
            for i in o.first..o.first + o.count {
                owner[i] = oi;
            }
        }
        // Ignore a breaking block's own cell only until it has turned to air.
        for o in &mut self.objects {
            if let Some(c) = o.ignore
                && (o.age > 0.5 || matches!(world.cell(c[0], c[1], c[2]), Cell::Air | Cell::Fluid))
            {
                o.ignore = None;
            }
        }
        for _ in 0..SUBSTEPS {
            self.substep(h, gravity, world, &owner);
        }
        self.after_step(dt, world);
    }

    fn substep(&mut self, h: f64, gravity: f64, world: &dyn World, owner: &[usize]) {
        // Move.
        for o in &self.objects {
            if o.sleeping {
                continue;
            }
            for b in &mut self.bodies[o.first..o.first + o.count] {
                b.px = b.x;
                b.pq = b.q;
                let mut g = gravity;
                let c = b.x;
                if matches!(world.cell(floor(c.x), floor(c.y), floor(c.z)), Cell::Fluid) {
                    g *= 0.15;
                    let k = (1.0 - 4.0 * h).max(0.0);
                    b.v = b.v * k;
                    b.w = b.w * k;
                }
                b.v.y -= g * h;
                b.x = b.x + b.v * h;
                b.q = b.q.integrate(b.w, h);
            }
        }
        // Joints of ragdolls.
        for o in &self.objects {
            if o.sleeping {
                continue;
            }
            for j in &o.joints {
                let (a, b) = two(&mut self.bodies, o.first + j.a, o.first + j.b);
                let ra = a.q.rotate(j.ra);
                let rb = b.q.rotate(j.rb);
                let corr = (b.x + rb) - (a.x + ra);
                correct(a, Some((b, rb)), corr, ra);
            }
        }
        // Blocks.
        for o in &self.objects {
            if o.sleeping {
                continue;
            }
            for b in &mut self.bodies[o.first..o.first + o.count] {
                b.contacts.clear();
                for c in b.corners() {
                    let p = b.x + b.q.rotate(c);
                    let prev = b.px + b.pq.rotate(c);
                    let Some((n, depth)) = penetration(world, p, prev, o.ignore) else {
                        continue;
                    };
                    let r = p - b.x;
                    let vn_before = (b.v + b.w.cross(r)).dot(n);
                    let lambda = correct(b, None, n * depth, r);
                    // Static friction: a corner that slid less than the push stays put.
                    let r = b.q.rotate(c);
                    let p = b.x + r;
                    let d = p - prev;
                    let slide = d - n * d.dot(n);
                    if slide.len() < STATIC_FRICTION * depth {
                        correct(b, None, -slide, r);
                    }
                    b.contacts.push(Contact { local: c, n, lambda, vn_before });
                }
            }
        }
        // Pieces against each other.
        self.collide_bodies(owner);
        // Velocities from the moves.
        for o in &self.objects {
            if o.sleeping {
                continue;
            }
            for b in &mut self.bodies[o.first..o.first + o.count] {
                b.v = (b.x - b.px) * (1.0 / h);
                let mut dq = b.q.mul(b.pq.conj());
                if dq.w < 0.0 {
                    dq = Quat { w: -dq.w, x: -dq.x, y: -dq.y, z: -dq.z };
                }
                b.w = v3(dq.x, dq.y, dq.z) * (2.0 / h);
                // Bounce and sliding friction at the corners that touched.
                let contacts = std::mem::take(&mut b.contacts);
                for c in &contacts {
                    let r = b.q.rotate(c.local);
                    let vel = b.v + b.w.cross(r);
                    let vn = vel.dot(c.n);
                    let vt = vel - c.n * vn;
                    let mut dv = V3::default();
                    let vt_len = vt.len();
                    if vt_len > 1e-9 {
                        let f = (DYNAMIC_FRICTION * c.lambda / h).min(vt_len);
                        dv = dv - vt * (f / vt_len);
                    }
                    let e = if c.vn_before < -1.0 { RESTITUTION } else { 0.0 };
                    dv = dv + c.n * (-vn + (-e * c.vn_before).max(0.0));
                    let l = dv.len();
                    if l > 1e-9 {
                        let dir = dv * (1.0 / l);
                        let w = b.gen_inv_mass(r, dir);
                        if w > 1e-12 {
                            b.push(dir * (l / w), r);
                        }
                    }
                }
                b.contacts = contacts;
                // A little air drag, so spinning pieces settle.
                b.w = b.w * (1.0 - 0.6 * h);
                b.v = b.v * (1.0 - 0.05 * h);
            }
        }
    }

    fn collide_bodies(&mut self, owner: &[usize]) {
        let n = self.bodies.len();
        if n < 2 {
            return;
        }
        let mut order: Vec<usize> = (0..n).collect();
        order.sort_by(|&a, &b| {
            (self.bodies[a].x.x - self.bodies[a].radius())
                .partial_cmp(&(self.bodies[b].x.x - self.bodies[b].radius()))
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        for ii in 0..n {
            let i = order[ii];
            let (xi, ri) = (self.bodies[i].x, self.bodies[i].radius());
            for &j in &order[ii + 1..] {
                let (xj, rj) = (self.bodies[j].x, self.bodies[j].radius());
                if xj.x - rj > xi.x + ri {
                    break;
                }
                if owner[i] == owner[j] {
                    continue;
                }
                let (si, sj) = (self.objects[owner[i]].sleeping, self.objects[owner[j]].sleeping);
                if si && sj {
                    continue;
                }
                if (xi - xj).len() > ri + rj {
                    continue;
                }
                self.pair(i, j, si, sj);
                self.pair(j, i, sj, si);
            }
        }
    }

    /// Pushes `a`'s corners out of `b`.
    fn pair(&mut self, a: usize, b: usize, a_static: bool, b_static: bool) {
        if a_static {
            return;
        }
        let corners = self.bodies[a].corners();
        for c in corners {
            let (ba, bb) = two(&mut self.bodies, a, b);
            let p = ba.x + ba.q.rotate(c);
            let lp = bb.q.conj().rotate(p - bb.x);
            let mut best: Option<(usize, f64)> = None;
            for k in 0..3 {
                let pen = bb.half.get(k) - lp.get(k).abs();
                if pen <= 0.0 {
                    best = None;
                    break;
                }
                if best.is_none_or(|x| pen < x.1) {
                    best = Some((k, pen));
                }
            }
            // `best` is None when the corner is outside on some axis.
            let inside = (0..3).all(|k| lp.get(k).abs() < bb.half.get(k));
            let (Some((k, pen)), true) = (best, inside) else { continue };
            let s = if lp.get(k) >= 0.0 { 1.0 } else { -1.0 };
            let nw = bb.q.rotate(V3::axis(k, s));
            let ra = p - ba.x;
            let rb = p - bb.x;
            if b_static {
                correct(ba, None, nw * pen, ra);
            } else {
                correct(ba, Some((bb, rb)), nw * pen, ra);
            }
        }
    }

    fn after_step(&mut self, dt: f64, world: &dyn World) {
        let mut keep = vec![true; self.objects.len()];
        for (oi, o) in self.objects.iter_mut().enumerate() {
            o.age += dt;
            if o.age > o.life {
                keep[oi] = false;
                continue;
            }
            let bodies = &mut self.bodies[o.first..o.first + o.count];
            if o.sleeping {
                // Wake when nothing holds the pieces up any more (the block below was broken).
                o.check += dt;
                if o.check > 0.5 {
                    o.check = 0.0;
                    let held = bodies.iter().any(|b| {
                        b.corners().iter().any(|&c| {
                            let p = b.x + b.q.rotate(c) - v3(0.0, 0.05, 0.0);
                            matches!(
                                world.cell(floor(p.x), floor(p.y), floor(p.z)),
                                Cell::Full | Cell::Boxes(_)
                            )
                        })
                    });
                    if !held {
                        o.sleeping = false;
                        o.still = 0.0;
                    }
                }
                continue;
            }
            let calm = bodies.iter().all(|b| b.v.len() < 0.12 && b.w.len() < 0.5);
            let touching = bodies.iter().any(|b| !b.contacts.is_empty());
            if calm && touching {
                o.still += dt;
            } else {
                o.still = 0.0;
            }
            if o.still > 0.8 {
                o.sleeping = true;
                for b in bodies.iter_mut() {
                    b.v = V3::default();
                    b.w = V3::default();
                }
            }
            // Pieces that fell out of the world.
            if bodies.iter().all(|b| b.x.y < -200.0) {
                keep[oi] = false;
            }
        }
        if keep.iter().any(|k| !k) {
            self.retain(&keep);
        }
    }

    /// Draws every piece, camera-relative, into `out`.
    pub fn emit(&self, camera: V3, world: &dyn World, out: &mut Vec<Vertex>) {
        for o in &self.objects {
            // Shrink away over the last half second.
            let s = ((o.life - o.age) / 0.5).clamp(0.0, 1.0) as f32;
            for b in &self.bodies[o.first..o.first + o.count] {
                let light = world.light(floor(b.x.x), floor(b.x.y), floor(b.x.z));
                let at = b.x - camera;
                for v in &b.verts {
                    let lp = v3(
                        f64::from(v.pos[0] * s),
                        f64::from(v.pos[1] * s),
                        f64::from(v.pos[2] * s),
                    );
                    let p = at + b.q.rotate(lp);
                    let n = b.q.rotate(V3::from_f32(v.normal));
                    let pack = |x: f64| (x.clamp(-1.0, 1.0) * 127.0) as i8;
                    out.push(Vertex {
                        pos: [p.x as f32, p.y as f32, p.z as f32],
                        uv: v.uv,
                        layer: v.layer,
                        color: v.color,
                        light,
                        normal: [pack(n.x), pack(n.y), pack(n.z), v.normal_w],
                        glint_uv: v.glint_uv,
                    });
                }
            }
        }
    }

    fn rng(&mut self) -> &mut Rng {
        &mut self.rng
    }
}

// ---------------------------------------------------------------------------------------------
// Crumbling blocks
// ---------------------------------------------------------------------------------------------

/// Splits box `b` (block space) into about `n` smaller boxes of uneven sizes.
pub fn split_box(b: [f64; 6], n: usize, rng: &mut Rng) -> Vec<[f64; 6]> {
    let mut boxes = vec![b];
    while boxes.len() < n.max(1) {
        // Split the biggest box across its longest side, somewhere near the middle.
        let (i, _) = boxes
            .iter()
            .enumerate()
            .map(|(i, b)| (i, (b[3] - b[0]) * (b[4] - b[1]) * (b[5] - b[2])))
            .fold((0, -1.0), |a, x| if x.1 > a.1 { x } else { a });
        let b = boxes[i];
        let size = [b[3] - b[0], b[4] - b[1], b[5] - b[2]];
        let mut k = if size[0] >= size[1] && size[0] >= size[2] {
            0
        } else if size[1] >= size[2] {
            1
        } else {
            2
        };
        // Now and then across another side, so not every piece is a cube.
        if rng.next() < 0.15 {
            k = (k + 1 + (rng.next() * 2.0) as usize) % 3;
        }
        if size[k] < 0.08 {
            break;
        }
        let at = b[k] + size[k] * rng.range(0.35, 0.65);
        let mut lo = b;
        let mut hi = b;
        lo[k + 3] = at;
        hi[k] = at;
        boxes[i] = lo;
        boxes.push(hi);
    }
    boxes
}

/// The six faces of box `b` (block space, block at `origin`), textured with the part of the
/// sprite `uv` that the box covers, so the pieces fit back together into the block.
fn box_mesh(b: [f64; 6], uv: [f32; 4], color: u32) -> (V3, V3, Vec<MeshVertex>) {
    let lo = v3(b[0], b[1], b[2]);
    let hi = v3(b[3], b[4], b[5]);
    let c = (lo + hi) * 0.5;
    let h = (hi - lo) * 0.5;
    let map = |u: f64, v: f64| -> [f32; 2] {
        let u = u.clamp(0.0, 1.0) as f32;
        let v = v.clamp(0.0, 1.0) as f32;
        [uv[0] + (uv[2] - uv[0]) * u, uv[1] + (uv[3] - uv[1]) * v]
    };
    let mut verts = Vec::with_capacity(24);
    // For each face: its normal and its four corners in block space, in order around the face.
    let faces: [([f32; 3], [V3; 4]); 6] = [
        ([1.0, 0.0, 0.0], [v3(hi.x, lo.y, lo.z), v3(hi.x, hi.y, lo.z), v3(hi.x, hi.y, hi.z), v3(hi.x, lo.y, hi.z)]),
        ([-1.0, 0.0, 0.0], [v3(lo.x, lo.y, hi.z), v3(lo.x, hi.y, hi.z), v3(lo.x, hi.y, lo.z), v3(lo.x, lo.y, lo.z)]),
        ([0.0, 1.0, 0.0], [v3(lo.x, hi.y, lo.z), v3(lo.x, hi.y, hi.z), v3(hi.x, hi.y, hi.z), v3(hi.x, hi.y, lo.z)]),
        ([0.0, -1.0, 0.0], [v3(lo.x, lo.y, hi.z), v3(lo.x, lo.y, lo.z), v3(hi.x, lo.y, lo.z), v3(hi.x, lo.y, hi.z)]),
        ([0.0, 0.0, 1.0], [v3(lo.x, lo.y, hi.z), v3(hi.x, lo.y, hi.z), v3(hi.x, hi.y, hi.z), v3(lo.x, hi.y, hi.z)]),
        ([0.0, 0.0, -1.0], [v3(hi.x, lo.y, lo.z), v3(lo.x, lo.y, lo.z), v3(lo.x, hi.y, lo.z), v3(hi.x, hi.y, lo.z)]),
    ];
    for (n, corners) in faces {
        for p in corners {
            let tex = if n[0] != 0.0 {
                map(if n[0] > 0.0 { 1.0 - p.z } else { p.z }, 1.0 - p.y)
            } else if n[1] != 0.0 {
                map(p.x, p.z)
            } else {
                map(if n[2] > 0.0 { p.x } else { 1.0 - p.x }, 1.0 - p.y)
            };
            let l = p - c;
            verts.push(MeshVertex {
                pos: [l.x as f32, l.y as f32, l.z as f32],
                normal: n,
                normal_w: -127,
                uv: tex,
                layer: 0,
                color,
                glint_uv: [0.0, 0.0],
            });
        }
    }
    (c, h, verts)
}

/// RGB tint (0xRRGGBB) as an entity vertex colour (ABGR bytes).
pub fn tint_color(tint: u32) -> u32 {
    let (r, g, b) = ((tint >> 16) & 255, (tint >> 8) & 255, tint & 255);
    0xFF00_0000 | b << 16 | g << 8 | r
}

impl Physics {
    /// The block at `pos` (outline `boxes` in block space) falls apart into textured pieces.
    pub fn spawn_block(&mut self, pos: [i32; 3], boxes: &[[f64; 6]], uv: [f32; 4], tint: u32, s: &Settings) {
        let total: f64 = boxes.iter().map(|b| (b[3] - b[0]) * (b[4] - b[1]) * (b[5] - b[2])).sum();
        if total <= 1e-6 {
            return;
        }
        let o = v3(f64::from(pos[0]), f64::from(pos[1]), f64::from(pos[2]));
        let color = tint_color(tint);
        let want = s.pieces_per_block();
        let mut bodies = Vec::new();
        for b in boxes {
            let vol = (b[3] - b[0]) * (b[4] - b[1]) * (b[5] - b[2]);
            let n = ((want as f64 * vol / total).round() as usize).max(1);
            for piece in split_box(*b, n, self.rng()) {
                let (c, h, verts) = box_mesh(piece, uv, color);
                let mut body = Body::new(o + c, Quat::IDENTITY, h, verts);
                // A gentle burst: outward from the middle of the block and a little up.
                let out = (c - v3(0.5, 0.5, 0.5)).norm();
                let r = self.rng().unit();
                body.v = v3(out.x * 0.6 + r.x * 0.4, self.rng.range(0.3, 1.6), out.z * 0.6 + r.z * 0.4);
                body.w = self.rng.unit() * self.rng.range(0.5, 4.0);
                bodies.push(body);
            }
        }
        let life = s.lifetime() + self.rng.range(0.0, 2.0);
        self.add(bodies, Vec::new(), life, Some(pos));
    }
}

// ---------------------------------------------------------------------------------------------
// Mobs: catching the model parts of a dying mob
// ---------------------------------------------------------------------------------------------

/// Set while this module draws a dying mob itself (the hook's own nested call).
pub struct Guard(AtomicBool);

impl Guard {
    pub const fn new() -> Self {
        Self(AtomicBool::new(false))
    }
    /// True when the guard was already set.
    pub fn enter(&self) -> bool {
        self.0.swap(true, Ordering::SeqCst)
    }
    pub fn leave(&self) {
        self.0.store(false, Ordering::SeqCst);
    }
}

impl Default for Guard {
    fn default() -> Self {
        Self::new()
    }
}

pub static RENDER: Guard = Guard::new();
/// Set while the lead hook draws the game's own leads into its buffer.
pub static LEADS: Guard = Guard::new();

static CAPTURE: Mutex<Option<Vec<usize>>> = Mutex::new(None);
static CAPTURING: AtomicBool = AtomicBool::new(false);
/// Whether the mod is on in Options > Mods (kept by the frame hook, read by the mob hook).
static ENABLED: AtomicBool = AtomicBool::new(true);

pub fn set_enabled(on: bool) {
    ENABLED.store(on, Ordering::Relaxed);
}

pub fn capture_begin() {
    if let Ok(mut c) = CAPTURE.lock() {
        *c = Some(Vec::new());
        CAPTURING.store(true, Ordering::SeqCst);
    }
}

/// A model part starts at vertex `len` of the buffer.
pub fn capture_mark(len: usize) {
    if !CAPTURING.load(Ordering::Relaxed) {
        return;
    }
    if let Ok(mut c) = CAPTURE.lock()
        && let Some(marks) = c.as_mut()
    {
        marks.push(len);
    }
}

pub fn capture_end() -> Vec<usize> {
    CAPTURING.store(false, Ordering::SeqCst);
    CAPTURE.lock().ok().and_then(|mut c| c.take()).unwrap_or_default()
}

/// A run of captured vertices: one model part (with any layers drawn over it).
struct Segment {
    verts: Vec<Vertex>,
    lo: V3,
    hi: V3,
}

impl Segment {
    fn volume(&self) -> f64 {
        let d = self.hi - self.lo;
        d.x.max(0.01) * d.y.max(0.01) * d.z.max(0.01)
    }
    fn center(&self) -> V3 {
        (self.lo + self.hi) * 0.5
    }
}

fn bounds(verts: &[Vertex], camera: V3) -> (V3, V3) {
    let mut lo = v3(f64::MAX, f64::MAX, f64::MAX);
    let mut hi = v3(f64::MIN, f64::MIN, f64::MIN);
    for v in verts {
        let p = V3::from_f32(v.pos) + camera;
        for k in 0..3 {
            lo.set(k, lo.get(k).min(p.get(k)));
            hi.set(k, hi.get(k).max(p.get(k)));
        }
    }
    (lo, hi)
}

fn overlap(a: (V3, V3), b: (V3, V3), margin: f64) -> Option<(V3, V3)> {
    let lo = v3(a.0.x.max(b.0.x), a.0.y.max(b.0.y), a.0.z.max(b.0.z)) - v3(margin, margin, margin);
    let hi = v3(a.1.x.min(b.1.x), a.1.y.min(b.1.y), a.1.z.min(b.1.z)) + v3(margin, margin, margin);
    if lo.x < hi.x && lo.y < hi.y && lo.z < hi.z { Some((lo, hi)) } else { None }
}

/// Cuts the captured vertices into model parts. Layers drawn over a part (an outer skin layer,
/// armour) land on the part they cover.
fn segments(verts: &[Vertex], marks: &[usize], camera: V3) -> Vec<Segment> {
    let mut cuts: Vec<usize> = marks.iter().copied().filter(|&m| m < verts.len()).collect();
    cuts.push(0);
    cuts.push(verts.len());
    cuts.sort_unstable();
    cuts.dedup();
    let mut segs: Vec<Segment> = Vec::new();
    for w in cuts.windows(2) {
        let part = &verts[w[0]..w[1]];
        if part.len() < 3 {
            if let Some(last) = segs.last_mut() {
                last.verts.extend_from_slice(part);
            }
            continue;
        }
        let (lo, hi) = bounds(part, camera);
        let seg = Segment { verts: part.to_vec(), lo, hi };
        // An overlay: mostly the same box as an earlier part.
        let host = segs.iter().position(|t| {
            let c = seg.center();
            let inside = (0..3).all(|k| c.get(k) > t.lo.get(k) - 0.1 && c.get(k) < t.hi.get(k) + 0.1);
            let ratio = seg.volume() / t.volume();
            let shared = overlap((t.lo, t.hi), (seg.lo, seg.hi), 0.0).map_or(0.0, |(l, h)| {
                let d = h - l;
                d.x * d.y * d.z
            });
            inside && (0.4..2.6).contains(&ratio) && shared > 0.45 * seg.volume().min(t.volume())
        });
        match host {
            Some(i) => {
                let t = &mut segs[i];
                t.verts.extend_from_slice(&seg.verts);
                for k in 0..3 {
                    t.lo.set(k, t.lo.get(k).min(seg.lo.get(k)));
                    t.hi.set(k, t.hi.get(k).max(seg.hi.get(k)));
                }
            }
            None => segs.push(seg),
        }
    }
    segs
}

/// A body for a part: a box along the part's own axes (from its face normals) around its vertices.
fn part_body(seg: &Segment, camera: V3) -> Body {
    let mut normals: Vec<V3> = Vec::new();
    for quad in seg.verts.chunks(4) {
        if quad.len() < 3 {
            continue;
        }
        let a = V3::from_f32(quad[0].pos);
        let n = (V3::from_f32(quad[1].pos) - a).cross(V3::from_f32(quad[2].pos) - a);
        if n.len() > 1e-7 {
            normals.push(n.norm());
        }
    }
    let ax = normals.first().copied().unwrap_or(v3(1.0, 0.0, 0.0));
    let other = normals
        .iter()
        .copied()
        .find(|n| n.dot(ax).abs() < 0.5)
        .unwrap_or(if ax.y.abs() < 0.9 { v3(0.0, 1.0, 0.0) } else { v3(1.0, 0.0, 0.0) });
    let by = (other - ax * other.dot(ax)).norm();
    let cz = ax.cross(by);
    let mut lo = v3(f64::MAX, f64::MAX, f64::MAX);
    let mut hi = v3(f64::MIN, f64::MIN, f64::MIN);
    for v in &seg.verts {
        let p = V3::from_f32(v.pos) + camera;
        let l = v3(p.dot(ax), p.dot(by), p.dot(cz));
        for k in 0..3 {
            lo.set(k, lo.get(k).min(l.get(k)));
            hi.set(k, hi.get(k).max(l.get(k)));
        }
    }
    let mid = (lo + hi) * 0.5;
    let center = ax * mid.x + by * mid.y + cz * mid.z;
    let half = (hi - lo) * 0.5;
    let q = Quat::from_axes(ax, by, cz);
    let inv = q.conj();
    let verts = seg
        .verts
        .iter()
        .map(|v| {
            let p = inv.rotate(V3::from_f32(v.pos) + camera - center);
            let n = inv.rotate(v3(
                f64::from(v.normal[0]) / 127.0,
                f64::from(v.normal[1]) / 127.0,
                f64::from(v.normal[2]) / 127.0,
            ));
            // The hurt-red overlay of the dying mob goes; an item's foil stays.
            let layer = match v.layer {
                1 => 0,
                8 => 7,
                l => l,
            };
            MeshVertex {
                pos: [p.x as f32, p.y as f32, p.z as f32],
                normal: [n.x as f32, n.y as f32, n.z as f32],
                normal_w: v.normal[3],
                uv: v.uv,
                layer,
                color: v.color,
                glint_uv: v.glint_uv,
            }
        })
        .collect();
    Body::new(center, q, half, verts)
}

/// Joints between parts that touch: a tree from the biggest part (the body), each joint where the
/// two parts meet. Parts that touch nothing hang from the nearest part.
fn joints(segs: &[Segment]) -> Vec<(usize, usize, V3)> {
    let n = segs.len();
    if n < 2 {
        return Vec::new();
    }
    let root = (0..n)
        .max_by(|&a, &b| segs[a].volume().partial_cmp(&segs[b].volume()).unwrap_or(std::cmp::Ordering::Equal))
        .unwrap_or(0);
    let mut inside = vec![false; n];
    inside[root] = true;
    let mut out = Vec::new();
    for _ in 1..n {
        let mut best: Option<(usize, usize, f64, V3)> = None;
        for a in (0..n).filter(|&a| inside[a]) {
            for b in (0..n).filter(|&b| !inside[b]) {
                let (sa, sb) = (&segs[a], &segs[b]);
                let (score, at) = match overlap((sa.lo, sa.hi), (sb.lo, sb.hi), 0.04) {
                    Some((l, h)) => {
                        let d = h - l;
                        (1.0 + d.x * d.y * d.z, (l + h) * 0.5)
                    }
                    None => {
                        // Not touching: hang it from the closest point of the other.
                        let c = sb.center();
                        let p = v3(
                            c.x.clamp(sa.lo.x, sa.hi.x),
                            c.y.clamp(sa.lo.y, sa.hi.y),
                            c.z.clamp(sa.lo.z, sa.hi.z),
                        );
                        (-(p - c).len(), (p + c) * 0.5)
                    }
                };
                if best.is_none_or(|x| score > x.2) {
                    best = Some((a, b, score, at));
                }
            }
        }
        let Some((a, b, _, at)) = best else { break };
        inside[b] = true;
        out.push((a, b, at));
    }
    out
}

impl Physics {
    /// A mob died: its parts (captured vertices, camera-relative) become a ragdoll or fall apart.
    /// `velocity` is the mob's motion in blocks per second.
    pub fn spawn_mob(
        &mut self,
        verts: &[Vertex],
        marks: &[usize],
        camera: V3,
        velocity: V3,
        mode: u8,
        life: f64,
    ) {
        let segs = segments(verts, marks, camera);
        if segs.is_empty() {
            return;
        }
        let mode = if mode == 3 {
            if self.rng.next() < 0.35 { 2 } else { 1 }
        } else {
            mode
        };
        let mut bodies: Vec<Body> = segs.iter().map(|s| part_body(s, camera)).collect();
        let mid = bodies.iter().fold(V3::default(), |a, b| a + b.x) * (1.0 / bodies.len() as f64);
        // Knocked away from the camera (the player that landed the blow, mostly).
        let away = v3(mid.x - camera.x, 0.0, mid.z - camera.z).norm();
        let base = velocity * 0.5 + away * 1.2 + v3(0.0, 1.2, 0.0);
        let mut joint_list = Vec::new();
        if mode == 2 {
            for b in &mut bodies {
                let out = (b.x - mid).norm();
                let r = self.rng.unit();
                b.v = base + out * 2.5 + r * 0.8 + v3(0.0, 1.0, 0.0);
                b.w = self.rng.unit() * self.rng.range(2.0, 8.0);
            }
        } else {
            for (a, b, at) in joints(&segs) {
                let ra = bodies[a].q.conj().rotate(at - bodies[a].x);
                let rb = bodies[b].q.conj().rotate(at - bodies[b].x);
                joint_list.push(Joint { a, b, ra, rb });
            }
            let spin = self.rng.unit() * 1.5;
            for b in &mut bodies {
                let r = self.rng.unit();
                b.v = base + r * 0.4;
                b.w = spin;
            }
        }
        self.add(bodies, joint_list, life, None);
    }
}

// ---------------------------------------------------------------------------------------------
// The shared state the hooks reach
// ---------------------------------------------------------------------------------------------

struct State {
    physics: Physics,
    /// Mobs already turned into pieces, by entity id, with when that was.
    dead: HashMap<u32, f64>,
    clock: Option<std::time::Instant>,
    last_dt: f64,
    camera: V3,
    dim: u8,
    items: HashMap<u32, (Quat, u64)>,
    frame: u64,
    env: Env,
}

static STATE: Mutex<Option<State>> = Mutex::new(None);

fn with<R>(f: impl FnOnce(&mut State) -> R) -> R {
    let mut g = STATE.lock().unwrap_or_else(|e| e.into_inner());
    let s = g.get_or_insert_with(|| State {
        physics: Physics::default(),
        dead: HashMap::new(),
        clock: None,
        last_dt: 1.0 / 60.0,
        camera: V3::default(),
        dim: 255,
        items: HashMap::new(),
        frame: 0,
        env: Env::default(),
    });
    f(s)
}

pub fn clear() {
    with(|s| {
        s.physics.clear();
        s.items.clear();
        s.env.clear();
    });
}

pub fn piece_count() -> usize {
    with(|s| s.physics.len())
}

/// Called by the block-breaking hook.
pub fn spawn_block(pos: [i32; 3], boxes: &[[f64; 6]], uv: [f32; 4], tint: u32) {
    let st = settings();
    if !st.blocks {
        return;
    }
    with(|s| s.physics.spawn_block(pos, boxes, uv, tint, &st));
}

/// Whether mob deaths become pieces at all.
pub fn deaths_on() -> bool {
    ENABLED.load(Ordering::Relaxed) && settings().deaths != 0
}

/// True once per mob: the first frame its death is seen.
pub fn first_death(id: u32) -> bool {
    with(|s| {
        if s.dead.contains_key(&id) {
            return false;
        }
        let now = s.physics.time;
        s.dead.insert(id, now);
        // Forget old deaths (ids are not reused within a session, but keep the map small).
        if s.dead.len() > 512 {
            s.dead.retain(|_, t| now - *t < 60.0);
        }
        true
    })
}

/// Called by the mob hook with the dying mob's vertices.
pub fn spawn_mob(verts: &[Vertex], marks: &[usize], start: usize, camera: [f64; 3], velocity: [f64; 3]) {
    let st = settings();
    if st.deaths == 0 {
        return;
    }
    let marks: Vec<usize> = marks.iter().filter_map(|m| m.checked_sub(start)).collect();
    let cam = v3(camera[0], camera[1], camera[2]);
    let vel = v3(velocity[0], velocity[1], velocity[2]);
    with(|s| {
        let before = s.physics.objects.len();
        s.physics.spawn_mob(verts, &marks, cam, vel, st.deaths, st.lifetime());
        // A mob killed by a blast is thrown by it.
        if s.physics.objects.len() > before {
            let now = s.env.clock;
            let blasts: Vec<Blast> = s.env.blasts.iter().filter(|b| now - b.at < 0.6).copied().collect();
            for b in blasts {
                s.physics.blast(b.center, b.radius * 2.0, 0.6, before);
            }
        }
    });
}

/// Once a frame: step the pieces and draw them. `dim` tells dimensions apart; a change, or a big
/// jump of the camera (a teleport, another world), clears them. `white` is a plain white texel of
/// the atlas, for smoke, sparks and footprints.
pub fn frame(dim: u8, camera: [f64; 3], white: [f32; 4], world: &dyn World, out: &mut Vec<Vertex>) {
    let st = settings();
    with(|s| {
        let cam = v3(camera[0], camera[1], camera[2]);
        if s.dim != dim || (cam - s.camera).len() > 300.0 {
            s.physics.clear();
            s.dead.clear();
            s.items.clear();
            s.env.clear();
            s.dim = dim;
        }
        s.camera = cam;
        let now = std::time::Instant::now();
        let dt = s.clock.map_or(0.0, |c| now.duration_since(c).as_secs_f64());
        s.clock = Some(now);
        s.last_dt = dt.clamp(0.0, 0.1);
        s.frame += 1;
        let frame = s.frame;
        s.items.retain(|_, (_, seen)| frame - *seen < 120);
        blow_pending(s, &st);
        s.physics.step(dt, st.gravity(), world);
        s.physics.emit(cam, world, out);
        s.env.step(s.last_dt, cam, frame, &st, world);
        s.env.emit(cam, white, world, out);
    });
}

// ---------------------------------------------------------------------------------------------
// Item physics
// ---------------------------------------------------------------------------------------------

pub fn items_on() -> bool {
    ENABLED.load(Ordering::Relaxed) && settings().items
}

/// The orientation (x, y, z, w) of dropped item `id` this frame. In the air it tumbles the way it
/// flies; on the ground it settles on its flattest side: flat items (`flat`) lie on their face,
/// blocks on whichever side is closest to down.
pub fn item_rotation(id: u32, grounded: bool, velocity: [f64; 3], flat: bool) -> [f32; 4] {
    with(|s| {
        let dt = s.last_dt;
        let frame = s.frame;
        let start = {
            let mut r = Rng::new(u64::from(id) ^ 0xA5A5);
            let yaw = Quat::axis_angle(v3(0.0, 1.0, 0.0), r.range(0.0, std::f64::consts::TAU));
            if flat { yaw.mul(Quat::axis_angle(v3(1.0, 0.0, 0.0), -std::f64::consts::FRAC_PI_2)) } else { yaw }
        };
        let entry = s.items.entry(id).or_insert((start, frame));
        entry.1 = frame;
        let vel = v3(velocity[0], velocity[1], velocity[2]);
        let mut q = entry.0;
        if !grounded {
            let flat_v = v3(vel.x, 0.0, vel.z);
            let axis = if flat_v.len() > 0.05 { v3(0.0, 1.0, 0.0).cross(flat_v).norm() } else { v3(1.0, 0.0, 0.3).norm() };
            let rate = 3.0 + 6.0 * vel.len().min(1.5);
            q = Quat::axis_angle(axis, rate * dt).mul(q).normalize();
        } else {
            // The local axis closest to straight up (only the face axis for flat items) is turned
            // the rest of the way, and the item eases into that.
            let up = v3(0.0, 1.0, 0.0);
            let axes: &[usize] = if flat { &[2] } else { &[0, 1, 2] };
            let mut best = (up, -2.0);
            for &k in axes {
                for sgn in [-1.0, 1.0] {
                    let a = q.rotate(V3::axis(k, sgn));
                    if a.y > best.1 {
                        best = (a, a.y);
                    }
                }
            }
            let target = Quat::between(best.0, up).mul(q).normalize();
            q = q.slerp(target, (dt * 12.0).min(1.0)).normalize();
        }
        entry.0 = q;
        [q.x as f32, q.y as f32, q.z as f32, q.w as f32]
    })
}


// ---------------------------------------------------------------------------------------------
// Explosions, smoke, footprints, ropes and capes
// ---------------------------------------------------------------------------------------------

/// Full block and sky light, for things that glow.
const GLOW: u32 = 240 | 240 << 8;

/// An entity vertex colour (ABGR bytes) from 0..1 RGB.
fn abgr(c: [f64; 3]) -> u32 {
    let b = |x: f64| (x.clamp(0.0, 1.0) * 255.0).round() as u32;
    0xFF00_0000 | b(c[2]) << 16 | b(c[1]) << 8 | b(c[0])
}

pub fn mix3(a: [f64; 3], b: [f64; 3], t: f64) -> [f64; 3] {
    let t = t.clamp(0.0, 1.0);
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t]
}

/// The wind at time `t` (seconds): a direction that wanders slowly and gusts, in blocks per
/// second, times `strength`.
pub fn wind_at(t: f64, strength: f64) -> V3 {
    if strength <= 0.0 {
        return V3::default();
    }
    let angle = 0.7 + t * 0.013 + 0.6 * (t * 0.071).sin();
    let gust = 0.65 + 0.25 * (t * 0.53).sin() + 0.1 * (t * 1.7 + 1.3).sin();
    v3(angle.cos(), 0.0, angle.sin()) * (1.6 * gust * strength)
}

fn solid(world: &dyn World, p: V3) -> bool {
    match world.cell(floor(p.x), floor(p.y), floor(p.z)) {
        Cell::Full => true,
        Cell::Boxes(b) => {
            let l = v3(p.x - p.x.floor(), p.y - p.y.floor(), p.z - p.z.floor());
            b.iter().any(|b| (0..3).all(|k| l.get(k) >= b[k] && l.get(k) < b[k + 3]))
        }
        _ => false,
    }
}

/// A cube of `half` around `at` (camera-relative), turned by `q`, in one colour.
#[allow(clippy::too_many_arguments)]
fn cube(at: V3, half: f64, q: Quat, white: [f32; 4], color: u32, light: u32, shaded: bool, out: &mut Vec<Vertex>) {
    let uv = [(white[0] + white[2]) * 0.5, (white[1] + white[3]) * 0.5];
    const F: [([f64; 3], [[f64; 3]; 4]); 6] = [
        ([1.0, 0.0, 0.0], [[1.0, -1.0, -1.0], [1.0, 1.0, -1.0], [1.0, 1.0, 1.0], [1.0, -1.0, 1.0]]),
        ([-1.0, 0.0, 0.0], [[-1.0, -1.0, 1.0], [-1.0, 1.0, 1.0], [-1.0, 1.0, -1.0], [-1.0, -1.0, -1.0]]),
        ([0.0, 1.0, 0.0], [[-1.0, 1.0, -1.0], [-1.0, 1.0, 1.0], [1.0, 1.0, 1.0], [1.0, 1.0, -1.0]]),
        ([0.0, -1.0, 0.0], [[-1.0, -1.0, 1.0], [-1.0, -1.0, -1.0], [1.0, -1.0, -1.0], [1.0, -1.0, 1.0]]),
        ([0.0, 0.0, 1.0], [[-1.0, -1.0, 1.0], [1.0, -1.0, 1.0], [1.0, 1.0, 1.0], [-1.0, 1.0, 1.0]]),
        ([0.0, 0.0, -1.0], [[1.0, -1.0, -1.0], [-1.0, -1.0, -1.0], [-1.0, 1.0, -1.0], [1.0, 1.0, -1.0]]),
    ];
    let pack = |x: f64| (x.clamp(-1.0, 1.0) * 127.0) as i8;
    for (n, corners) in F {
        let nw = q.rotate(v3(n[0], n[1], n[2]));
        let normal = if shaded { [pack(nw.x), pack(nw.y), pack(nw.z), -127] } else { [0; 4] };
        for c in corners {
            let p = at + q.rotate(v3(c[0], c[1], c[2]) * half);
            out.push(Vertex {
                pos: [p.x as f32, p.y as f32, p.z as f32],
                uv,
                layer: 0,
                color,
                light,
                normal,
                glint_uv: [0.0, 0.0],
            });
        }
    }
}

/// A flat quad (both sides) at four camera-relative corners.
fn quad(c: [V3; 4], white: [f32; 4], color: u32, light: u32, out: &mut Vec<Vertex>) {
    let uv = [(white[0] + white[2]) * 0.5, (white[1] + white[3]) * 0.5];
    for order in [[0, 1, 2, 3], [3, 2, 1, 0]] {
        for i in order {
            let p = c[i];
            out.push(Vertex {
                pos: [p.x as f32, p.y as f32, p.z as f32],
                uv,
                layer: 0,
                color,
                light,
                normal: [0, 127, 0, -127],
                glint_uv: [0.0, 0.0],
            });
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FxKind {
    /// Rises, drifts with the wind, grows and thins out.
    Smoke,
    /// Falls and bounces, glowing.
    Spark,
    /// A glowing ball of fire that swells and dies away quickly.
    Flame,
}

#[derive(Clone, Copy, Debug)]
pub struct Puff {
    pub x: V3,
    pub v: V3,
    pub size: f64,
    pub age: f64,
    pub life: f64,
    pub from: [f64; 3],
    pub to: [f64; 3],
    pub kind: FxKind,
    q: Quat,
    w: V3,
}

const MAX_PUFFS: usize = 1600;

impl Puff {
    fn new(kind: FxKind, x: V3, v: V3, size: f64, life: f64, from: [f64; 3], to: [f64; 3], rng: &mut Rng) -> Puff {
        let q = Quat::axis_angle(rng.unit(), rng.range(0.0, std::f64::consts::TAU));
        Puff { x, v, size, age: 0.0, life, from, to, kind, q, w: rng.unit() * rng.range(0.3, 1.5) }
    }

    /// How big the puff is now (0..1 of its size).
    pub fn scale_pub(&self) -> f64 {
        self.scale()
    }

    fn scale(&self) -> f64 {
        let t = (self.age / self.life).clamp(0.0, 1.0);
        match self.kind {
            FxKind::Smoke => (0.35 + 0.65 * (t * 4.0).min(1.0)) * (1.0 + 0.8 * t) * ((1.0 - t) / 0.35).min(1.0),
            FxKind::Spark => 1.0 - t * t,
            FxKind::Flame => (0.5 + 1.5 * (t * 3.0).min(1.0)) * ((1.0 - t) / 0.5).min(1.0),
        }
    }
}

/// An explosion seen in the last moments.
#[derive(Clone, Copy, Debug)]
pub struct Blast {
    pub center: V3,
    pub radius: f64,
    pub at: f64,
}

/// A block that has just turned to air (from the server), in case an explosion explains it.
#[derive(Clone, Debug)]
pub struct Aired {
    pub pos: [i32; 3],
    pub boxes: Vec<[f64; 6]>,
    pub uv: [f32; 4],
    pub tint: u32,
    pub at: f64,
}

#[derive(Clone, Copy, Debug)]
pub struct Print {
    pub x: V3,
    pub yaw: f64,
    pub age: f64,
}

#[derive(Clone, Debug)]
pub struct Rope {
    pub pts: Vec<V3>,
    prev: Vec<V3>,
    seen: u64,
    group: u8,
}

#[derive(Clone, Debug)]
pub struct Cloth {
    pub pts: Vec<V3>,
    prev: Vec<V3>,
    seen: u64,
    width: f64,
    height: f64,
}

/// The cloth grid of a cape: points across and down.
const CAPE_W: usize = 5;
const CAPE_H: usize = 8;

/// Everything that is not a rigid piece.
pub struct Env {
    pub puffs: Vec<Puff>,
    pub blasts: Vec<Blast>,
    aired: Vec<Aired>,
    /// Fire, campfires and lava near the camera, by position, with their kind.
    pub emitters: HashMap<[i32; 3], u8>,
    scan: i32,
    pub prints: Vec<Print>,
    last_step: Option<V3>,
    left: bool,
    pub ropes: Vec<Rope>,
    pub capes: HashMap<u32, Cloth>,
    /// Seconds since the start, for the wind.
    pub clock: f64,
    rng: Rng,
    /// Explosions waiting for the next frame's world (their pieces need the blocks).
    pending: Vec<(Blast, Vec<Aired>)>,
    frame: u64,
    camera: V3,
    /// Liquid droplets.
    pub drops: Vec<Drop>,
    /// Whether the player's feet were in water last frame.
    was_in_water: bool,
}

impl Default for Env {
    fn default() -> Self {
        Env {
            puffs: Vec::new(),
            blasts: Vec::new(),
            aired: Vec::new(),
            emitters: HashMap::new(),
            scan: 0,
            prints: Vec::new(),
            last_step: None,
            left: false,
            ropes: Vec::new(),
            capes: HashMap::new(),
            clock: 0.0,
            rng: Rng::new(0xF1CE),
            pending: Vec::new(),
            frame: 0,
            camera: V3::default(),
            drops: Vec::new(),
            was_in_water: false,
        }
    }
}

impl Env {
    pub fn clear(&mut self) {
        self.puffs.clear();
        self.blasts.clear();
        self.aired.clear();
        self.emitters.clear();
        self.prints.clear();
        self.last_step = None;
        self.ropes.clear();
        self.capes.clear();
        self.pending.clear();
        self.drops.clear();
    }

    fn puff(&mut self, p: Puff) {
        if self.puffs.len() >= MAX_PUFFS {
            // The oldest smoke makes room.
            if let Some(i) = self.puffs.iter().position(|q| q.kind == FxKind::Smoke) {
                self.puffs.swap_remove(i);
            } else {
                return;
            }
        }
        self.puffs.push(p);
    }

    /// An explosion at `center`: the fireball, sparks and smoke, and the blocks it removed (those
    /// that already turned to air, and those that will within a moment) are blown away.
    pub fn explode(&mut self, center: V3, radius: f64) {
        let r = radius.clamp(0.5, 12.0);
        let rng = &mut self.rng;
        let mut new = Vec::new();
        let flames = (6.0 + r * 3.0) as usize;
        for _ in 0..flames {
            let d = rng.unit();
            let x = center + d * rng.range(0.0, r * 0.45);
            let v = d * rng.range(1.0, 3.0 + r) + v3(0.0, rng.range(0.5, 2.0), 0.0);
            let warm = [1.0, rng.range(0.75, 0.95), rng.range(0.35, 0.55)];
            new.push(Puff::new(FxKind::Flame, x, v, rng.range(0.25, 0.45) * (0.6 + r * 0.15), rng.range(0.35, 0.7), warm, [0.55, 0.12, 0.02], rng));
        }
        let sparks = (16.0 + r * 10.0) as usize;
        for _ in 0..sparks {
            let d = rng.unit();
            let v = d * rng.range(5.0, 9.0 + r * 2.0) + v3(0.0, rng.range(2.0, 6.0), 0.0);
            new.push(Puff::new(FxKind::Spark, center + d * 0.3, v, rng.range(0.025, 0.05), rng.range(0.8, 2.2), [1.0, 0.9, 0.5], [0.9, 0.25, 0.05], rng));
        }
        let smoke = (8.0 + r * 5.0) as usize;
        for _ in 0..smoke {
            let d = rng.unit();
            let x = center + d * rng.range(0.0, r * 0.7);
            let v = d * rng.range(0.5, 2.0) + v3(0.0, rng.range(0.8, 2.2), 0.0);
            let g = rng.range(0.18, 0.32);
            new.push(Puff::new(FxKind::Smoke, x, v, rng.range(0.35, 0.65) * (0.7 + r * 0.1), rng.range(3.0, 6.5), [g * 0.8, g * 0.75, g * 0.7], [0.55, 0.55, 0.56], rng));
        }
        for p in new {
            self.puff(p);
        }
        let blast = Blast { center, radius: r, at: self.clock };
        self.blasts.push(blast);
        // Blocks that went first, then the explosion: they belong to it.
        let now = self.clock;
        let (mine, rest): (Vec<Aired>, Vec<Aired>) = std::mem::take(&mut self.aired)
            .into_iter()
            .partition(|a| now - a.at < 0.6 && in_blast(&blast, a.pos));
        self.aired = rest;
        self.pending.push((blast, mine));
    }

    /// A block at `pos` just turned to air. Inside a fresh blast it is blown away; otherwise it is
    /// remembered for a moment, in case its explosion comes next.
    pub fn aired(&mut self, a: Aired) {
        let now = self.clock;
        self.aired.retain(|x| now - x.at < 0.6);
        if let Some(b) = self.blasts.iter().rev().find(|b| now - b.at < 1.0 && in_blast(b, a.pos)).copied() {
            self.pending.push((b, vec![a]));
            return;
        }
        if self.aired.len() < 4096 {
            self.aired.push(a);
        }
    }

    pub fn step(&mut self, dt: f64, camera: V3, frame: u64, st: &Settings, world: &dyn World) {
        self.clock += dt;
        self.frame = frame;
        self.camera = camera;
        let now = self.clock;
        self.blasts.retain(|b| now - b.at < 2.0);
        let wind = wind_at(now, st.wind());
        let g = st.gravity();
        // Fire, campfires and lava give off smoke.
        if st.smoke && dt > 0.0 {
            self.emitters.retain(|p, _| {
                let d = v3(f64::from(p[0]), f64::from(p[1]), f64::from(p[2])) - camera;
                d.len() < 40.0
            });
            let list: Vec<([i32; 3], u8)> = self.emitters.iter().map(|(p, k)| (*p, *k)).collect();
            for (p, kind) in list {
                // Puffs a second, size, life.
                let (rate, size, life, from) = match kind {
                    1 => (2.5, 0.16, 3.5, [0.2, 0.2, 0.2]),
                    2 => (2.0, 0.15, 3.5, [0.22, 0.25, 0.27]),
                    3 => (3.5, 0.2, 7.0, [0.32, 0.32, 0.32]),
                    4 => (3.0, 0.2, 7.0, [0.3, 0.33, 0.35]),
                    _ => (0.12, 0.14, 3.0, [0.25, 0.22, 0.2]),
                };
                if self.rng.next() > rate * dt {
                    continue;
                }
                let base = v3(f64::from(p[0]), f64::from(p[1]), f64::from(p[2]));
                let x = base + v3(self.rng.range(0.25, 0.75), if kind == 5 { 1.0 } else { 0.6 }, self.rng.range(0.25, 0.75));
                let v = v3(self.rng.range(-0.1, 0.1), self.rng.range(0.8, 1.4), self.rng.range(-0.1, 0.1));
                let p = Puff::new(FxKind::Smoke, x, v, size * self.rng.range(0.8, 1.2), life * self.rng.range(0.8, 1.2), from, [0.62, 0.62, 0.63], &mut self.rng);
                self.puff(p);
            }
        }
        // The puffs.
        let mut i = 0;
        while i < self.puffs.len() {
            let p = &mut self.puffs[i];
            p.age += dt;
            if p.age >= p.life {
                self.puffs.swap_remove(i);
                continue;
            }
            match p.kind {
                FxKind::Smoke => {
                    // Warm smoke rises and slows; the wind takes it, more the higher it gets.
                    let lift = 1.6 * (1.0 - p.age / p.life);
                    let pull = (wind - v3(p.v.x, 0.0, p.v.z)) * (0.6 * dt);
                    p.v = p.v + pull + v3(0.0, lift * dt, 0.0);
                    p.v = p.v * (1.0 - 1.2 * dt).max(0.0);
                    let next = p.x + p.v * dt;
                    if solid(world, next + v3(0.0, p.size, 0.0)) {
                        // Under a ceiling it spreads sideways.
                        p.v = v3(p.v.x * 1.2 + (next.x - next.x.floor() - 0.5) * 0.3, -0.05, p.v.z * 1.2 + (next.z - next.z.floor() - 0.5) * 0.3);
                    } else {
                        p.x = next;
                    }
                }
                FxKind::Flame => {
                    p.v = p.v * (1.0 - 3.0 * dt).max(0.0) + v3(0.0, 1.5 * dt, 0.0);
                    p.x = p.x + p.v * dt;
                }
                FxKind::Spark => {
                    p.v.y -= g * dt;
                    p.v = p.v * (1.0 - 0.4 * dt);
                    let next = p.x + p.v * dt;
                    if solid(world, next) {
                        // Bounce off whichever side it came through.
                        for k in 0..3 {
                            let mut probe = p.x;
                            probe.set(k, next.get(k));
                            if solid(world, probe) {
                                let vk = p.v.get(k);
                                p.v.set(k, -vk * 0.35);
                            }
                        }
                        p.v = v3(p.v.x * 0.7, p.v.y, p.v.z * 0.7);
                    } else {
                        p.x = next;
                    }
                }
            }
            i += 1;
        }
        self.step_drops(dt, g, world);
        // The footprints fade; the ropes and capes nobody draws any more are forgotten.
        let life = 30.0;
        self.prints.retain_mut(|p| {
            p.age += dt;
            p.age < life
        });
        self.ropes.retain(|r| frame.saturating_sub(r.seen) < 30);
        self.capes.retain(|_, c| frame.saturating_sub(c.seen) < 60);
    }

    /// The blasts of this frame throw their blocks. Called with the pieces' physics.
    pub fn blow(&mut self, physics: &mut Physics, st: &Settings) {
        for (blast, blocks) in std::mem::take(&mut self.pending) {
            if blocks.is_empty() {
                continue;
            }
            // Big blasts make fewer pieces per block, so the scene stays light.
            let per = (st.pieces_per_block() / 2).clamp(2, 6);
            let per = if blocks.len() > 120 { (per / 2).max(1) } else { per };
            for a in blocks {
                physics.spawn_blown(&a.boxes, a.pos, a.uv, a.tint, per, &blast, st.lifetime());
            }
        }
    }

    pub fn emit(&self, camera: V3, white: [f32; 4], world: &dyn World, out: &mut Vec<Vertex>) {
        for p in &self.puffs {
            let t = (p.age / p.life).clamp(0.0, 1.0);
            let half = p.size * p.scale();
            if half <= 0.005 {
                continue;
            }
            let color = abgr(mix3(p.from, p.to, t));
            let spin = p.q.mul(Quat::axis_angle(p.w.norm(), p.w.len() * p.age)).normalize();
            let (light, shaded) = match p.kind {
                FxKind::Smoke => (world.light(floor(p.x.x), floor(p.x.y), floor(p.x.z)), true),
                _ => (GLOW, false),
            };
            cube(p.x - camera, half, spin, white, color, light, shaded, out);
        }
        for d in &self.drops {
            let t = (d.age / d.life).clamp(0.0, 1.0);
            let half = d.size * ((1.0 - t) / 0.3).min(1.0);
            if half <= 0.004 {
                continue;
            }
            if d.lava {
                let c = abgr(mix3([1.0, 0.62, 0.15], [0.75, 0.2, 0.02], t));
                cube(d.x - camera, half, Quat::IDENTITY, white, c, GLOW, false, out);
            } else {
                let light = world.light(floor(d.x.x), floor(d.x.y), floor(d.x.z));
                let c = abgr([0.24 + d.shade * 0.1, 0.42 + d.shade * 0.1, 0.86 + d.shade * 0.08]);
                cube(d.x - camera, half, Quat::IDENTITY, white, c, light, true, out);
            }
        }
        for f in &self.prints {
            let t = (f.age / 30.0).clamp(0.0, 1.0);
            // From a pressed, shaded print to the snow around it.
            let color = abgr(mix3([0.7, 0.75, 0.82], [0.95, 0.97, 1.0], t * t));
            let light = world.light(floor(f.x.x), floor(f.x.y + 0.1), floor(f.x.z));
            let (s, c) = f.yaw.sin_cos();
            let fwd = v3(-s, 0.0, c);
            let side = v3(c, 0.0, s);
            let at = f.x - camera;
            let rect = |center: f64, half_l: f64, half_w: f64| -> [V3; 4] {
                let m = at + fwd * center;
                [m - fwd * half_l - side * half_w, m - fwd * half_l + side * half_w, m + fwd * half_l + side * half_w, m + fwd * half_l - side * half_w]
            };
            quad(rect(0.06, 0.1, 0.075), white, color, light, out);
            quad(rect(-0.13, 0.055, 0.065), white, color, light, out);
        }
    }

    /// The player stands at `feet` (yaw in degrees), on snow whose top is `top`, or not on snow.
    pub fn footstep(&mut self, feet: V3, yaw: f64, top: Option<f64>) {
        let Some(top) = top else {
            self.last_step = None;
            return;
        };
        let here = v3(feet.x, top, feet.z);
        let Some(last) = self.last_step else {
            self.last_step = Some(here);
            return;
        };
        let d = v3(here.x - last.x, 0.0, here.z - last.z);
        if d.len() < 0.55 {
            return;
        }
        if d.len() > 3.0 {
            self.last_step = Some(here);
            return;
        }
        self.last_step = Some(here);
        self.left = !self.left;
        let r = yaw.to_radians();
        let (s, c) = r.sin_cos();
        let side = v3(c, 0.0, s) * if self.left { -0.13 } else { 0.13 };
        self.prints.push(Print { x: here + side + v3(0.0, 0.004, 0.0), yaw: r, age: 0.0 });
        if self.prints.len() > 240 {
            self.prints.remove(0);
        }
    }

    /// A rope from `a` to `b` this frame (an id-less match against last frame's ropes of the same
    /// `group`): sags under gravity, sways in the wind, lies on blocks. Returns its points.
    #[allow(clippy::too_many_arguments)]
    pub fn rope(&mut self, group: u8, a: V3, b: V3, slack: f64, dt: f64, g: f64, wind: V3, world: &dyn World) -> Vec<V3> {
        let frame = self.frame;
        let dist = (b - a).len();
        let n = ((dist * 3.0).round() as usize).clamp(8, 28);
        let best = self
            .ropes
            .iter()
            .enumerate()
            .filter(|(_, r)| r.group == group && r.seen != frame + 1 && r.pts.len() == n + 1)
            .map(|(i, r)| (i, (r.pts[0] - a).len() + (r.pts[n] - b).len()))
            .filter(|(_, d)| *d < 2.5)
            .fold(None, |acc: Option<(usize, f64)>, x| if acc.is_none_or(|y| x.1 < y.1) { Some(x) } else { acc });
        let i = match best {
            Some((i, _)) => i,
            None => {
                let pts: Vec<V3> = (0..=n).map(|k| a + (b - a) * (k as f64 / n as f64)).collect();
                self.ropes.push(Rope { prev: pts.clone(), pts, seen: frame + 1, group });
                self.ropes.len() - 1
            }
        };
        let r = &mut self.ropes[i];
        r.seen = frame + 1;
        let rest = dist * (1.0 + slack) / n as f64;
        let steps = 3;
        let h = dt.clamp(0.0, 0.05) / steps as f64;
        for _ in 0..steps {
            for k in 1..n {
                let p = r.pts[k];
                let vel = (p - r.prev[k]) * 0.985;
                r.prev[k] = p;
                r.pts[k] = p + vel + (v3(0.0, -g * 0.6, 0.0) + wind * 0.4) * (h * h);
            }
            r.pts[0] = a;
            r.pts[n] = b;
            for it in 0..8 {
                for k in 0..n {
                    let (p, q) = (r.pts[k], r.pts[k + 1]);
                    let d = q - p;
                    let l = d.len();
                    if l <= rest || l < 1e-9 {
                        continue;
                    }
                    let corr = d * ((l - rest) / l);
                    let (wa, wb) = (if k == 0 { 0.0 } else { 1.0 }, if k + 1 == n { 0.0 } else { 1.0 });
                    let sum = wa + wb;
                    if sum == 0.0 {
                        continue;
                    }
                    r.pts[k] = p + corr * (wa / sum);
                    r.pts[k + 1] = q - corr * (wb / sum);
                }
                // Out of the blocks, every other pass, so the length cannot pull it back in.
                if it % 2 == 1 {
                    for k in 1..n {
                        push_out(world, &mut r.pts[k], &mut r.prev[k]);
                    }
                }
            }
        }
        r.pts.clone()
    }
}

/// Moves point `p` out of the blocks it is in, and takes away its speed into them (`prev` holds
/// last step's place, so the speed is `p - prev`), with some friction along the surface.
fn push_out(world: &dyn World, p: &mut V3, prev: &mut V3) {
    for _ in 0..3 {
        let Some((n, depth)) = penetration(world, *p, *prev, None) else { return };
        *p = *p + n * depth;
        let v = *p - *prev;
        let vn = v.dot(n);
        let along = v - n * vn;
        let v = along * 0.6 + n * vn.max(0.0);
        *prev = *p - v;
    }
}

fn in_blast(b: &Blast, pos: [i32; 3]) -> bool {
    let c = v3(f64::from(pos[0]) + 0.5, f64::from(pos[1]) + 0.5, f64::from(pos[2]) + 0.5);
    (c - b.center).len() <= b.radius * 1.6 + 1.0
}

impl Physics {
    /// A block blown out of an explosion: a few pieces thrown away from its center.
    #[allow(clippy::too_many_arguments)]
    pub fn spawn_blown(&mut self, boxes: &[[f64; 6]], pos: [i32; 3], uv: [f32; 4], tint: u32, per: usize, blast: &Blast, life: f64) {
        let total: f64 = boxes.iter().map(|b| (b[3] - b[0]) * (b[4] - b[1]) * (b[5] - b[2])).sum();
        if total <= 1e-6 {
            return;
        }
        let o = v3(f64::from(pos[0]), f64::from(pos[1]), f64::from(pos[2]));
        let color = tint_color(tint);
        let mut bodies = Vec::new();
        for b in boxes {
            let vol = (b[3] - b[0]) * (b[4] - b[1]) * (b[5] - b[2]);
            let n = ((per as f64 * vol / total).round() as usize).max(1);
            for piece in split_box(*b, n, &mut self.rng) {
                let (c, h, verts) = box_mesh(piece, uv, color);
                let at = o + c;
                let mut body = Body::new(at, Quat::IDENTITY, h, verts);
                let d = at - blast.center;
                let dist = d.len().max(0.3);
                let dir = (d * (1.0 / dist) + self.rng.unit() * 0.35).norm();
                let power = (1.0 - dist / (blast.radius * 1.6 + 1.0)).clamp(0.15, 1.0);
                let speed = (4.0 + blast.radius * 2.2) * power * self.rng.range(0.6, 1.2);
                let up = self.rng.range(2.0, 5.0) * power + speed * 0.3;
                body.v = dir * speed.min(13.0) + v3(0.0, up, 0.0);
                body.w = self.rng.unit() * self.rng.range(3.0, 12.0);
                bodies.push(body);
            }
        }
        let life = life + self.rng.range(0.0, 2.0);
        self.add(bodies, Vec::new(), life, None);
    }

    /// Pushes the pieces (of objects from `from` on) near a blast away from it.
    pub fn blast(&mut self, center: V3, reach: f64, strength: f64, from: usize) {
        for oi in from..self.objects.len() {
            let o = &mut self.objects[oi];
            let mut hit = false;
            for b in &mut self.bodies[o.first..o.first + o.count] {
                let d = b.x - center;
                let l = d.len();
                if l >= reach {
                    continue;
                }
                let k = (1.0 - l / reach) * strength * 12.0;
                b.v = b.v + d.norm() * k + v3(0.0, k * 0.4, 0.0);
                b.w = b.w + self.rng.unit() * k;
                hit = true;
            }
            if hit {
                o.sleeping = false;
                o.still = 0.0;
            }
        }
    }
}

// --- What the hooks call ----------------------------------------------------------------------

/// Whether a part of the mod is on at all (the mod itself and the setting).
fn part_on(f: impl Fn(&Settings) -> bool) -> bool {
    ENABLED.load(Ordering::Relaxed) && f(&settings())
}

pub fn explosions_on() -> bool {
    part_on(|s| s.explosions)
}

pub fn smoke_on() -> bool {
    part_on(|s| s.smoke)
}

pub fn snow_on() -> bool {
    part_on(|s| s.snow)
}

pub fn ropes_on() -> bool {
    part_on(|s| s.ropes)
}

pub fn capes_on() -> bool {
    part_on(|s| s.capes)
}

/// The explosion packet: a blast at `center` of `radius`.
pub fn explosion(center: [f64; 3], radius: f32) {
    if !explosions_on() {
        return;
    }
    with(|s| {
        let c = v3(center[0], center[1], center[2]);
        s.env.explode(c, f64::from(radius));
        // Pieces already lying around are thrown too.
        s.physics.blast(c, f64::from(radius) * 2.5, 0.8, 0);
    });
}

/// A block that just turned to air from the server: its outline (block space), sprite and tint.
pub fn block_aired(pos: [i32; 3], boxes: &[[f64; 6]], uv: [f32; 4], tint: u32) {
    if !explosions_on() || boxes.is_empty() {
        return;
    }
    with(|s| {
        let at = s.env.clock;
        s.env.aired(Aired { pos, boxes: boxes.to_vec(), uv, tint, at })
    });
}

/// Blows away this frame's blocks of explosions. Called from `frame` before the step.
fn blow_pending(s: &mut State, st: &Settings) {
    let State { env, physics, .. } = s;
    env.blow(physics, st);
}

/// Which layer of blocks around the camera the hook should look through for fire this frame:
/// `(y, x0, x1, z0, z1)`, all inclusive. A different layer each frame, 25 layers in turn.
pub fn scan_layer(camera: [f64; 3]) -> (i32, i32, i32, i32, i32) {
    with(|s| {
        s.env.scan = (s.env.scan + 1) % 25;
        let (x, y, z) = (floor(camera[0]), floor(camera[1]), floor(camera[2]));
        (y - 12 + s.env.scan, x - 20, x + 20, z - 20, z + 20)
    })
}

/// What the hook found on layer `y`: fire (1), soul fire (2), a lit campfire (3), a lit soul
/// campfire (4), open lava (5).
pub fn scan_found(y: i32, found: &[([i32; 3], u8)]) {
    with(|s| {
        s.env.emitters.retain(|p, _| p[1] != y);
        for &(p, k) in found.iter().take(400) {
            if s.env.emitters.len() >= 600 {
                break;
            }
            s.env.emitters.insert(p, k);
        }
    });
}

/// The player's feet this frame and the top of the snow they stand on (None: not on snow).
pub fn footstep(feet: [f64; 3], yaw: f32, top: Option<f64>) {
    if !snow_on() {
        return;
    }
    with(|s| s.env.footstep(v3(feet[0], feet[1], feet[2]), f64::from(yaw), top));
}

/// The points of a rope (lead: group 0, fishing line: group 1) between `a` and `b` this frame.
pub fn rope(group: u8, a: [f64; 3], b: [f64; 3], slack: f64, world: &dyn World) -> Vec<[f64; 3]> {
    let st = settings();
    with(|s| {
        let dt = s.last_dt;
        let wind = wind_at(s.env.clock, st.wind());
        let pts = s.env.rope(group, v3(a[0], a[1], a[2]), v3(b[0], b[1], b[2]), slack, dt, st.gravity(), wind, world);
        pts.iter().map(|p| [p.x, p.y, p.z]).collect()
    })
}

/// Leads: the lead geometry the game drew this frame (`drawn`, camera-relative) is replaced by
/// ropes. Each vanilla lead is 384 vertices with zero normals, the knots come after with real
/// normals and are kept as they are.
pub fn leads(drawn: &[Vertex], camera: [f64; 3], world: &dyn World, out: &mut Vec<Vertex>) {
    const PER: usize = 384;
    let cam = v3(camera[0], camera[1], camera[2]);
    let mut i = 0;
    while i < drawn.len() {
        if drawn[i].normal != [0; 4] || i + PER > drawn.len() || drawn[i..i + PER].iter().any(|v| v.normal != [0; 4]) {
            out.push(drawn[i]);
            i += 1;
            continue;
        }
        let lead = &drawn[i..i + PER];
        i += PER;
        let mid = |a: &Vertex, b: &Vertex| v3(f64::from(a.pos[0] + b.pos[0]) * 0.5, f64::from(a.pos[1] + b.pos[1]) * 0.5, f64::from(a.pos[2] + b.pos[2]) * 0.5);
        // Step 0's first corners and step 23's last ones (quad order a0 b0 b1 a1).
        let start = mid(&lead[0], &lead[1]) + cam;
        let end = mid(&lead[23 * 8 + 2], &lead[23 * 8 + 3]) + cam;
        let pts = rope(0, [start.x, start.y, start.z], [end.x, end.y, end.z], 0.07, world);
        let pts: Vec<V3> = pts.iter().map(|p| v3(p[0], p[1], p[2]) - cam).collect();
        let (c0, c1) = (lead[0].color, lead[8].color);
        let (light, uv) = (lead[0].light, lead[0].uv);
        tube(&pts, 0.025, [c0, c1], light, uv, out);
    }
}

/// A square tube of `half` thickness along `pts`, its segments in alternating colours.
fn tube(pts: &[V3], half: f64, colors: [u32; 2], light: u32, uv: [f32; 2], out: &mut Vec<Vertex>) {
    for (k, w) in pts.windows(2).enumerate() {
        let d = (w[1] - w[0]).norm();
        let side = {
            let s = d.cross(v3(0.0, 1.0, 0.0));
            if s.len() < 1e-6 { v3(1.0, 0.0, 0.0) } else { s.norm() }
        };
        let up = side.cross(d).norm();
        let ring = |p: V3| [p + side * half + up * half, p - side * half + up * half, p - side * half - up * half, p + side * half - up * half];
        let (ra, rb) = (ring(w[0]), ring(w[1]));
        let color = colors[k % 2];
        for f in 0..4 {
            let g = (f + 1) % 4;
            for order in [[ra[f], ra[g], rb[g], rb[f]], [rb[f], rb[g], ra[g], ra[f]]] {
                for p in order {
                    out.push(Vertex {
                        pos: [p.x as f32, p.y as f32, p.z as f32],
                        uv,
                        layer: 0,
                        color,
                        light,
                        normal: [0; 4],
                        glint_uv: [0.0, 0.0],
                    });
                }
            }
        }
    }
}

/// A cape as cloth. `rest` is the cape the game would draw hanging straight (camera-relative,
/// the last frame's camera), `pivot` where it hangs from (the neck). Draws the cloth into `out`
/// and returns true, or returns false when the game should draw its own (not a cape we know how
/// to read, or a second drawing of the same player this frame, as in the inventory).
pub fn cape(id: u32, rest: &[Vertex], pivot: [f32; 3], out: &mut Vec<Vertex>) -> bool {
    if !capes_on() || rest.len() < 24 || rest.len() % 4 != 0 {
        return false;
    }
    let st = settings();
    with(|s| {
        let cam = s.camera;
        let dt = s.last_dt;
        let frame = s.frame;
        let wind = wind_at(s.env.clock, st.wind());
        let g = st.gravity();
        let pos = |v: &Vertex| v3(f64::from(v.pos[0]), f64::from(v.pos[1]), f64::from(v.pos[2]));
        // The two biggest faces: the outside (away from the neck) and the inside.
        let mut quads: Vec<(usize, f64, V3)> = rest
            .chunks(4)
            .enumerate()
            .map(|(i, q)| {
                let (a, b, c) = (pos(&q[0]), pos(&q[1]), pos(&q[2]));
                let n = (b - a).cross(c - a);
                let center = (pos(&q[0]) + pos(&q[1]) + pos(&q[2]) + pos(&q[3])) * 0.25;
                (i, n.len(), center)
            })
            .collect();
        quads.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        let piv = v3(f64::from(pivot[0]), f64::from(pivot[1]), f64::from(pivot[2]));
        let (qa, qb) = (quads[0], quads[1]);
        let (outer, inner) = if (qa.2 - piv).len() >= (qb.2 - piv).len() { (qa, qb) } else { (qb, qa) };
        let oq = &rest[outer.0 * 4..outer.0 * 4 + 4];
        let iq = &rest[inner.0 * 4..inner.0 * 4 + 4];
        // The outside's corners: the two nearest the neck are the top.
        let mut idx = [0usize, 1, 2, 3];
        idx.sort_by(|&a, &b| (pos(&oq[a]) - piv).len().partial_cmp(&(pos(&oq[b]) - piv).len()).unwrap_or(std::cmp::Ordering::Equal));
        // The two tops are equally far from the neck; take them in the face's own order, so the
        // cloth's left and right never swap from one frame to the next.
        let (t0, t1) = if (idx[0] + 1) % 4 == idx[1] { (idx[0], idx[1]) } else { (idx[1], idx[0]) };
        // Each top corner's bottom corner is the one next to it around the face.
        let next = |a: usize| if (a + 1) % 4 == t0 || (a + 1) % 4 == t1 { (a + 3) % 4 } else { (a + 1) % 4 };
        let (b0, b1) = (next(t0), next(t1));
        let corner = |k: usize| pos(&oq[k]) + cam;
        let (tl, tr, bl, br) = (corner(t0), corner(t1), corner(b0), corner(b1));
        let width = (tr - tl).len();
        let height = (bl - tl).len();
        if width < 0.05 || height < 0.05 {
            return false;
        }
        let into = (inner.2 - outer.2).norm();
        let grid_rest = |r: usize, c: usize| {
            let (u, v) = (c as f64 / (CAPE_W - 1) as f64, r as f64 / (CAPE_H - 1) as f64);
            let top = tl + (tr - tl) * u;
            let bottom = bl + (br - bl) * u;
            top + (bottom - top) * v
        };
        // A second drawing in the same frame somewhere else (the inventory's player): not ours.
        if let Some(c) = s.env.capes.get(&id)
            && c.seen == frame
            && (c.pts[0] - tl).len() > 1.5
        {
            return false;
        }
        let fresh = s.env.capes.get(&id).is_none_or(|c| (c.pts[0] - tl).len() > 3.0 || c.pts.len() != CAPE_W * CAPE_H);
        if fresh {
            let pts: Vec<V3> = (0..CAPE_H).flat_map(|r| (0..CAPE_W).map(move |c| (r, c))).map(|(r, c)| grid_rest(r, c)).collect();
            s.env.capes.insert(id, Cloth { prev: pts.clone(), pts, seen: 0, width, height });
        }
        let cl = s.env.capes.get_mut(&id).expect("cape just made");
        if cl.seen != frame {
            cl.seen = frame;
            cl.width = width;
            cl.height = height;
            let (dx, dy) = (width / (CAPE_W - 1) as f64, height / (CAPE_H - 1) as f64);
            let steps = 3;
            let h = dt.clamp(0.0, 0.05) / steps as f64;
            let at = |r: usize, c: usize| r * CAPE_W + c;
            let top_mid = (tl + tr) * 0.5;
            for _ in 0..steps {
                for r in 1..CAPE_H {
                    for c in 0..CAPE_W {
                        let k = at(r, c);
                        let p = cl.pts[k];
                        let vel = (p - cl.prev[k]) * 0.96;
                        cl.prev[k] = p;
                        // A cloth catches the wind; a little flutter so it never hangs dead.
                        let flutter = ((self_hash(id) + r as f64 * 0.7 + c as f64 * 0.3) + frame as f64 * 0.21).sin() * 0.25;
                        let force = v3(0.0, -g * 0.5, 0.0) + wind * (1.0 + flutter);
                        cl.pts[k] = p + vel + force * (h * h);
                    }
                }
                for c in 0..CAPE_W {
                    cl.pts[at(0, c)] = grid_rest(0, c);
                }
                for _ in 0..6 {
                    for r in 0..CAPE_H {
                        for c in 0..CAPE_W {
                            if c + 1 < CAPE_W {
                                link(&mut cl.pts, at(r, c), at(r, c + 1), dx, r == 0, r == 0);
                            }
                            if r + 1 < CAPE_H {
                                link(&mut cl.pts, at(r, c), at(r + 1, c), dy, r == 0, false);
                            }
                            if r + 2 < CAPE_H {
                                link(&mut cl.pts, at(r, c), at(r + 2, c), dy * 2.0, r == 0, false);
                            }
                            if r + 1 < CAPE_H && c + 1 < CAPE_W {
                                let diag = (dx * dx + dy * dy).sqrt();
                                link(&mut cl.pts, at(r, c), at(r + 1, c + 1), diag, r == 0, false);
                                link(&mut cl.pts, at(r, c + 1), at(r + 1, c), diag, r == 0, false);
                            }
                        }
                    }
                    // Not through the player's back.
                    for k in CAPE_W..cl.pts.len() {
                        let d = (cl.pts[k] - top_mid).dot(into);
                        if d > -0.01 {
                            cl.pts[k] = cl.pts[k] - into * (d + 0.01);
                        }
                    }
                }
            }
        }
        // Draw: the outside with the outer face's texture, the inside with the inner's.
        let uv_of = |q: &[Vertex], k: usize| [f64::from(q[k].uv[0]), f64::from(q[k].uv[1])];
        let near = |p: V3| {
            (0..4)
                .min_by(|&a, &b| (pos(&iq[a]) - p).len().partial_cmp(&(pos(&iq[b]) - p).len()).unwrap_or(std::cmp::Ordering::Equal))
                .unwrap_or(0)
        };
        let o_uv = [uv_of(oq, t0), uv_of(oq, t1), uv_of(oq, b0), uv_of(oq, b1)];
        let i_uv = [
            uv_of(iq, near(pos(&oq[t0]))),
            uv_of(iq, near(pos(&oq[t1]))),
            uv_of(iq, near(pos(&oq[b0]))),
            uv_of(iq, near(pos(&oq[b1]))),
        ];
        let bilerp = |uv: &[[f64; 2]; 4], r: usize, c: usize| -> [f32; 2] {
            let (u, v) = (c as f64 / (CAPE_W - 1) as f64, r as f64 / (CAPE_H - 1) as f64);
            let top = [uv[0][0] + (uv[1][0] - uv[0][0]) * u, uv[0][1] + (uv[1][1] - uv[0][1]) * u];
            let bot = [uv[2][0] + (uv[3][0] - uv[2][0]) * u, uv[2][1] + (uv[3][1] - uv[2][1]) * u];
            [(top[0] + (bot[0] - top[0]) * v) as f32, (top[1] + (bot[1] - top[1]) * v) as f32]
        };
        let base = rest[0];
        let pts = &cl.pts;
        let at = |r: usize, c: usize| pts[r * CAPE_W + c];
        // The cloth's normal at each point, pointing outward (away from the back).
        let normal = |r: usize, c: usize| {
            let du = at(r, (c + 1).min(CAPE_W - 1)) - at(r, c.saturating_sub(1));
            let dv = at((r + 1).min(CAPE_H - 1), c) - at(r.saturating_sub(1), c);
            let n = du.cross(dv).norm();
            if n.dot(into) > 0.0 { -n } else { n }
        };
        let pack = |x: f64| (x.clamp(-1.0, 1.0) * 127.0) as i8;
        let thick = 0.06;
        let mut push = |p: V3, n: V3, uv: [f32; 2]| {
            let p = p - cam;
            out.push(Vertex {
                pos: [p.x as f32, p.y as f32, p.z as f32],
                uv,
                layer: base.layer,
                color: base.color,
                light: base.light,
                normal: [pack(n.x), pack(n.y), pack(n.z), base.normal[3]],
                glint_uv: base.glint_uv,
            });
        };
        for r in 0..CAPE_H - 1 {
            for c in 0..CAPE_W - 1 {
                let cells = [(r, c), (r, c + 1), (r + 1, c + 1), (r + 1, c)];
                // Outward winding: check against the normal.
                let (p0, p1, p2) = (at(r, c), at(r, c + 1), at(r + 1, c + 1));
                let n0 = normal(r, c);
                let out_order = if (p1 - p0).cross(p2 - p0).dot(n0) >= 0.0 { [0, 1, 2, 3] } else { [3, 2, 1, 0] };
                for k in out_order {
                    let (rr, cc) = cells[k];
                    push(at(rr, cc), normal(rr, cc), bilerp(&o_uv, rr, cc));
                }
                let in_order = [out_order[3], out_order[2], out_order[1], out_order[0]];
                for k in in_order {
                    let (rr, cc) = cells[k];
                    let n = normal(rr, cc);
                    push(at(rr, cc) - n * thick, -n, bilerp(&i_uv, rr, cc));
                }
            }
        }
        true
    })
}

fn self_hash(id: u32) -> f64 {
    f64::from(id.wrapping_mul(2_654_435_761) % 1000) * 0.01
}

/// Keeps points `a` and `b` no further apart than `len` (cloth stretches little and folds freely).
fn link(pts: &mut [V3], a: usize, b: usize, len: f64, a_pinned: bool, b_pinned: bool) {
    let d = pts[b] - pts[a];
    let l = d.len();
    if l < 1e-9 {
        return;
    }
    let (wa, wb) = (if a_pinned { 0.0 } else { 1.0 }, if b_pinned { 0.0 } else { 1.0 });
    let sum = wa + wb;
    if sum == 0.0 {
        return;
    }
    // Pulled back hard when stretched, softly when squeezed.
    let k = if l > len { 1.0 } else { 0.15 };
    let corr = d * ((l - len) / l * k);
    pts[a] = pts[a] + corr * (wa / sum);
    pts[b] = pts[b] - corr * (wb / sum);
}


// --- Liquids (beta) -------------------------------------------------------------------------

/// A drop of water or lava: a tiny sphere that falls, flows over blocks and keeps a little apart
/// from the others (position-based fluid, much simplified).
#[derive(Clone, Copy, Debug)]
pub struct Drop {
    pub x: V3,
    pub v: V3,
    pub age: f64,
    pub life: f64,
    pub size: f64,
    pub lava: bool,
    shade: f64,
}

const MAX_DROPS: usize = 1200;
/// How close two drops may come.
const DROP_GAP: f64 = 0.12;

impl Env {
    fn add_drop(&mut self, x: V3, v: V3, lava: bool, life: f64) {
        if self.drops.len() >= MAX_DROPS {
            self.drops.remove(0);
        }
        let size = self.rng.range(0.035, 0.06);
        let shade = self.rng.next();
        self.drops.push(Drop { x, v, age: 0.0, life, size, lava, shade });
    }

    /// Water or lava was poured into the block at `pos`: a blob of it spills out over the block
    /// and its neighbours before it settles into the block.
    pub fn pour(&mut self, pos: [i32; 3], lava: bool) {
        let c = v3(f64::from(pos[0]) + 0.5, f64::from(pos[1]) + 0.75, f64::from(pos[2]) + 0.5);
        let n = if lava { 40 } else { 70 };
        for _ in 0..n {
            let d = self.rng.unit();
            let x = c + v3(d.x * 0.3, d.y.abs() * 0.2, d.z * 0.3);
            let v = v3(d.x * self.rng.range(0.4, 1.5), self.rng.range(-0.5, 1.0), d.z * self.rng.range(0.4, 1.5));
            let life = self.rng.range(1.6, 3.2) * if lava { 1.4 } else { 1.0 };
            self.add_drop(x, v, lava, life);
        }
    }

    /// Something fell into water at `at` with downward `speed` (blocks a second): a splash.
    pub fn splash(&mut self, at: V3, speed: f64) {
        let n = (12.0 + speed * 18.0).min(80.0) as usize;
        for _ in 0..n {
            let a = self.rng.range(0.0, std::f64::consts::TAU);
            let r = self.rng.range(0.2, 0.55);
            let x = at + v3(a.cos() * r, 0.05, a.sin() * r);
            let up = self.rng.range(2.0, 3.5) * (0.5 + speed * 0.12).min(1.4);
            let v = v3(a.cos() * self.rng.range(0.8, 2.0), up, a.sin() * self.rng.range(0.8, 2.0));
            let life = self.rng.range(1.0, 1.8);
            self.add_drop(x, v, false, life);
        }
    }

    /// The player's feet this frame: in water or not, and how fast they move down.
    pub fn player_in_water(&mut self, feet: V3, in_water: bool, down: f64) {
        if in_water && !self.was_in_water && down > 3.0 {
            self.splash(v3(feet.x, feet.y.floor() + 0.9, feet.z), down);
        }
        self.was_in_water = in_water;
    }

    fn step_drops(&mut self, dt: f64, g: f64, world: &dyn World) {
        if self.drops.is_empty() || dt <= 0.0 {
            return;
        }
        let steps = 2;
        let h = dt.min(0.05) / steps as f64;
        for _ in 0..steps {
            let mut prev: Vec<V3> = Vec::with_capacity(self.drops.len());
            for d in &mut self.drops {
                prev.push(d.x);
                d.v.y -= g * h;
                d.x = d.x + d.v * h;
            }
            // Keep apart: a grid of cells one gap wide, each drop against its neighbours.
            let cell = |p: V3| ((p.x / DROP_GAP).floor() as i64, (p.y / DROP_GAP).floor() as i64, (p.z / DROP_GAP).floor() as i64);
            let mut grid: HashMap<(i64, i64, i64), Vec<usize>> = HashMap::new();
            for (i, d) in self.drops.iter().enumerate() {
                grid.entry(cell(d.x)).or_default().push(i);
            }
            let mut push = vec![V3::default(); self.drops.len()];
            for (i, d) in self.drops.iter().enumerate() {
                let (cx, cy, cz) = cell(d.x);
                for dx in -1..=1 {
                    for dy in -1..=1 {
                        for dz in -1..=1 {
                            let Some(list) = grid.get(&(cx + dx, cy + dy, cz + dz)) else { continue };
                            for &j in list {
                                if j <= i {
                                    continue;
                                }
                                let e = self.drops[j].x - d.x;
                                let l = e.len();
                                if l >= DROP_GAP || l < 1e-9 {
                                    continue;
                                }
                                let corr = e * ((DROP_GAP - l) / l * 0.5 * 0.5);
                                push[i] = push[i] - corr;
                                push[j] = push[j] + corr;
                            }
                        }
                    }
                }
            }
            for (k, d) in self.drops.iter_mut().enumerate() {
                d.x = d.x + push[k];
                let mut p = prev[k];
                let before = d.x;
                push_out(world, &mut d.x, &mut p);
                let touched = (d.x - before).len() > 1e-9;
                d.v = (d.x - prev[k]) * (1.0 / h);
                if touched {
                    // Water runs on along the ground; it barely bounces.
                    d.v = v3(d.v.x * 0.94, d.v.y.max(0.0) * 0.1, d.v.z * 0.94);
                }
            }
        }
        let mut i = 0;
        while i < self.drops.len() {
            let d = &mut self.drops[i];
            d.age += dt;
            // A drop that reaches standing water joins it (after its first moment: the block it
            // was poured into is water itself).
            let joined = d.age > 0.5 && !d.lava && matches!(world.cell(floor(d.x.x), floor(d.x.y), floor(d.x.z)), Cell::Fluid);
            if d.age >= d.life || joined || d.x.y < -100.0 {
                self.drops.swap_remove(i);
            } else {
                i += 1;
            }
        }
    }
}

pub fn liquids_on() -> bool {
    part_on(|s| s.liquids)
}

/// Water (or lava) was placed at `pos`.
pub fn pour(pos: [i32; 3], lava: bool) {
    if !liquids_on() {
        return;
    }
    with(|s| s.env.pour(pos, lava));
}

/// The player's feet: in water or not, and their speed down (blocks a second).
pub fn player_water(feet: [f64; 3], in_water: bool, down: f64) {
    if !liquids_on() {
        return;
    }
    with(|s| s.env.player_in_water(v3(feet[0], feet[1], feet[2]), in_water, down));
}
