use crate::mesher::Vertex;
use crate::native_horizon_physics::*;

/// A flat world: stone up to y = 63 (the top of the floor is y = 64), with an optional pillar.
struct Flat {
    pillar: Option<[i32; 3]>,
    water_above: bool,
}

impl World for Flat {
    fn cell(&self, x: i32, y: i32, z: i32) -> Cell {
        if y < 64 {
            return Cell::Full;
        }
        if self.pillar == Some([x, y, z]) {
            return Cell::Full;
        }
        if self.water_above && y < 70 {
            return Cell::Fluid;
        }
        Cell::Air
    }
    fn light(&self, _: i32, _: i32, _: i32) -> u32 {
        15 * 16
    }
}

fn run(p: &mut Physics, w: &dyn World, seconds: f64) {
    let steps = (seconds * 60.0) as usize;
    for _ in 0..steps {
        p.step(1.0 / 60.0, 20.0, w);
    }
}

fn cube_verts() -> Vec<MeshVertex> {
    Vec::new()
}

#[test]
fn a_dropped_box_rests_on_the_floor() {
    let w = Flat { pillar: None, water_above: false };
    let mut p = Physics::default();
    let mut b = Body::new(v3(0.5, 70.0, 0.5), Quat::axis_angle(v3(1.0, 0.3, 0.2), 0.7), v3(0.2, 0.2, 0.2), cube_verts());
    b.v = v3(1.0, 0.0, 0.0);
    p.add(vec![b], Vec::new(), 100.0, None);
    run(&mut p, &w, 6.0);
    let b = &p.bodies[0];
    // Resting on a face: the centre sits half an edge above the floor.
    assert!((b.x.y - 64.2).abs() < 0.03, "y = {}", b.x.y);
    assert!(b.v.len() < 0.05, "still moving: {:?}", b.v);
    assert!(p.objects[0].sleeping, "should have gone to sleep");
}

#[test]
fn a_fast_box_does_not_tunnel_into_the_ground() {
    let w = Flat { pillar: None, water_above: false };
    let mut p = Physics::default();
    let mut b = Body::new(v3(0.5, 66.0, 0.5), Quat::IDENTITY, v3(0.1, 0.1, 0.1), cube_verts());
    b.v = v3(0.0, -25.0, 0.0);
    p.add(vec![b], Vec::new(), 100.0, None);
    run(&mut p, &w, 3.0);
    assert!(p.bodies[0].x.y > 63.95, "y = {}", p.bodies[0].x.y);
}

#[test]
fn a_crumbling_block_leaves_a_pile_on_the_floor() {
    // The block itself is gone (air) at 0 64 0; the pieces start inside its cell.
    let w = Flat { pillar: None, water_above: false };
    let mut p = Physics::default();
    let s = Settings::default();
    p.spawn_block([0, 64, 0], &[[0.0, 0.0, 0.0, 1.0, 1.0, 1.0]], [0.0, 0.0, 0.1, 0.1], 0xFFFFFF, &s);
    assert_eq!(p.bodies.len(), 8);
    run(&mut p, &w, 8.0);
    for b in &p.bodies {
        assert!(b.x.y > 64.0 && b.x.y < 65.2, "piece at {:?}", b.x);
        assert!((b.x.x - 0.5).abs() < 3.0 && (b.x.z - 0.5).abs() < 3.0, "flew off: {:?}", b.x);
    }
    // Pieces do not sink into each other much: their centres keep apart.
    for i in 0..p.bodies.len() {
        for j in i + 1..p.bodies.len() {
            let d = (p.bodies[i].x - p.bodies[j].x).len();
            assert!(d > 0.05, "pieces {i} and {j} merged");
        }
    }
}

#[test]
fn pieces_inside_their_still_standing_block_are_not_shot_out() {
    // The server's update has not arrived yet: the cell is still stone for a moment.
    let w = Flat { pillar: Some([0, 64, 0]), water_above: false };
    let mut p = Physics::default();
    p.spawn_block([0, 64, 0], &[[0.0, 0.0, 0.0, 1.0, 1.0, 1.0]], [0.0; 4], 0xFFFFFF, &Settings::default());
    run(&mut p, &w, 0.3);
    for b in &p.bodies {
        assert!(b.v.len() < 10.0, "piece shot out at {:?}", b.v);
    }
}

#[test]
fn split_box_keeps_the_volume() {
    let mut r = Rng::new(3);
    for n in [1, 4, 8, 16] {
        let parts = split_box([0.0, 0.0, 0.0, 1.0, 0.5, 1.0], n, &mut r);
        assert_eq!(parts.len(), n);
        let vol: f64 = parts.iter().map(|b| (b[3] - b[0]) * (b[4] - b[1]) * (b[5] - b[2])).sum();
        assert!((vol - 0.5).abs() < 1e-9);
    }
}

fn quad(out: &mut Vec<Vertex>, c: [f32; 3], n: [f32; 3], u: [f32; 3], v: [f32; 3]) {
    for (a, b) in [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
        out.push(Vertex {
            pos: [c[0] + u[0] * a + v[0] * b, c[1] + u[1] * a + v[1] * b, c[2] + u[2] * a + v[2] * b],
            uv: [0.0, 0.0],
            layer: 1,
            color: 0xFFFF_FFFF,
            light: 0,
            normal: [(n[0] * 127.0) as i8, (n[1] * 127.0) as i8, (n[2] * 127.0) as i8, -127],
            glint_uv: [0.0; 2],
        });
    }
}

/// A box as a model part draws it: six quads.
fn part(out: &mut Vec<Vertex>, c: [f32; 3], h: [f32; 3]) {
    let [x, y, z] = c;
    let [a, b, d] = h;
    quad(out, [x + a, y, z], [1.0, 0.0, 0.0], [0.0, 0.0, d], [0.0, b, 0.0]);
    quad(out, [x - a, y, z], [-1.0, 0.0, 0.0], [0.0, 0.0, d], [0.0, b, 0.0]);
    quad(out, [x, y + b, z], [0.0, 1.0, 0.0], [a, 0.0, 0.0], [0.0, 0.0, d]);
    quad(out, [x, y - b, z], [0.0, -1.0, 0.0], [a, 0.0, 0.0], [0.0, 0.0, d]);
    quad(out, [x, y, z + d], [0.0, 0.0, 1.0], [a, 0.0, 0.0], [0.0, b, 0.0]);
    quad(out, [x, y, z - d], [0.0, 0.0, -1.0], [a, 0.0, 0.0], [0.0, b, 0.0]);
}

/// A zombie-like model, camera-relative with the camera at (0, 65.6, -4): head, body, two arms,
/// two legs, and a head overlay drawn over the head.
fn humanoid() -> (Vec<Vertex>, Vec<usize>) {
    let mut v = Vec::new();
    let mut marks = Vec::new();
    let base = -1.6f32; // feet at y = 64
    let parts: [([f32; 3], [f32; 3]); 7] = [
        ([0.0, base + 1.75, 4.0], [0.25, 0.25, 0.25]),  // head
        ([0.0, base + 1.75, 4.0], [0.28, 0.28, 0.28]),  // hat layer
        ([0.0, base + 1.125, 4.0], [0.25, 0.375, 0.125]), // body
        ([0.375, base + 1.125, 4.0], [0.125, 0.375, 0.125]),
        ([-0.375, base + 1.125, 4.0], [0.125, 0.375, 0.125]),
        ([0.125, base + 0.375, 4.0], [0.125, 0.375, 0.125]),
        ([-0.125, base + 0.375, 4.0], [0.125, 0.375, 0.125]),
    ];
    for (c, h) in parts {
        marks.push(v.len());
        part(&mut v, c, h);
    }
    (v, marks)
}

#[test]
fn a_ragdoll_falls_and_stays_together() {
    let w = Flat { pillar: None, water_above: false };
    let mut p = Physics::default();
    let (verts, marks) = humanoid();
    p.spawn_mob(&verts, &marks, v3(0.0, 65.6, -4.0), V3::default(), 1, 100.0);
    // Head and its overlay are one body: six parts.
    assert_eq!(p.bodies.len(), 6);
    assert_eq!(p.objects[0].joints.len(), 5);
    // The hurt overlay is gone from the pieces.
    assert!(p.bodies.iter().all(|b| b.verts.iter().all(|v| v.layer == 0)));
    run(&mut p, &w, 6.0);
    for b in &p.bodies {
        assert!(b.x.y > 64.0 && b.x.y < 65.0, "part at {:?}", b.x);
    }
    // The joints hold: each joint's two anchors are still close together.
    let o = &p.objects[0];
    for j in &o.joints {
        let (a, b) = (&p.bodies[j.a], &p.bodies[j.b]);
        let gap = (a.x + a.q.rotate(j.ra)) - (b.x + b.q.rotate(j.rb));
        assert!(gap.len() < 0.12, "joint {j:?} opened by {}", gap.len());
    }
    // It went over: the head is no longer above the legs.
    let head = p.bodies.iter().map(|b| b.x.y).fold(f64::MIN, f64::max);
    assert!(head < 64.7, "still standing, head at {head}");
}

#[test]
fn a_mob_that_falls_apart_scatters() {
    let w = Flat { pillar: None, water_above: false };
    let mut p = Physics::default();
    let (verts, marks) = humanoid();
    p.spawn_mob(&verts, &marks, v3(0.0, 65.6, -4.0), V3::default(), 2, 100.0);
    assert!(p.objects[0].joints.is_empty());
    run(&mut p, &w, 5.0);
    let xs: Vec<f64> = p.bodies.iter().map(|b| b.x.x).collect();
    let spread = xs.iter().cloned().fold(f64::MIN, f64::max) - xs.iter().cloned().fold(f64::MAX, f64::min);
    assert!(spread > 0.8, "spread {spread}");
    for b in &p.bodies {
        assert!(b.x.y > 63.95, "part fell through: {:?}", b.x);
    }
}

#[test]
fn emitted_vertices_follow_the_pieces() {
    let w = Flat { pillar: None, water_above: false };
    let mut p = Physics::default();
    let (verts, marks) = humanoid();
    let cam = v3(0.0, 65.6, -4.0);
    p.spawn_mob(&verts, &marks, cam, V3::default(), 1, 100.0);
    let mut out = Vec::new();
    p.emit(cam, &w, &mut out);
    assert_eq!(out.len(), verts.len());
    // Before any step the pieces sit exactly where the model was drawn.
    let mut a: Vec<[i32; 3]> = verts.iter().map(|v| v.pos.map(|x| (x * 1000.0).round() as i32)).collect();
    let mut b: Vec<[i32; 3]> = out.iter().map(|v| v.pos.map(|x| (x * 1000.0).round() as i32)).collect();
    a.sort();
    b.sort();
    let off = a.iter().zip(&b).filter(|(x, y)| (0..3).any(|k| (x[k] - y[k]).abs() > 2)).count();
    assert_eq!(off, 0);
}

#[test]
fn pieces_expire() {
    let w = Flat { pillar: None, water_above: false };
    let mut p = Physics::default();
    let b = Body::new(v3(0.5, 64.2, 0.5), Quat::IDENTITY, v3(0.2, 0.2, 0.2), Vec::new());
    p.add(vec![b], Vec::new(), 1.0, None);
    run(&mut p, &w, 1.2);
    assert!(p.is_empty());
}

