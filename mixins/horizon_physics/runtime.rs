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
pub const VERSION: &str = "0.1.0";
pub const DESCRIPTION: &str =
    "Blocks crumble into pieces, mobs fall as ragdolls or apart, items lie flat. Client-side.";

// ---------------------------------------------------------------------------------------------
// Settings
// ---------------------------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Settings {
    pub blocks: bool,
    /// 0 off, 1 ragdoll, 2 fall apart, 3 mixed.
    pub deaths: u8,
    pub items: bool,
    /// 0 Earth, 1 Moon, 2 Mars, 3 Pluto, 4 zero.
    pub gravity: u8,
    /// 0 few, 1 normal, 2 many.
    pub pieces: u8,
    /// 0 10 s, 1 30 s, 2 60 s, 3 5 min.
    pub lifetime: u8,
}

impl Default for Settings {
    fn default() -> Self {
        Self { blocks: true, deaths: 1, items: true, gravity: 0, pieces: 1, lifetime: 1 }
    }
}

const DEATHS: [&str; 4] = ["Off", "Ragdoll", "Fall apart", "Mixed"];
const GRAVITY: [(&str, f64); 5] =
    [("Earth", 1.0), ("Moon", 0.165), ("Mars", 0.38), ("Pluto", 0.063), ("Zero", 0.0)];
const PIECES: [(&str, usize); 3] = [("Few", 4), ("Normal", 8), ("Many", 16)];
const LIFETIME: [(&str, f64); 4] = [("10 s", 10.0), ("30 s", 30.0), ("60 s", 60.0), ("5 min", 300.0)];
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
            3 => self.gravity = (self.gravity + 1) % 5,
            4 => self.pieces = (self.pieces + 1) % 3,
            5 => self.lifetime = (self.lifetime + 1) % 4,
            6 => return true,
            _ => {}
        }
        false
    }

    pub fn to_text(&self) -> String {
        format!(
            "blocks={}\ndeaths={}\nitems={}\ngravity={}\npieces={}\nlifetime={}\n",
            self.blocks as u8, self.deaths, self.items as u8, self.gravity, self.pieces, self.lifetime
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
        let inside = (0..3).all(|k| p.get(k) > lo.get(k) && p.get(k) < hi.get(k));
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
const MAX_BODIES: usize = 900;

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
    });
    f(s)
}

pub fn clear() {
    with(|s| {
        s.physics.clear();
        s.items.clear();
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
    with(|s| s.physics.spawn_mob(verts, &marks, cam, vel, st.deaths, st.lifetime()));
}

/// Once a frame: step the pieces and draw them. `dim` tells dimensions apart; a change, or a big
/// jump of the camera (a teleport, another world), clears them.
pub fn frame(dim: u8, camera: [f64; 3], world: &dyn World, out: &mut Vec<Vertex>) {
    let st = settings();
    with(|s| {
        let cam = v3(camera[0], camera[1], camera[2]);
        if s.dim != dim || (cam - s.camera).len() > 300.0 {
            s.physics.clear();
            s.dead.clear();
            s.items.clear();
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
        s.physics.step(dt, st.gravity(), world);
        s.physics.emit(cam, world, out);
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

