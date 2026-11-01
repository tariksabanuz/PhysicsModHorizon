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
    s.click(3);
    s.click(0);
    let t = Settings::from_text(&s.to_text());
    assert_eq!(s, t);
    assert_eq!(t.labels().len(), 7);
    assert!(t.labels()[1].ends_with("Fall apart"));
    assert!(s.click(6));
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