#[test]
fn pieces_sink_slowly_in_water() {
    let w = Flat { pillar: None, water_above: true };
    let mut p = Physics::default();
    let b = Body::new(v3(0.5, 69.0, 0.5), Quat::IDENTITY, v3(0.1, 0.1, 0.1), Vec::new());
    p.add(vec![b], Vec::new(), 100.0, None);
    run(&mut p, &w, 1.0);
    assert!(p.bodies[0].x.y > 67.0, "sank too fast: {}", p.bodies[0].x.y);
}

#[test]
fn settings_round_trip_and_cycle() {
    let mut s = Settings::default();
    s.click(1);
    s.click(12);
    s.click(0);
    s.click(4);
    s.click(11);
    s.click(10);
    let t = Settings::from_text(&s.to_text());
    assert_eq!(s, t);
    assert_eq!(t.labels().len(), 20);
    assert!(t.labels()[1].ends_with("Blocky"));
    assert!(t.labels()[4].ends_with("Off"));
    assert!(t.labels()[11].ends_with("Strong"));
    assert!(!t.smoke && t.explosions && !t.liquids && t.waves && t.plants);
    assert!(s.click(15));
}

#[test]
fn quaternion_from_axes_matches_the_axes() {
    let a = v3(0.0, 0.0, 1.0);
    let b = v3(1.0, 0.0, 0.0);
    let c = a.cross(b);
    let q = Quat::from_axes(a, b, c);
    assert!((q.rotate(v3(1.0, 0.0, 0.0)) - a).len() < 1e-9);
    assert!((q.rotate(v3(0.0, 1.0, 0.0)) - b).len() < 1e-9);
}

#[test]
fn a_flat_item_ends_face_up() {
    // Tumble in the air for a while, then land.
    for _ in 0..30 {
        item_rotation(7, false, [0.3, -0.5, 0.1], true);
    }
    let mut q = [0.0; 4];
    for _ in 0..200 {
        q = item_rotation(7, true, [0.0, 0.0, 0.0], true);
    }
    let q = Quat { x: q[0].into(), y: q[1].into(), z: q[2].into(), w: q[3].into() };
    let face = q.rotate(v3(0.0, 0.0, 1.0));
    assert!(face.y.abs() > 0.99, "face normal {face:?}");
}

/// Writes the boxes of three scenes over time, for a picture of how they move (not a check).
#[test]
#[ignore]
fn dump_scenes() {
    let w = Flat { pillar: None, water_above: false };
    let mut out = String::from("[");
    for (si, mode) in [(0, 1u8), (1, 2u8), (2, 0u8)] {
        let mut p = Physics::default();
        if mode == 0 {
            let s = Settings { pieces: 2, ..Settings::default() };
            p.spawn_block([0, 65, 4], &[[0.0, 0.0, 0.0, 1.0, 1.0, 1.0]], [0.0; 4], 0, &s);
            p.add(vec![Body::new(v3(0.5, 64.5, 4.5), Quat::IDENTITY, v3(0.5, 0.5, 0.5), Vec::new())], Vec::new(), 1e9, None);
        } else {
            let (verts, marks) = humanoid();
            p.spawn_mob(&verts, &marks, v3(0.0, 65.6, -4.0), v3(0.0, 0.0, 1.0), mode, 100.0);
        }
        if si > 0 {
            out.push(',');
        }
        out.push('[');
        for f in 0..90 {
            if f > 0 {
                out.push(',');
            }
            out.push('[');
            for (i, b) in p.bodies.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                let c: Vec<String> = (0..8)
                    .map(|k| {
                        let h = b.half;
                        let l = v3(if k & 1 == 0 { -h.x } else { h.x }, if k & 2 == 0 { -h.y } else { h.y }, if k & 4 == 0 { -h.z } else { h.z });
                        let p = b.x + b.q.rotate(l);
                        format!("[{:.3},{:.3},{:.3}]", p.x, p.y, p.z)
                    })
                    .collect();
                out.push_str(&format!("[{}]", c.join(",")));
            }
            out.push(']');
            for _ in 0..2 {
                p.step(1.0 / 60.0, 20.0, &w);
            }
        }
        out.push(']');
    }
    out.push(']');
    std::fs::write("/tmp/hp_dump.json", out).unwrap();
}

// --- 0.2.0 ----------------------------------------------------------------------------------

fn stone(pos: [i32; 3], at: f64) -> Aired {
    Aired { pos, boxes: vec![[0.0, 0.0, 0.0, 1.0, 1.0, 1.0]], uv: [0.0, 0.0, 0.1, 0.1], tint: 0xFFFFFF, at }
}

#[test]
fn an_explosion_throws_its_blocks_outward() {
    let w = Flat { pillar: None, water_above: false };
    let mut env = Env::default();
    let mut p = Physics::default();
    let st = Settings::default();
    let center = v3(0.5, 64.5, 0.5);
    // Two blocks went to air just before the packet, one after; one far away is not the blast's.
    env.aired(stone([2, 64, 0], 0.0));
    env.aired(stone([40, 64, 0], 0.0));
    env.explode(center, 4.0);
    env.aired(stone([-2, 64, 1], 0.0));
    assert!(!env.puffs.is_empty(), "no fireball, sparks or smoke");
    env.blow(&mut p, &st);
    assert_eq!(p.objects.len(), 2, "two blocks belong to the blast");
    for b in &p.bodies {
        let out = v3(b.x.x - center.x, 0.0, b.x.z - center.z).norm();
        assert!(b.v.dot(out) > 1.0, "a piece is not thrown outward: {:?}", b.v);
        assert!(b.v.y > -1.0, "thrown down: {:?}", b.v);
    }
    let up = p.bodies.iter().map(|b| b.v.y).sum::<f64>() / p.bodies.len() as f64;
    assert!(up > 1.0, "not thrown up: {up}");
    // They land and stay above the floor.
    run(&mut p, &w, 5.0);
    for b in &p.bodies {
        assert!(b.x.y > 63.9, "fell through: {}", b.x.y);
    }
    // Far from the start.
    assert!(p.bodies.iter().any(|b| (b.x - center).len() > 3.0));
}

#[test]
fn smoke_rises_and_drifts_with_the_wind_sparks_bounce() {
    let w = Flat { pillar: None, water_above: false };
    let mut env = Env::default();
    let st = Settings { wind_clear: 2.4, ..Settings::default() };
    env.explode(v3(0.5, 65.0, 0.5), 3.0);
    let start: Vec<Puff> = env.puffs.clone();
    let mut t = 0.0;
    while t < 2.0 {
        env.step(1.0 / 60.0, v3(0.5, 66.0, 0.5), 1, &st, &w);
        t += 1.0 / 60.0;
    }
    let smoke: Vec<&Puff> = env.puffs.iter().filter(|p| p.kind == FxKind::Smoke).collect();
    assert!(!smoke.is_empty());
    let avg_y = smoke.iter().map(|p| p.x.y).sum::<f64>() / smoke.len() as f64;
    let avg_y0 = start.iter().filter(|p| p.kind == FxKind::Smoke).map(|p| p.x.y).sum::<f64>()
        / start.iter().filter(|p| p.kind == FxKind::Smoke).count() as f64;
    assert!(avg_y > avg_y0 + 0.8, "smoke did not rise: {avg_y0} -> {avg_y}");
    let wind = wind_at(env.clock, st.wind()).norm();
    let drift = smoke.iter().map(|p| v3(p.v.x, 0.0, p.v.z).dot(wind)).sum::<f64>() / smoke.len() as f64;
    assert!(drift > 0.2, "smoke does not move with the wind: {drift}");
    for p in env.puffs.iter().filter(|p| p.kind == FxKind::Spark) {
        assert!(p.x.y > 63.95, "a spark went through the floor: {}", p.x.y);
    }
    // Everything is gone after its life.
    for _ in 0..600 {
        env.step(1.0 / 60.0, v3(0.5, 66.0, 0.5), 1, &st, &w);
    }
    assert!(env.puffs.is_empty());
}

#[test]
fn a_rope_sags_lies_on_the_ground_and_keeps_its_length() {
    let w = Flat { pillar: None, water_above: false };
    let mut env = Env::default();
    let (a, b) = (v3(0.0, 65.0, 0.0), v3(6.0, 65.0, 0.0));
    let mut pts = Vec::new();
    for f in 0..240 {
        env.step(1.0 / 60.0, a, f, &Settings::default(), &w);
        pts = env.rope(0, a, b, 0.3, 1.0 / 60.0, 20.0, V3::default(), &w);
    }
    assert_eq!(env.ropes.len(), 1, "the rope is found again each frame");
    let low = pts.iter().map(|p| p.y).fold(f64::MAX, f64::min);
    assert!(low < 64.6, "no sag: {low}");
    assert!(low > 63.95, "through the floor: {low} {:?}", pts.iter().map(|p| (p.x, p.y)).collect::<Vec<_>>());
    let len: f64 = pts.windows(2).map(|w| (w[1] - w[0]).len()).sum();
    assert!(len < 6.0 * 1.3 + 0.2 && len > 6.0, "length {len}");
    // A light slack hangs, it does not touch the ground.
    let mut env = Env::default();
    for f in 0..240 {
        env.step(1.0 / 60.0, a, f, &Settings::default(), &w);
        pts = env.rope(0, a + v3(0.0, 2.0, 0.0), b + v3(0.0, 2.0, 0.0), 0.05, 1.0 / 60.0, 20.0, V3::default(), &w);
    }
    let low = pts.iter().map(|p| p.y).fold(f64::MAX, f64::min);
    assert!(low < 66.95 && low > 65.5, "light sag: {low}");
}

#[test]
fn steps_on_snow_kick_up_powder_only() {
    // The print itself is the snow pressed down (`snow_gives_way_where_walked`); a step only
    // kicks up powder, and only on snow.
    let mut env = Env::default();
    for i in 0..60 {
        env.footstep(v3(f64::from(i) * 0.1, 64.125, 0.0), -90.0, Some(64.125));
    }
    assert!(env.prints.is_empty());
    let n = env.puffs.len();
    assert!(n >= 30, "{n} puffs over 6 blocks");
    for i in 60..120 {
        env.footstep(v3(f64::from(i) * 0.1, 64.0, 0.0), -90.0, None);
    }
    assert_eq!(env.puffs.len(), n);
}

/// The vertices of a cape box as the game would draw it hanging: 10 by 16 by 1 pixels behind a
/// neck at `neck`, the outside facing +z (the player looks toward -z).
fn cape_box(neck: V3) -> Vec<Vertex> {
    let (w, h, d) = (10.0 / 16.0, 1.0, 1.0 / 16.0);
    let lo = v3(neck.x - w / 2.0, neck.y - h, neck.z + 0.125);
    let hi = v3(neck.x + w / 2.0, neck.y, neck.z + 0.125 + d);
    let faces: [[V3; 4]; 6] = [
        [v3(lo.x, lo.y, hi.z), v3(hi.x, lo.y, hi.z), v3(hi.x, hi.y, hi.z), v3(lo.x, hi.y, hi.z)],
        [v3(hi.x, lo.y, lo.z), v3(lo.x, lo.y, lo.z), v3(lo.x, hi.y, lo.z), v3(hi.x, hi.y, lo.z)],
        [v3(hi.x, lo.y, lo.z), v3(hi.x, hi.y, lo.z), v3(hi.x, hi.y, hi.z), v3(hi.x, lo.y, hi.z)],
        [v3(lo.x, lo.y, hi.z), v3(lo.x, hi.y, hi.z), v3(lo.x, hi.y, lo.z), v3(lo.x, lo.y, lo.z)],
        [v3(lo.x, hi.y, lo.z), v3(lo.x, hi.y, hi.z), v3(hi.x, hi.y, hi.z), v3(hi.x, hi.y, lo.z)],
        [v3(lo.x, lo.y, hi.z), v3(lo.x, lo.y, lo.z), v3(hi.x, lo.y, lo.z), v3(hi.x, lo.y, hi.z)],
    ];
    let mut out = Vec::new();
    for f in faces {
        for (k, p) in f.iter().enumerate() {
            out.push(Vertex {
                pos: [p.x as f32, p.y as f32, p.z as f32],
                uv: [k as f32 * 0.1, 0.5],
                layer: 0,
                color: 0xFFFF_FFFF,
                light: 240,
                normal: [0, 0, 127, 0],
                glint_uv: [0.0, 0.0],
            });
        }
    }
    out
}

/// Global state: leads, ropes and capes in one test, so tests running side by side do not meet.
#[test]
fn capes_hang_as_cloth_and_leads_become_ropes() {
    let w = Flat { pillar: None, water_above: false };
    clear();
    update_settings(|s| s.cape_range = 128.0);
    let neck = v3(0.0, 65.5, 0.0);
    let mut out = Vec::new();
    let mut frames = 0;
    for _ in 0..100 {
        out.clear();
        assert!(cape(7, &cape_box(neck), [neck.x as f32, neck.y as f32, neck.z as f32], &mut out));
        frame(0, [0.0, 0.0, 0.0], [0.0, 0.0, 1.0, 1.0], &w, &mut Vec::new());
        frames += 1;
        std::thread::sleep(std::time::Duration::from_millis(16));
    }
    assert_eq!(frames, 100);
    // 5 by 10 cells, both sides.
    assert_eq!(out.len(), 5 * 10 * 2 * 4);
    let ys: Vec<f32> = out.iter().map(|v| v.pos[1]).collect();
    let low = ys.iter().cloned().fold(f32::MAX, f32::min);
    assert!(low < 64.62 && low > 64.3, "the cape does not hang: {low}");
    // Never in front of the back (z < 0.125 is inside the player).
    assert!(out.iter().all(|v| v.pos[2] > 0.0), "the cape went through the player");
    // The inventory's player drawn somewhere else in the same frame keeps the game's own cape.
    assert!(cape(7, &cape_box(neck), [neck.x as f32, neck.y as f32, neck.z as f32], &mut Vec::new()));
    assert!(!cape(7, &cape_box(neck + v3(50.0, 0.0, 0.0)), [50.0, 65.5, 0.0], &mut Vec::new()));

    // A lead (384 zero-normal vertices from (0, 66, 0) to (5, 66, 0)) and a knot vertex after it.
    let mut drawn = Vec::new();
    for pass in 0..2 {
        for step in 0..24 {
            let _ = pass;
            let x0 = 5.0 * step as f32 / 24.0;
            let x1 = 5.0 * (step + 1) as f32 / 24.0;
            for _ in 0..2 {
                for (x, z) in [(x0, -0.02), (x0, 0.02), (x1, 0.02), (x1, -0.02)] {
                    drawn.push(Vertex { pos: [x, 66.0, z], uv: [0.0, 0.0], layer: 0, color: 1, light: 2, normal: [0; 4], glint_uv: [0.0, 0.0] });
                }
            }
        }
    }
    assert_eq!(drawn.len(), 384);
    drawn.push(Vertex { pos: [9.0, 9.0, 9.0], uv: [0.0, 0.0], layer: 0, color: 3, light: 2, normal: [0, 127, 0, 0], glint_uv: [0.0, 0.0] });
    let mut out = Vec::new();
    for _ in 0..120 {
        out.clear();
        leads(&drawn, [0.0, 0.0, 0.0], &w, &mut out);
        frame(0, [0.0, 0.0, 0.0], [0.0, 0.0, 1.0, 1.0], &w, &mut Vec::new());
        std::thread::sleep(std::time::Duration::from_millis(16));
    }
    assert!(out.iter().any(|v| v.color == 3), "the knot was lost");
    let low = out.iter().filter(|v| v.color != 3).map(|v| v.pos[1]).fold(f32::MAX, f32::min);
    assert!(low < 65.9 && low > 64.0, "the lead does not sag: {low}");
    a_turning_cape_keeps_hanging();
    clear();
}

// --- Previews (cargo-less: run the ignored test, then render_v2.py) ----------------------------

fn box_faces(c: [V3; 8]) -> Vec<[V3; 4]> {
    [[0, 2, 6, 4], [1, 3, 7, 5], [0, 1, 5, 4], [2, 3, 7, 6], [0, 1, 3, 2], [4, 5, 7, 6]]
        .iter()
        .map(|f| [c[f[0]], c[f[1]], c[f[2]], c[f[3]]])
        .collect()
}

fn corners(x: V3, q: Quat, h: V3) -> [V3; 8] {
    std::array::from_fn(|k| {
        let l = v3(if k & 1 == 0 { -h.x } else { h.x }, if k & 2 == 0 { -h.y } else { h.y }, if k & 4 == 0 { -h.z } else { h.z });
        x + q.rotate(l)
    })
}

fn poly_json(polys: &[(Vec<V3>, [u8; 3])]) -> String {
    let items: Vec<String> = polys
        .iter()
        .map(|(pts, c)| {
            let p: Vec<String> = pts.iter().map(|p| format!("[{:.3},{:.3},{:.3}]", p.x, p.y, p.z)).collect();
            format!("[[{}],[{},{},{}]]", p.join(","), c[0], c[1], c[2])
        })
        .collect();
    format!("[{}]", items.join(","))
}

fn rgb(c: u32) -> [u8; 3] {
    [(c & 255) as u8, (c >> 8 & 255) as u8, (c >> 16 & 255) as u8]
}

#[test]
#[ignore]
fn dump_v2() {
    let w = Flat { pillar: None, water_above: false };
    let mut scenes = Vec::new();
    // 1. TNT in a mound of stone and dirt.
    {
        let center = v3(0.5, 64.5, 4.5);
        let mut blocks = Vec::new();
        for x in -3..=3 {
            for z in 1..=7 {
                for y in 64..=65 {
                    let c = v3(f64::from(x) + 0.5, f64::from(y) + 0.5, f64::from(z) + 0.5);
                    let d = (c - center).len();
                    if d < 3.6 && !(y == 65 && d > 2.6) {
                        blocks.push(([x, y, z], if y == 65 { 0x4A8A3Cu32 } else { 0x7D7D7D }, d));
                    }
                }
            }
        }
        let mut env = Env::default();
        let mut p = Physics::default();
        let st = Settings { pieces: 2, ..Settings::default() };
        let mut frames = Vec::new();
        for f in 0..110 {
            let mut polys: Vec<(Vec<V3>, [u8; 3])> = Vec::new();
            if f < 15 {
                for (b, c, _) in &blocks {
                    let x = v3(f64::from(b[0]) + 0.5, f64::from(b[1]) + 0.5, f64::from(b[2]) + 0.5);
                    for q in box_faces(corners(x, Quat::IDENTITY, v3(0.5, 0.5, 0.5))) {
                        polys.push((q.to_vec(), [(c >> 16) as u8, (c >> 8 & 255) as u8, (c & 255) as u8]));
                    }
                }
                let tnt = if f % 8 < 4 { [220, 60, 50] } else { [245, 240, 235] };
                for q in box_faces(corners(center + v3(0.0, 1.0, 0.0), Quat::IDENTITY, v3(0.5, 0.5, 0.5))) {
                    polys.push((q.to_vec(), tnt));
                }
            }
            if f == 15 {
                for (b, c, d) in &blocks {
                    if *d < 3.2 {
                        env.aired(Aired { pos: *b, boxes: vec![[0.0, 0.0, 0.0, 1.0, 1.0, 1.0]], uv: [0.0; 4], tint: *c, at: env.clock });
                    }
                }
                env.explode(center + v3(0.0, 1.0, 0.0), 4.0);
                env.blow(&mut p, &st);
                blocks.retain(|b| b.2 >= 3.2);
            }
            if f >= 15 {
                for (b, c, _) in &blocks {
                    let x = v3(f64::from(b[0]) + 0.5, f64::from(b[1]) + 0.5, f64::from(b[2]) + 0.5);
                    for q in box_faces(corners(x, Quat::IDENTITY, v3(0.5, 0.5, 0.5))) {
                        polys.push((q.to_vec(), [(c >> 16) as u8, (c >> 8 & 255) as u8, (c & 255) as u8]));
                    }
                }
            }
            for b in &p.bodies {
                let c = rgb(b.verts.first().map_or(0xFF7D7D7D, |v| v.color));
                for q in box_faces(corners(b.x, b.q, b.half)) {
                    polys.push((q.to_vec(), c));
                }
            }
            for pf in &env.puffs {
                let t = (pf.age / pf.life).clamp(0.0, 1.0);
                let col = mix3(pf.from, pf.to, t);
                let half = pf.size * pf.scale_pub();
                let c = [(col[0] * 255.0) as u8, (col[1] * 255.0) as u8, (col[2] * 255.0) as u8];
                for q in box_faces(corners(pf.x, Quat::IDENTITY, v3(half, half, half))) {
                    polys.push((q.to_vec(), c));
                }
            }
            frames.push(poly_json(&polys));
            for _ in 0..2 {
                p.step(1.0 / 60.0, 20.0, &w);
                env.step(1.0 / 60.0, center, f, &st, &w);
            }
        }
        scenes.push(format!("[{}]", frames.join(",")));
    }
    // 2. A player with a cape walks, stops and turns, holding a lead to a post.
    {
        clear();
        let post = v3(4.5, 65.4, 6.5);
        let mut frames = Vec::new();
        for f in 0..150 {
            let t = f as f64 / 30.0;
            // Walk along x, then stand and turn.
            let x = if t < 2.0 { -3.0 + 2.5 * t } else { 2.0 };
            let yaw = if t < 3.0 { 0.0 } else { (t - 3.0) * 0.8 };
            let feet = v3(x, 64.0, 3.0);
            let (s, c) = yaw.sin_cos();
            // The player faces +x at yaw 0; the back is toward -x.
            let fwd = v3(c, 0.0, s);
            let side = v3(-s, 0.0, c);
            let neck = feet + v3(0.0, 1.5, 0.0);
            // A cape box behind the neck (outer face away from the body).
            let (w2, h, d) = (5.0 / 16.0, 1.0, 1.0 / 16.0);
            let back = neck - fwd * 0.17;
            let pt = |a: f64, b: f64, e: f64| back + side * a + v3(0.0, b, 0.0) - fwd * e;
            let faces: [[V3; 4]; 6] = [
                [pt(-w2, -h, d), pt(w2, -h, d), pt(w2, 0.0, d), pt(-w2, 0.0, d)],
                [pt(w2, -h, 0.0), pt(-w2, -h, 0.0), pt(-w2, 0.0, 0.0), pt(w2, 0.0, 0.0)],
                [pt(w2, -h, 0.0), pt(w2, 0.0, 0.0), pt(w2, 0.0, d), pt(w2, -h, d)],
                [pt(-w2, -h, d), pt(-w2, 0.0, d), pt(-w2, 0.0, 0.0), pt(-w2, -h, 0.0)],
                [pt(-w2, 0.0, 0.0), pt(-w2, 0.0, d), pt(w2, 0.0, d), pt(w2, 0.0, 0.0)],
                [pt(-w2, -h, d), pt(-w2, -h, 0.0), pt(w2, -h, 0.0), pt(w2, -h, d)],
            ];
            let rest: Vec<Vertex> = faces
                .iter()
                .flat_map(|q| q.iter().map(|p| Vertex { pos: [p.x as f32, p.y as f32, p.z as f32], uv: [0.0; 2], layer: 0, color: 0xFFFF_FFFF, light: 240, normal: [0, 0, 127, 0], glint_uv: [0.0; 2] }).collect::<Vec<_>>())
                .collect();
            let mut out = Vec::new();
            assert!(cape(1, &rest, [neck.x as f32, neck.y as f32, neck.z as f32], &mut out));
            let hand = feet + v3(0.0, 0.9, 0.0) + side * 0.35 + fwd * 0.2;
            let rope_pts = rope(0, [hand.x, hand.y, hand.z], [post.x, post.y, post.z], 0.08, &w);
            frame(0, [0.0, 0.0, 0.0], [0.0; 4], &w, &mut Vec::new());
            std::thread::sleep(std::time::Duration::from_millis(33));
            let mut polys: Vec<(Vec<V3>, [u8; 3])> = Vec::new();
            // The player: legs, body, head as one box each, facing fwd.
            let q = Quat::axis_angle(v3(0.0, 1.0, 0.0), -yaw);
            for (cx, hy, hx, col) in [(0.375, 0.375, 0.25, [60u8, 70, 160]), (1.125, 0.375, 0.25, [40, 160, 170]), (1.75, 0.25, 0.25, [200, 160, 130])] {
                for f4 in box_faces(corners(feet + v3(0.0, cx, 0.0), q, v3(0.125 * 1.0, hy, hx))) {
                    polys.push((f4.to_vec(), col));
                }
                let _ = hx;
            }
            for (k, quad) in out.chunks(4).enumerate() {
                let pts: Vec<V3> = quad.iter().map(|v| v3(f64::from(v.pos[0]), f64::from(v.pos[1]), f64::from(v.pos[2]))).collect();
                let col = if (k / 2) % 2 == 0 { [180, 30, 40] } else { [150, 20, 30] };
                polys.push((pts, col));
            }
            for wseg in rope_pts.windows(2) {
                let (a, b) = (v3(wseg[0][0], wseg[0][1], wseg[0][2]), v3(wseg[1][0], wseg[1][1], wseg[1][2]));
                let up = v3(0.0, 0.03, 0.0);
                polys.push((vec![a - up, b - up, b + up, a + up], [130, 100, 70]));
            }
            for f4 in box_faces(corners(post - v3(0.0, 0.9, 0.0), Quat::IDENTITY, v3(0.12, 0.5, 0.12))) {
                polys.push((f4.to_vec(), [140, 105, 60]));
            }
            frames.push(poly_json(&polys));
        }
        scenes.push(format!("[{}]", frames.join(",")));
        clear();
    }
    // 3. Water poured onto the ground, lava next to it, then a splash.
    {
        let mut env = Env::default();
        let st = Settings::default();
        let mut frames = Vec::new();
        for f in 0..120 {
            if f == 5 {
                env.pour([0, 64, 3], false);
            }
            if f == 30 {
                env.pour([3, 64, 5], true);
            }
            if f == 60 {
                env.splash(v3(-2.5, 64.0, 5.5), 12.0);
            }
            let mut polys: Vec<(Vec<V3>, [u8; 3])> = Vec::new();
            for d in &env.drops {
                let t = (d.age / d.life).clamp(0.0, 1.0);
                let half = d.size * ((1.0 - t) / 0.3).min(1.0);
                let c = if d.lava { [255, 140, 30] } else { [60, 110, 220] };
                for q in box_faces(corners(d.x, Quat::IDENTITY, v3(half, half, half))) {
                    polys.push((q.to_vec(), c));
                }
            }
            frames.push(poly_json(&polys));
            for _ in 0..2 {
                env.step(1.0 / 60.0, v3(0.0, 66.0, 0.0), f, &st, &w);
            }
        }
        scenes.push(format!("[{}]", frames.join(",")));
    }
    std::fs::write("/tmp/hp_dump2.json", format!("[{}]", scenes.join(","))).unwrap();
}

/// A turning player's cape keeps hanging (its left and right edge do not swap). Global state, so
/// this runs inside `capes_hang_as_cloth_and_leads_become_ropes`.
fn a_turning_cape_keeps_hanging() {
    let w = Flat { pillar: None, water_above: false };
    clear();
    for (yaw0, rate) in [(0.0, 0.0), (1.0, 0.0), (0.0, 0.8)] {
        clear();
        let mut low = 0.0;
        for f in 0..45 {
            let yaw: f64 = yaw0 + rate * f as f64 / 30.0;
            let feet = v3(0.0, 64.0, 0.0);
            let (s, c) = yaw.sin_cos();
            let fwd = v3(c, 0.0, s);
            let side = v3(-s, 0.0, c);
            let neck = feet + v3(0.0, 1.5, 0.0);
            let (w2, h, d) = (5.0 / 16.0, 1.0, 1.0 / 16.0);
            let back = neck - fwd * 0.17;
            let pt = |a: f64, b: f64, e: f64| back + side * a + v3(0.0, b, 0.0) - fwd * e;
            let faces: [[V3; 4]; 2] = [
                [pt(-w2, -h, d), pt(w2, -h, d), pt(w2, 0.0, d), pt(-w2, 0.0, d)],
                [pt(w2, -h, 0.0), pt(-w2, -h, 0.0), pt(-w2, 0.0, 0.0), pt(w2, 0.0, 0.0)],
            ];
            let mut rest: Vec<Vertex> = faces.iter().flat_map(|q| q.iter().map(|p| Vertex { pos: [p.x as f32, p.y as f32, p.z as f32], uv: [0.0; 2], layer: 0, color: 0, light: 0, normal: [0; 4], glint_uv: [0.0; 2] }).collect::<Vec<_>>()).collect();
            let small = rest[0..4].iter().map(|v| Vertex { pos: [v.pos[0], 65.5, v.pos[2]], ..*v }).collect::<Vec<_>>();
            for _ in 0..4 { rest.extend(small.iter().cloned()); }
            let mut out = Vec::new();
            assert!(cape(1, &rest, [neck.x as f32, neck.y as f32, neck.z as f32], &mut out));
            frame(0, [0.0; 3], [0.0; 4], &w, &mut Vec::new());
            std::thread::sleep(std::time::Duration::from_millis(33));
            low = out.iter().map(|v| v.pos[1]).fold(f32::MAX, f32::min);
        }
        assert!(low < 64.6, "yaw {yaw0}, turning {rate}: the cape is lifted to {low}");
    }
}

// --- 0.6.0 ----------------------------------------------------------------------------------

#[test]
fn poured_water_spreads_settles_and_goes() {
    let w = Flat { pillar: None, water_above: false };
    let mut env = Env::default();
    let st = Settings::default();
    env.pour([0, 64, 0], false);
    let n = env.drops.len();
    assert!(n >= 50);
    for f in 0..60 {
        env.step(1.0 / 60.0, v3(0.0, 66.0, 0.0), f, &st, &w);
    }
    // On the floor, spread wider than the block, never through it.
    assert!(env.drops.iter().all(|d| d.x.y > 63.99), "a drop went through the floor");
    let wide = env.drops.iter().map(|d| (d.x.x - 0.5).abs().max((d.x.z - 0.5).abs())).fold(0.0, f64::max);
    assert!(wide > 0.7, "the water did not spread: {wide}");
    // Not piled up in one place: the drops keep apart.
    let mut close = 0;
    for i in 0..env.drops.len() {
        for j in i + 1..env.drops.len() {
            if (env.drops[i].x - env.drops[j].x).len() < DROP_GAP_TEST * 0.5 {
                close += 1;
            }
        }
    }
    assert!(close < env.drops.len() / 4, "{close} pairs on top of each other");
    for f in 60..400 {
        env.step(1.0 / 60.0, v3(0.0, 66.0, 0.0), f, &st, &w);
    }
    assert!(env.drops.is_empty());
}

const DROP_GAP_TEST: f64 = 0.12;

#[test]
fn falling_into_water_splashes_once() {
    let mut env = Env::default();
    env.player_in_water(v3(0.5, 65.0, 0.5), false, 10.0);
    env.player_in_water(v3(0.5, 64.8, 0.5), true, 10.0);
    let n = env.drops.len();
    assert!(n > 20);
    assert!(env.drops.iter().all(|d| d.v.y > 0.0));
    env.player_in_water(v3(0.5, 64.6, 0.5), true, 10.0);
    assert_eq!(env.drops.len(), n, "only on going in");
    let mut env = Env::default();
    env.player_in_water(v3(0.5, 65.0, 0.5), false, 1.0);
    env.player_in_water(v3(0.5, 64.8, 0.5), true, 1.0);
    assert!(env.drops.is_empty(), "walking in makes no splash");
}

#[test]
fn sway_table_and_shader() {
    use crate::native_horizon_physics_sway as sway;
    assert_eq!(sway::kind("minecraft:block/oak_leaves"), sway::LEAVES);
    assert_eq!(sway::kind("minecraft:block/short_grass"), sway::PLANT);
    assert_eq!(sway::kind("minecraft:block/tall_grass_top"), sway::PLANT_TOP);
    assert_eq!(sway::kind("minecraft:block/tall_grass_bottom"), sway::PLANT);
    assert_eq!(sway::kind("minecraft:block/water_still"), sway::WATER);
    assert_eq!(sway::kind("minecraft:block/kelp_plant"), sway::KELP);
    assert_eq!(sway::kind("minecraft:block/vine"), sway::VINES);
    assert_eq!(sway::kind("minecraft:block/grass_block_top"), 0);
    assert_eq!(sway::kind("minecraft:block/stone"), 0);
    let names = vec![("minecraft:block/oak_leaves", 7u32), ("minecraft:block/water_still", 3), ("minecraft:block/stone", 1), ("minecraft:block/short_grass", 9)];
    assert!(sway::set(1, || names.clone(), true, true, 1.0));
    assert!(!sway::set(1, || names.clone(), true, true, 1.0), "nothing changed");
    assert!(sway::set(1, || names.clone(), true, false, 1.0), "a setting changed");
    let s = sway::current().unwrap();
    assert_eq!(s.layers, vec![(3, sway::WATER), (7, sway::LEAVES), (9, sway::PLANT)]);
    let w = sway::wgsl(&s).unwrap();
    assert!(w.contains("case 3u: { return 5u; }"));
    assert!(!w.contains("9u: { return 2u"), "plants are off");
    assert!(w.contains("7u: { return 1u"), "leaves have their own switch");
    let entries = "struct HorizonMaterialOut {\n    @location(7) cylindrical: f32,\n}\nfn horizon_material_vertex(position: vec3<f32>, uv: vec2<f32>, layer: u32) {\n    let pos = position + origin;\n    out.cylindrical = max(length(pos.xz), abs(pos.y));\n}";
    let p = sway::patch(entries, "").unwrap();
    assert!(p.contains("horizon_physics_sway(position + origin, position, uv, layer)"));
    assert!(p.contains("out.hp_open = horizon_physics_open(position, layer);"));
    assert!(sway::patch("no anchor here", "").is_none());
    sway::set_options(sway::SwayOptions { leaves: false, vines: false, water_plants: false, ..Default::default() });
    sway::set(1, || names.clone(), false, false, 1.0);
    assert!(sway::patch(entries, "").is_none(), "all off: the game's own shader");
    sway::set_options(sway::SwayOptions::default());
    // For the WGSL check outside the tests.
    sway::set(1, || names.clone(), true, true, 2.4);
    std::fs::write("/tmp/hp_sway.wgsl", sway::wgsl(&sway::current().unwrap()).unwrap()).unwrap();
}

/// A floor at y = 64 with a roof (y = 68, x and z in -3..=3) on it: a shelter over fires.
struct Roofed;

impl World for Roofed {
    fn cell(&self, x: i32, y: i32, z: i32) -> Cell {
        if y < 64 || (y == 68 && (-3..=3).contains(&x) && (-3..=3).contains(&z)) {
            return Cell::Full;
        }
        Cell::Air
    }
    fn light(&self, _: i32, _: i32, _: i32) -> u32 {
        15 * 16
    }
}

#[test]
fn smoke_gathers_under_a_roof_and_spreads_along_it() {
    let w = Roofed;
    let mut env = Env::default();
    let st = Settings { wind: 0, ..Settings::default() };
    env.emitters.insert([0, 64, 0], 3);
    let mut embers = 0;
    for f in 0..(60 * 12) {
        env.step(1.0 / 60.0, v3(0.5, 66.0, -6.0), f, &st, &w);
        embers += env.puffs.iter().filter(|p| p.kind == FxKind::Ember).count();
    }
    let smoke: Vec<&Puff> = env.puffs.iter().filter(|p| p.kind == FxKind::Smoke).collect();
    assert!(smoke.len() > 30, "too little smoke: {}", smoke.len());
    // Above the roof only where it got out past an edge.
    let through = smoke.iter().filter(|p| p.x.y > 68.0 && (-2.5..3.5).contains(&p.x.x) && (-2.5..3.5).contains(&p.x.z)).count();
    assert!(through == 0, "smoke went through the roof: {through}");
    // Under the roof, spread out wide, not one thin column.
    let high: Vec<&&Puff> = smoke.iter().filter(|p| p.x.y > 66.5).collect();
    assert!(high.len() > 10, "nothing gathered under the roof");
    let wide = high.iter().map(|p| v3(p.x.x - 0.5, 0.0, p.x.z - 0.5).len()).fold(0.0, f64::max);
    assert!(wide > 1.5, "it does not spread along the roof: {wide}");
    // Embers fly, glowing, and vanish.
    assert!(embers > 0, "no embers");
}

#[test]
fn puffs_are_soft_round_and_sorted() {
    let w = Flat { pillar: None, water_above: false };
    let mut env = Env::default();
    env.explode(v3(0.5, 65.0, 0.5), 3.0);
    for f in 0..30 {
        env.step(1.0 / 60.0, v3(0.5, 66.0, -6.0), f, &Settings::default(), &w);
    }
    let mut out = Vec::new();
    env.emit(v3(0.5, 66.0, -6.0), [0.0, 0.0, 1.0, 1.0], &w, &mut out);
    assert!(!out.is_empty() && out.len() % 4 == 0);
    // Every puff has an opaque middle and a clear rim: no hard-edged cubes any more.
    let alphas: Vec<u32> = out.iter().map(|v| v.color >> 24).collect();
    assert!(alphas.iter().any(|&a| a == 0), "no clear rims");
    assert!(alphas.iter().any(|&a| a > 100), "nothing opaque");
    // Back to front: each puff's middle (its first vertex) is no closer than the next one's.
    let per = 80;
    let mids: Vec<f32> = out.chunks(per).map(|c| c[0].pos.iter().map(|x| x * x).sum::<f32>()).collect();
    let sorted = mids.windows(2).filter(|w| w[0] + 1e-3 >= w[1]).count();
    assert!(sorted * 10 >= (mids.len() - 1) * 9, "not drawn far to near");
}

#[test]
#[ignore]
fn dump_v3() {
    // Smoke from a campfire under a roof, and an explosion, as vertex soups for render_v3.py.
    let dump = |name: &str, out: &[Vertex]| {
        let mut s = String::from("[");
        for (i, v) in out.iter().enumerate() {
            if i > 0 {
                s.push(',');
            }
            s.push_str(&format!("[{},{},{},{}]", v.pos[0], v.pos[1], v.pos[2], v.color));
        }
        s.push(']');
        std::fs::write(format!("/tmp/hp3_{name}.json"), s).unwrap();
    };
    let cam = v3(0.5, 65.7, -7.0);
    let w = Roofed;
    let mut env = Env::default();
    let st = Settings { wind: 0, ..Settings::default() };
    env.emitters.insert([0, 64, 0], 3);
    env.emitters.insert([2, 64, 1], 1);
    for f in 0..(60 * 14) {
        env.step(1.0 / 60.0, cam, f, &st, &w);
    }
    let mut out = Vec::new();
    env.emit(cam, [0.0, 0.0, 1.0, 1.0], &w, &mut out);
    dump("roof", &out);
    let w = Flat { pillar: None, water_above: false };
    let cam = v3(0.5, 66.0, -9.0);
    for (name, t) in [("blast_a", 0.15), ("blast_b", 0.6), ("blast_c", 2.5)] {
        let mut env = Env::default();
        env.explode(v3(0.5, 64.6, 0.5), 4.0);
        let steps = (t * 60.0) as u64;
        for f in 0..steps {
            env.step(1.0 / 60.0, cam, f, &Settings { wind: 0, ..Settings::default() }, &w);
        }
        let mut out = Vec::new();
        env.emit(cam, [0.0, 0.0, 1.0, 1.0], &w, &mut out);
        dump(name, &out);
    }
    let mut env = Env::default();
    env.splash(v3(0.5, 64.9, 0.5), 9.0);
    for f in 0..20 {
        env.step(1.0 / 60.0, cam, f, &Settings::default(), &Flat { pillar: None, water_above: false });
    }
    let mut out = Vec::new();
    env.emit(v3(0.5, 66.3, -2.5), [0.0, 0.0, 1.0, 1.0], &w, &mut out);
    dump("splash", &out);
    // Footprints on snow, seen from behind and above.
    let mut env = Env::default();
    for i in 0..40 {
        env.footstep(v3(0.5 + (f64::from(i) * 0.15).sin() * 0.2, 64.125, f64::from(i) * 0.12), 0.0, Some(64.125));
    }
    for f in 0..30 {
        env.step(1.0 / 60.0, cam, f, &Settings::default(), &w);
    }
    let mut out = Vec::new();
    env.emit(v3(0.5, 65.4, -1.2), [0.0, 0.0, 1.0, 1.0], &w, &mut out);
    dump("snow", &out);
}

#[test]
fn a_lead_hangs_still_in_the_wind() {
    let w = Flat { pillar: None, water_above: false };
    let mut env = Env::default();
    let (a, b) = (v3(0.0, 67.0, 0.0), v3(4.0, 67.0, 0.0));
    let strong = wind_at(0.0, 2.4);
    let mut side: Vec<f64> = Vec::new();
    for f in 0..600 {
        env.step(1.0 / 60.0, a, f, &Settings::default(), &w);
        let pts = env.rope(0, a, b, 0.12, 1.0 / 60.0, 20.0, strong, &w);
        if f > 300 {
            side.push(pts[pts.len() / 2].z);
        }
    }
    let max = side.iter().fold(0.0f64, |m, z| m.max(z.abs()));
    let swing = side.iter().fold(f64::MIN, |m, z| m.max(*z)) - side.iter().fold(f64::MAX, |m, z| m.min(*z));
    assert!(max < 0.2, "the wind blows the lead aside: {max}");
    assert!(swing < 0.02, "it keeps swinging: {swing}");
}

#[test]
fn walking_into_a_ragdoll_pushes_it() {
    let w = Flat { pillar: None, water_above: false };
    let mut p = Physics::default();
    let (verts, marks) = humanoid();
    p.spawn_mob(&verts, &marks, v3(0.0, 65.6, -4.0), V3::default(), 1, 100.0);
    run(&mut p, &w, 4.0);
    let center = |p: &Physics| p.bodies.iter().map(|b| b.x.x).sum::<f64>() / p.bodies.len() as f64;
    let before = center(&p);
    let z = p.bodies.iter().map(|b| b.x.z).sum::<f64>() / p.bodies.len() as f64;
    // The player walks through it along +x at 4.3 blocks a second.
    for i in 0..90 {
        let x = before - 1.5 + f64::from(i) * 4.3 / 60.0;
        p.shove(v3(x, 64.0, z), v3(4.3, 0.0, 0.0));
        p.step(1.0 / 60.0, 20.0, &w);
    }
    let after = center(&p);
    assert!(after > before + 0.5, "not pushed along: {before} -> {after}");
    for b in &p.bodies {
        assert!(b.x.y > 63.95, "pushed into the floor: {:?}", b.x);
    }
}

#[test]
fn a_lead_keeps_its_shape_as_the_mob_walks_off() {
    let w = Flat { pillar: None, water_above: false };
    let mut env = Env::default();
    let a = v3(0.0, 66.0, 0.0);
    let mut last: Option<V3> = None;
    let mut jump: f64 = 0.0;
    for f in 0..600 {
        // The mob walks from 2 to 8 blocks away and back.
        let d = 5.0 - 3.0 * (f64::from(f as u32) / 100.0).cos();
        let b = v3(d, 65.0, 0.0);
        env.step(1.0 / 60.0, a, f, &Settings::default(), &w);
        let pts = env.rope(0, a, b, 0.12, 1.0 / 60.0, 20.0, V3::default(), &w);
        let mid = pts[pts.len() / 2];
        if let Some(l) = last && f > 60 {
            jump = jump.max((mid - l).len());
        }
        last = Some(mid);
    }
    assert_eq!(env.ropes.len(), 1, "a new rope was started");
    assert!(jump < 0.15, "the lead jumps: {jump}");
}

#[test]
fn water_is_marked_by_depth_and_shore() {
    use crate::native_horizon_physics_sway as sway;
    // A sea 12 deep (y 0..=11) with a beach at x >= 10, and a pond one deep at x <= -10.
    let water = |x: i32, y: i32, z: i32| -> bool {
        let _ = z;
        if x >= 10 || y > 11 || y < 0 {
            return false;
        }
        if x <= -10 {
            return y == 11;
        }
        true
    };
    assert_eq!(sway::water_code(0, 11, 0, water, -2), 6, "deep open water");
    assert_eq!(sway::water_code(10, 11, 0, water, -2), 0, "the shore stays still");
    assert_eq!(sway::water_code(9, 11, 0, water, -2), 3, "near the shore: small");
    assert_eq!(sway::water_code(-11, 11, 0, water, -2), 1, "a puddle");
    // The shader and the boats move the water alike: at the shore nothing, on the sea a swell.
    let calm = sway::water_motion(3.0, 4.0, 100.0, 0.0, 1.0);
    assert!(calm.iter().skip(1).all(|v| v.abs() < 1e-9) && (calm[0] + 0.04).abs() < 1e-9);
    let mut top = f64::MIN;
    let mut low = f64::MAX;
    for i in 0..400 {
        let m = sway::water_motion(f64::from(i) * 0.7, 2.0, 50.0, 1.0, 1.0);
        top = top.max(m[0]);
        low = low.min(m[0]);
    }
    assert!(top - low > 1.0, "no swell on the open sea: {}", top - low);
    // The game's clock for the water follows the world's.
    assert!((sway::water_time(24000 * 5 + 600, 0.5) - 600.5 / 20.0).abs() < 1e-3);
}

#[test]
fn snow_gives_way_where_walked() {
    use crate::native_horizon_physics_sway as sway;
    sway::snow_world(7, 0);
    sway::snow_clear();
    std::thread::sleep(std::time::Duration::from_millis(110));
    let _ = sway::snow_take_dirty();
    // Snow three layers deep (top 0.375) on y 0, x -6..=1 (x 2 and on is grass), z -6..=6.
    let top = |x: i32, y: i32, z: i32| -> Option<f64> {
        (y == 0 && (-6..=1).contains(&x) && (-6..=6).contains(&z)).then_some(0.375)
    };
    assert!(sway::snow_press([0.3, 0.375, 0.2], 0.6, [0.0, 1.0], top), "the snow moved");
    std::thread::sleep(std::time::Duration::from_millis(110));
    let dirty = sway::snow_take_dirty();
    assert!(dirty.contains(&(0, 0, 0)) && dirty.contains(&(-1, 0, -1)), "{dirty:?}");
    let here = sway::snow_in_section(0, 0, 0);
    let b = here.iter().find(|d| d.0 == [0, 0, 0]).expect("block 0,0,0 pressed");
    let pts = sway::SNOW_GRID + 1;
    let at = |d: &sway::SnowDent, i: usize, j: usize| d.depth[i * pts + j];
    // Under the foot (x 0.25, z 0.25): pressed down, not through the snow.
    let deep = at(&b.1, 2, 2);
    assert!((deep - 0.125).abs() < 1e-6, "an eighth of a block, as feet sink: {deep}");
    // A lip pushed up somewhere around it.
    let lip = sway::snow_in_section(0, 0, 0).iter().chain(sway::snow_in_section(-1, 0, 0).iter())
        .chain(sway::snow_in_section(0, 0, -1).iter()).chain(sway::snow_in_section(-1, 0, -1).iter())
        .flat_map(|d| d.1.depth).fold(0.0f32, f32::min);
    assert!(lip < -0.005, "{lip}");
    // The surface stays whole: a point on the edge of two blocks moves alike in both.
    let west = sway::snow_in_section(-1, 0, 0);
    let w = west.iter().find(|d| d.0 == [15, 0, 0]).expect("block -1,0,0 pressed");
    for j in 0..pts {
        assert!((at(&w.1, sway::SNOW_GRID, j) - at(&b.1, 0, j)).abs() < 1e-6, "edge {j}");
    }
    // Next to the grass the snow's edge does not move.
    assert!(sway::snow_press([1.6, 0.375, 0.5], 0.6, [1.0, 0.0], top));
    let b1 = sway::snow_in_section(0, 0, 0).into_iter().find(|d| d.0 == [1, 0, 0]).expect("block 1 pressed");
    for j in 0..pts {
        assert_eq!(at(&b1.1, sway::SNOW_GRID, j), 0.0, "edge by the grass {j}");
    }
    // The pressed top: 64 cells, deepest and darker under the foot, the shading by the slope.
    let corner = |x: f32, z: f32| sway::SnowCorner { at: [x, z], uv: [x * 32768.0, z * 32768.0], light: 240 | 0 << 8, color: 0xffff_ffff };
    let quads = sway::snow_top([0.0, 0.375, 0.0], 0.375, &b.1, &[corner(0.0, 0.0), corner(0.0, 1.0), corner(1.0, 1.0), corner(1.0, 0.0)]);
    assert_eq!(quads.len(), 64 * 4);
    let low = quads.iter().map(|q| q.0[1]).fold(f32::MAX, f32::min);
    assert!((low - (0.375 - 0.125)).abs() < 1e-4, "{low}");
    let darkest = quads.iter().map(|q| q.3 & 255).min().unwrap();
    assert!(darkest < 230, "{darkest}");
    assert!(quads.iter().all(|q| q.3 >> 24 == 0xff), "keeps the alpha byte");
    // Facing up, as the flat top did.
    for c in quads.chunks(4) {
        let a = [c[1].0[0] - c[0].0[0], c[1].0[2] - c[0].0[2]];
        let b = [c[2].0[0] - c[0].0[0], c[2].0[2] - c[0].0[2]];
        assert!(a[1] * b[0] - a[0] * b[1] > 0.0);
    }
    // Forgetting them meshes the blocks again, flat.
    sway::snow_clear();
    assert!(sway::snow_in_section(0, 0, 0).is_empty());
    std::thread::sleep(std::time::Duration::from_millis(110));
    assert!(!sway::snow_take_dirty().is_empty());
}

#[test]
#[ignore]
fn dump_snow_trail() {
    use crate::native_horizon_physics_sway as sway;
    use std::fmt::Write as _;
    sway::snow_world(9, 0);
    sway::snow_clear();
    // A field of full snow blocks: a player walks a bend, a pig crosses it.
    let top = |x: i32, y: i32, z: i32| -> Option<f64> { (y == 0 && (-8..8).contains(&x) && (-8..8).contains(&z)).then_some(0.5) };
    let mut prev = [-7.0f64, -5.0];
    for k in 0..400 {
        let t = f64::from(k) / 399.0;
        let p = [-7.0 + 13.0 * t, -5.0 + 9.0 * (t * 2.2).sin() * 0.8];
        sway::snow_press([p[0], 0.5, p[1]], 0.6, [p[0] - prev[0], p[1] - prev[1]], top);
        prev = p;
    }
    for k in 0..200 {
        let z = -7.0 + 14.0 * f64::from(k) / 199.0;
        sway::snow_press([3.0 + 0.3 * (z * 0.7).sin(), 0.5, z], 0.9, [0.0, 1.0], top);
    }
    let mut out = String::new();
    let corner = |x: f32, z: f32| sway::SnowCorner { at: [x, z], uv: [x * 32768.0, z * 32768.0], light: 240, color: 0xffff_ffff };
    let corners = [corner(0.0, 0.0), corner(0.0, 1.0), corner(1.0, 1.0), corner(1.0, 0.0)];
    for cx in [-1, 0] {
        for cz in [-1, 0] {
            for (p, d) in sway::snow_in_section(cx, 0, cz) {
                let o = [(cx * 16 + p[0]) as f32, 0.5, (cz * 16 + p[2]) as f32];
                for q in sway::snow_top(o, 0.5, &d, &corners) {
                    let _ = writeln!(out, "{} {} {} {}", q.0[0], q.0[1], q.0[2], q.3 & 255);
                }
            }
        }
    }
    std::fs::write("/tmp/snow_trail.txt", out).unwrap();
}

#[test]
fn a_cut_tree_falls_over_away_from_the_player() {
    // A trunk of five logs above its cut at y 64 (the stump gone), leaves on top; pushed toward +x.
    let mut p = Physics::default();
    let st = Settings::default();
    let mut blocks: Vec<Falling> = (65..70).map(|y| Falling { pos: [0, y, 0], uv: [0.0; 4], tint: 0xffffff, leaves: false }).collect();
    blocks.push(Falling { pos: [0, 70, 0], uv: [0.0; 4], tint: 0xffffff, leaves: true });
    p.spawn_falling(&blocks, v3(0.5, 64.0, 0.5), v3(1.0, 0.0, 0.0), &st);
    assert_eq!(p.bodies.len(), 5 + 2, "five whole logs and the leaves in clumps");
    run(&mut p, &Flat { pillar: None, water_above: false }, 4.0);
    let logs = &p.bodies[..5];
    // Down on the ground, and spread out away from the player: the top went furthest.
    assert!(logs.iter().all(|b| b.x.y < 64.9), "{:?}", logs.iter().map(|b| b.x.y).collect::<Vec<_>>());
    assert!(logs.iter().all(|b| b.x.x > 0.0));
    assert!(logs[4].x.x > logs[0].x.x + 1.0, "fell over: {} vs {}", logs[4].x.x, logs[0].x.x);
    assert_eq!(collapse_kind("minecraft:birch_leaves"), 2);
    assert_eq!(collapse_kind("minecraft:grass_block"), 1);
    assert_eq!(collapse_kind("minecraft:iron_ore"), 1);
    assert_eq!(collapse_kind("minecraft:birch_log"), 3);
    assert!(is_log("minecraft:birch_log") && !is_log("minecraft:birch_planks"));
}

#[test]
fn the_quick_setup_walks_through_its_pages() {
    let (w, h) = (640.0, 345.0);
    let mut s = Settings::default();
    assert!(!s.setup);
    // Mobs: the middle panel is Ragdoll.
    assert_eq!(setup_hit(0, w, h, 320.0, 200.0), SetupHit::Pick(1));
    setup_choose(&mut s, 0, Some(1));
    assert_eq!(s.deaths, 1);
    setup_choose(&mut s, 0, Some(2));
    assert_eq!(s.deaths, 3, "fractured");
    // Blocks: four panels; the second is Particles.
    assert_eq!(setup_hit(1, w, h, 640.0 * 0.375, 200.0), SetupHit::Pick(1));
    setup_choose(&mut s, 1, Some(1));
    assert!(s.blocks && s.block_style == 1);
    setup_choose(&mut s, 1, None);
    assert!(!s.blocks);
    // The buttons, the bars that pick nothing, the last page's Continue.
    assert_eq!(setup_hit(2, w, h, 40.0, 25.0), SetupHit::Off);
    assert_eq!(setup_hit(2, w, h, 600.0, 25.0), SetupHit::Skip);
    assert_eq!(setup_hit(2, w, h, 320.0, 320.0), SetupHit::None);
    assert_eq!(setup_hit(4, w, h, 320.0, 250.0), SetupHit::Continue);
    // Saved and read back.
    s.setup = true;
    s.weather = false;
    let back = Settings::from_text(&s.to_text());
    assert_eq!(back, s);
    // Hovering a panel lights it up and frames it.
    let plain = setup_ui(0, w, h, (0.0, 0.0));
    let hover = setup_ui(0, w, h, (320.0, 200.0));
    let white = |ui: &[SetupUi]| ui.iter().filter(|u| matches!(u, SetupUi::Rect { color: [255, 255, 255, 255], w: 2.0, .. })).count();
    assert_eq!(white(&plain), 0);
    assert_eq!(white(&hover), 2);
}

#[test]
fn the_previews_play_their_choice() {
    for (scene, moving) in [(Scene::Mob(1), 7), (Scene::Mob(2), 7), (Scene::Mob(3), 10), (Scene::Block(0), 1), (Scene::Block(1), 40), (Scene::Block(2), 4), (Scene::Block(3), 4), (Scene::Collapse, 30)] {
        let mut sc = SetupScene::new(scene, 5);
        for _ in 0..150 {
            sc.step(1.0 / 60.0);
        }
        let quads = sc.quads(320.0, 172.0, 213.0, 345.0);
        assert!(quads.len() > 200, "{scene:?}: the grass and the pieces");
        let _ = moving;
    }
    let mut rain = SetupScene::new(Scene::Weather, 1);
    for _ in 0..90 {
        rain.step(1.0 / 60.0);
    }
    assert!(rain.quads(320.0, 172.0, 640.0, 345.0).iter().filter(|q| q.color[3] < 1.0).count() > 50, "rain and spray");
}

/// Writes a screen's items (and its previews' quads) for `render_setup.py`.
fn dump_items(out: &mut String, name: &str, items: Vec<SetupUi>, scenes: &[&SetupScene]) {
    use std::fmt::Write as _;
    let _ = writeln!(out, "SHOT {name}");
    for item in items {
        match item {
            SetupUi::Rect { x, y, w, h, color } => { let _ = writeln!(out, "R {x} {y} {w} {h} {} {} {} {}", color[0], color[1], color[2], color[3]); }
            SetupUi::Gradient { x, y, w, h, top, bottom } => { let _ = writeln!(out, "G {x} {y} {w} {h} {} {} {} {} {} {}", top[0], top[1], top[2], bottom[0], bottom[1], bottom[2]); }
            SetupUi::Text { text, x, y, color } => { let _ = writeln!(out, "T {x} {y} {} {} {} 1 0 {text}", color[0], color[1], color[2]); }
            SetupUi::Label { text, x, y, color, scale, left } => { let _ = writeln!(out, "T {x} {y} {} {} {} {scale} {} {text}", color[0], color[1], color[2], u8::from(left)); }
            SetupUi::Tiles { x, y, w, h, sprite, size, color } => { let _ = writeln!(out, "I {x} {y} {w} {h} {sprite} {size} {} {} {}", color[0], color[1], color[2]); }
            SetupUi::Clip { x, y, w, h } => { let _ = writeln!(out, "K {x} {y} {w} {h}"); }
            SetupUi::Unclip => { let _ = writeln!(out, "U"); }
            SetupUi::Scene { index, x, y, w: sw, h: sh } => {
                let _ = writeln!(out, "C {x} {y} {sw} {sh}");
                for q in scenes[index].quads(x + sw * 0.5, y + sh * 0.5, sw, sh) {
                    let mut line = format!("Q {} {} {} {} {}", q.sprite, q.color[0], q.color[1], q.color[2], q.color[3]);
                    for k in 0..4 {
                        let _ = write!(line, " {} {} {} {} {}", q.p[k][0], q.p[k][1], q.p[k][2], q.uv[k][0], q.uv[k][1]);
                    }
                    let _ = writeln!(out, "{line}");
                }
                let _ = writeln!(out, "E");
            }
        }
    }
}

#[test]
#[ignore]
fn dump_setup_screens() {
    let (w, h) = (640.0f32, 345.0f32);
    let shots: [(usize, (f32, f32), f64, &str); 14] = [
        (0, (5.0, 340.0), 2.4, "mobs"),
        (0, (100.0, 200.0), 2.4, "mobs_left"),
        (0, (320.0, 200.0), 2.4, "mobs_middle"),
        (0, (540.0, 200.0), 2.4, "mobs_right"),
        (0, (600.0, 28.0), 2.4, "mobs_skip"),
        (0, (60.0, 28.0), 2.4, "mobs_off"),
        (1, (5.0, 340.0), 2.0, "blocks"),
        (1, (240.0, 200.0), 2.0, "blocks_particles"),
        (2, (5.0, 340.0), 1.5, "weather"),
        (2, (320.0, 200.0), 1.5, "weather_hover"),
        (3, (5.0, 340.0), 2.6, "collapse"),
        (3, (320.0, 200.0), 1.9, "collapse_hover"),
        (4, (5.0, 340.0), 0.0, "done"),
        (4, (320.0, 250.0), 0.0, "done_hover"),
    ];
    let mut out = String::new();
    for (page, cursor, t, name) in shots {
        let scenes: Vec<SetupScene> = setup_page(page).1.iter().enumerate().map(|(i, c)| {
            let mut sc = SetupScene::new(c.1, 17 + i as u64 * 31);
            for _ in 0..(t * 60.0) as usize {
                sc.step(1.0 / 60.0);
            }
            sc
        }).collect();
        let refs: Vec<&SetupScene> = scenes.iter().collect();
        dump_items(&mut out, name, setup_ui(page, w, h, cursor), &refs);
    }
    // The Physics Settings screen: the menu with the pointer on each category, and two pages.
    for (i, cat) in CATEGORIES.iter().enumerate() {
        let mut m = Menu::for_test();
        let tab_y = 25.0 + 30.0 * i as f32 - if i > 10 { 150.0 } else { 0.0 };
        m.scroll = if i > 10 { 150.0 } else { 0.0 };
        let items0 = menu_ui(&mut m, w, h, (40.0, tab_y + 10.0));
        let _ = items0;
        for _ in 0..(2.4 * 60.0) as usize {
            m.picture.step(1.0 / 60.0);
        }
        let items = menu_ui(&mut m, w, h, (40.0, tab_y + 10.0));
        dump_items(&mut out, &format!("menu_{}", cat.name.to_lowercase().replace(' ', "_").replace('&', "and")), items, &[&m.picture]);
    }
    let pages: Vec<(MenuView, f32, &str)> = vec![
        (MenuView::Page(0), 0.0, "page_general"),
        (MenuView::Page(1), 0.0, "page_mobs"),
        (MenuView::Page(2), 0.0, "page_blocks"),
        (MenuView::Page(3), 0.0, "page_ocean"),
        (MenuView::Page(4), 0.0, "page_dynamic"),
        (MenuView::Page(5), 0.0, "page_cloth"),
        (MenuView::Page(6), 0.0, "page_projectiles"),
        (MenuView::Page(7), 0.0, "page_liquids"),
        (MenuView::Page(8), 0.0, "page_snow"),
        (MenuView::Page(9), 0.0, "page_sounds"),
        (MenuView::Page(10), 0.0, "page_animations"),
        (MenuView::Anim(1), 0.0, "page_animation_edit"),
        (MenuView::Page(11), 0.0, "page_fire"),
        (MenuView::Page(12), 0.0, "page_weather"),
        (MenuView::Page(12), 120.0, "page_weather_scrolled"),
        (MenuView::Page(13), 0.0, "page_collapse"),
    ];
    for (view, scroll, name) in pages {
        let mut m = Menu::for_test();
        m.view = view;
        m.page_scroll = scroll;
        if let MenuView::Anim(slot) = view {
            set_anim_edit(slot);
        }
        let items = menu_ui(&mut m, w, h, (330.0, 95.0));
        dump_items(&mut out, name, items, &[&m.picture]);
    }
    std::fs::write("/tmp/setup_dump.txt", out).unwrap();
}


#[test]
fn physics_settings_pages_change_the_settings() {
    let (w, h) = (640.0, 345.0);
    let mut s = Settings::default();
    // Sliders land on their steps and stay in range.
    Key::SimSpeed.slide(&mut s, 0.5);
    assert!((s.sim_speed - 1.05).abs() < 1e-4, "{}", s.sim_speed);
    Key::MaxObjects.slide(&mut s, 2.0);
    assert_eq!(s.max_objects, 10000);
    Key::RagdollLimit.slide(&mut s, 0.0);
    assert_eq!(s.ragdoll_limit, 1);
    // Switches and choices.
    Key::Deaths.press(&mut s);
    assert_eq!(Key::Deaths.label(&s), "Mob Physics: Blocky");
    Key::Crack.press(&mut s);
    assert_eq!(Key::Crack.label(&s), "Crack Particle Physics: OFF");
    assert_eq!(Key::ParticleLife.label(&Settings::default()), "Lifetime (Particles): 0,10");
    // Reset puts a page back.
    for row in category_rows(0) {
        for k in row {
            k.reset(&mut s);
        }
    }
    assert!(s.crack && s.sim_speed == 1.0 && s.max_objects == 1000);
    // All the settings survive the file.
    s.joint_force = 2.5;
    s.anim = 1;
    assert_eq!(Settings::from_text(&s.to_text()), s);
    // The menu: every category has a page or says it is not in the mod yet; the box says what
    // it is about in a few lines.
    for (i, c) in CATEGORIES.iter().enumerate() {
        assert_eq!(category_rows(i).is_empty(), c.kind != 0, "{}", c.name);
        assert!(ui_wrap(c.about, 470.0).len() <= 3, "{}", c.name);
    }
    let mut m = Menu::for_test();
    let items = menu_ui(&mut m, w, h, (40.0, 25.0 + 30.0 + 10.0));
    assert_eq!(m.shown, 1, "the pointer on MOBS shows its picture");
    assert!(items.iter().any(|u| matches!(u, SetupUi::Rect { color: [110, 232, 40, 255], .. })), "its green bar");
    m.view = MenuView::Page(0);
    let page = menu_ui(&mut m, w, h, (0.0, 0.0));
    let texts: Vec<String> = page.iter().filter_map(|u| match u { SetupUi::Text { text, .. } => Some(text.clone()), _ => None }).collect();
    for t in ["General Settings", "Physics Max Objects: 1000", "MC Break Particles: OFF", "RESET", "Done"] {
        assert!(texts.iter().any(|x| x == t), "{t} in {texts:?}");
    }
    assert!(ui_text_width("GENERAL") > 30.0);
}

#[test]
fn every_settings_page_fits_and_survives_the_file() {
    let s = Settings::default();
    // Two side by side: each label fits its half.
    for cat in 0..CATEGORIES.len() {
        for row in category_rows(cat) {
            if row.len() == 2 {
                for k in row {
                    let w = ui_text_width(&k.label(&s));
                    assert!(w <= 146.0, "{} on page {cat} is {w} wide", k.label(&s));
                }
            }
        }
    }
    // The photographed pages' labels, as the mod comes.
    for (k, want) in [
        (Key::Activation, "Activation Distance: 96,00"),
        (Key::BlockLife, "Lifetime (Blocks): 4,00"),
        (Key::ObjScale, "Physics Objects Scale: 1,00"),
        (Key::BlockVar, "Lifetime Variance: 3,00"),
        (Key::Anim, "Animation: default"),
        (Key::CapeOn, "Entity Cloth Physics: ON"),
        (Key::CapeRange, "Entity Cloth Range: 48,00"),
        (Key::HideCape, "Hide Cloth when Armor equipped: OFF"),
        (Key::ImpactVolume, "Physics Impact Volume: 1,00"),
        (Key::WindVolume, "Wind Volume: 1,00"),
        (Key::WindForces, "Wind Forces: ON"),
        (Key::Weather, "Weather Particles: ON"),
        (Key::RainAmount, "Rain Weather Particle Amount: 5"),
        (Key::ThunderAmount, "Thunder Weather Particle Amount: 2"),
        (Key::WindClear, "Clear Weather: 0,10"),
        (Key::WindRain, "Rainy Weather: 1,00"),
        (Key::WindThunder, "Thunder Weather: 1,40"),
        (Key::OpRain, "Rain: 1,00"),
        (Key::Collapse, "Collapse Blocks: ON"),
        (Key::CollapseDrops, "Drop collapsed items: ON"),
        (Key::CollapseTicks, "Collapse Speed (Ticks): 10"),
        (Key::CollapseLimit, "Collapse Limit (Blocks): 100"),
    ] {
        assert_eq!(k.label(&s), want);
    }
    let mut t = Settings::default();
    assert_eq!(Key::BlockPhysics.label(&t), "Block Physics: Crumbled");
    Key::BlockPhysics.press(&mut t);
    assert_eq!(Key::BlockPhysics.label(&t), "Block Physics: Fractured");
    Key::BlockPhysics.press(&mut t);
    assert_eq!(Key::BlockPhysics.label(&t), "Block Physics: Off");
    assert!(!t.blocks);
    // Everything, the animations too, survives the file.
    t.activation = 40.0;
    t.collapse_ticks = 0;
    t.wind_thunder = 2.5;
    t.hide_cape = true;
    t.anim_used |= 1 << 8;
    t.anims[8] = AnimDef { motion: 2, particle: 1, duration: 1.5, amount: 20 };
    t.anim = 8;
    assert_eq!(Settings::from_text(&t.to_text()), t);
    // Bigger pieces, fewer of them.
    assert!(Settings { obj_scale: 2.0, ..s }.pieces_per_block() < s.pieces_per_block());
}

#[test]
fn the_wind_follows_the_weather() {
    let s = Settings::default();
    set_weather(0.0, 0.0);
    assert!((s.wind() - 0.1).abs() < 1e-6);
    set_weather(1.0, 0.0);
    assert!((s.wind() - 1.0).abs() < 1e-6);
    set_weather(1.0, 1.0);
    assert!((s.wind() - 1.4).abs() < 1e-6);
    assert_eq!(Settings { wind_forces: false, ..s }.wind(), 0.0);
    set_weather(0.0, 0.0);
}

#[test]
fn despawn_animations_move_the_pieces_and_puff() {
    let a = |i: usize| BUILTIN_ANIMS[i].1;
    // Shrink: full size until the end, nothing at the end.
    assert_eq!(despawn_pose(&a(0), 5.0), (1.0, 0.0));
    assert_eq!(despawn_pose(&a(0), 0.0).0, 0.0);
    // Sink: down into the ground.
    assert!(despawn_pose(&a(2), 0.0).1 < -0.5);
    // Bounce: up in the middle of it.
    assert!(despawn_pose(&a(6), 1.0).1 > 0.1);
    // Pieces whose time is up leave where they were for the puffs.
    let w = Flat { pillar: None, water_above: false };
    let mut p = Physics::default();
    let (c, h, verts) = box_mesh([0.0, 0.0, 0.0, 0.5, 0.5, 0.5], [0.0, 0.0, 1.0, 1.0], 0xFFFF_FFFF);
    p.add(vec![Body::new(v3(0.5, 64.3, 0.5) + c, Quat::IDENTITY, h, verts)], Vec::new(), 0.5, None);
    run(&mut p, &w, 1.0);
    assert!(p.objects.is_empty());
    assert_eq!(p.gone.len(), 1);
    let mut env = Env::default();
    env.despawn_puffs(&a(1), p.gone[0].0, p.gone[0].1);
    assert!(!env.puffs.is_empty(), "portal puffs");
    let mut env = Env::default();
    env.despawn_puffs(&a(2), p.gone[0].0, p.gone[0].1);
    assert!(!env.drops.is_empty(), "splash drops");
}

#[test]
fn a_falling_block_lands_with_a_sound() {
    let w = Flat { pillar: None, water_above: false };
    let mut p = Physics::default();
    let (c, h, verts) = box_mesh([0.0, 0.0, 0.0, 1.0, 1.0, 1.0], [0.0, 0.0, 1.0, 1.0], 0xFFFF_FFFF);
    p.add(vec![Body::new(v3(0.0, 70.0, 0.0) + c, Quat::IDENTITY, h, verts)], Vec::new(), 30.0, None);
    let mut heard = 0;
    for _ in 0..120 {
        p.step(1.0 / 60.0, 20.0, &w);
        heard += p.impacts.len();
        p.impacts.clear();
    }
    assert!((1..=4).contains(&heard), "{heard} landings");
}

#[test]
fn collapses_wait_their_ticks() {
    queue_collapse([1, 2, 3], 0.0);
    queue_collapse([4, 5, 6], 60.0);
    assert_eq!(due_collapses(), vec![[1, 2, 3]]);
    assert!(due_collapses().is_empty());
    set_collapse_replay([1, 2, 3]);
    assert!(!take_collapse_replay([9, 9, 9]));
    assert!(take_collapse_replay([1, 2, 3]));
    assert!(!take_collapse_replay([1, 2, 3]));
}

#[test]
fn thrown_snowballs_shatter_into_pieces() {
    use crate::native_horizon_physics_sway as sway;
    sway::set_particle_catch(32);
    let snow = vec!["minecraft:item/snowball".to_string()];
    let apple = vec!["minecraft:item/apple".to_string()];
    assert!(sway::catch_particle(None, Some(&snow), [0.0; 3], [0.0; 3]));
    assert!(!sway::catch_particle(None, Some(&apple), [0.0; 3], [0.0; 3]));
    sway::set_particle_catch(0);
    let _ = sway::take_particles();
}

#[test]
fn settings_pages_scroll_and_the_animation_list_works() {
    let (w, h) = (640.0, 300.0);
    let texts = |items: &[SetupUi]| -> Vec<String> { items.iter().filter_map(|u| match u { SetupUi::Text { text, .. } => Some(text.clone()), _ => None }).collect() };
    // Weather: too long for the screen, it scrolls, with a bar.
    let mut m = Menu::for_test();
    m.view = MenuView::Page(12);
    let top = menu_ui(&mut m, w, h, (0.0, 0.0));
    let t = texts(&top);
    assert!(t.iter().any(|x| x == "Wind Strength"));
    assert!(!t.iter().any(|x| x == "Dust: 1,00"), "below the fold: {t:?}");
    assert!(top.iter().any(|u| matches!(u, SetupUi::Clip { .. })));
    m.page_scroll = 1000.0;
    let low = menu_ui(&mut m, w, h, (0.0, 0.0));
    assert!(texts(&low).iter().any(|x| x == "Dust: 1,00"));
    assert!(m.page_scroll < 1000.0, "clamped");
    // Sound: two footer buttons.
    let mut m = Menu::for_test();
    m.view = MenuView::Page(9);
    let t = texts(&menu_ui(&mut m, w, h, (0.0, 0.0)));
    assert!(t.iter().any(|x| x == "RESET") && t.iter().any(|x| x == "Done") && !t.iter().any(|x| x.starts_with("Gravity")));
    // Despawn animations: the list, the chosen one in a white box.
    let mut m = Menu::for_test();
    m.view = MenuView::Page(ANIM_PAGE);
    let items = menu_ui(&mut m, w, h, (0.0, 0.0));
    let t = texts(&items);
    for name in ["default", "portal", "splash", "vanish", "explosion", "poof", "bounce", "REPLAY", "Create Animation"] {
        assert!(t.iter().any(|x| x == name), "{name} in {t:?}");
    }
    assert!(items.iter().any(|u| matches!(u, SetupUi::Rect { w: 222.0, color: [255, 255, 255, 255], .. })));
}

#[test]
fn making_and_removing_a_despawn_animation() {
    let (w, h) = (640.0, 300.0);
    menu_open();
    with_menu(|m| m.view = MenuView::Page(ANIM_PAGE));
    assert_eq!(menu_mouse(w, h, 320.0, 280.0, true), MenuOut::Click, "Create Animation");
    let slot = BUILTIN_ANIMS.len();
    assert_eq!(menu_view(), MenuView::Anim(slot));
    assert!(settings().anim_used & (1 << slot) != 0);
    // The editor: Motion goes round.
    let before = settings().anims[slot].motion;
    menu_mouse(w, h, 320.0, 50.0, true);
    assert_ne!(settings().anims[slot].motion, before);
    assert_eq!(menu_mouse(w, h, 360.0, 280.0, true), MenuOut::Click, "Done");
    assert_eq!(menu_view(), MenuView::Page(ANIM_PAGE));
    // Choose it, then take it away: back to the first.
    let y = 44.0 + 24.0 * slot as f32 + 8.0;
    menu_mouse(w, h, 320.0, y, true);
    assert_eq!(settings().anim as usize, slot);
    menu_mouse(w, h, 320.0 - 110.0 + 220.0 + 38.0, y, true);
    assert!(settings().anim_used & (1 << slot) == 0);
    assert_eq!(settings().anim, 0);
    assert!(!menu_escape());
    assert_eq!(menu_view(), MenuView::Main);
    menu_close();
}

#[test]
fn page_notes_take_one_line() {
    for cat in 0..CATEGORIES.len() {
        for row in page_rows(cat) {
            if let Row::Note(t) = row {
                assert_eq!(ui_wrap(t, 300.0).len(), 1, "{t}");
            }
        }
    }
}

#[test]
fn a_hit_knocks_a_piece_away() {
    let w = Flat { pillar: None, water_above: false };
    let mut p = Physics::default();
    let (c, h, verts) = box_mesh([0.0, 0.0, 0.0, 0.5, 0.5, 0.5], [0.0, 0.0, 1.0, 1.0], 0xFFFF_FFFF);
    p.add(vec![Body::new(v3(0.0, 64.0, 2.0) + c, Quat::IDENTITY, h, verts)], Vec::new(), 30.0, None);
    run(&mut p, &w, 2.0);
    let before = p.bodies[0].x;
    // Looking the other way: nothing.
    assert!(!p.punch(v3(0.25, 64.6, 0.0), v3(0.0, 0.0, -1.0), 3.0, 6.0));
    // Looking at it (down a little): it flies off, away from the player.
    let dir = (v3(0.25, 64.25, 2.25) - v3(0.25, 65.0, 0.0)).norm();
    assert!(p.punch(v3(0.25, 65.0, 0.0), dir, 3.0, 6.0));
    run(&mut p, &w, 1.0);
    assert!(p.bodies[0].x.z > before.z + 1.0, "{:?} -> {:?}", before, p.bodies[0].x);
}
