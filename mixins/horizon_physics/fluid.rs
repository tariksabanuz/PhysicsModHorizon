// Mesher.mesh_reusing, then each still water surface corner in the section it made is raised by a
// mark far too small to see (ten-thousandths of a block): how open the water is there, from its
// depth and its shore (`water_code`). The terrain shader reads it back: the sea swells, rivers
// lap, the shore and puddles stay still.
fn horizon_physics_fluid(
    mesher: &mut Mesher<'_>,
    region: &[[Option<&Chunk>; 3]; 3],
    sy: i32,
    out: &mut SectionMesh,
    scratch: &mut MeshScratch,
) -> ::core::ops::ControlFlow<SectionMesh> {
    use crate::native_horizon_physics_sway as sway;
    if !sway::marks_water() || !sway::fluid_enter() {
        return ::core::ops::ControlFlow::Continue(());
    }
    let mut done = mesher.mesh_reusing(region, sy, ::core::mem::take(out), scratch);
    sway::fluid_leave();
    let info = &mesher.tables.info;
    let pad = &scratch.padded;
    let water = |x: i32, y: i32, z: i32| -> bool {
        if !(-2..18).contains(&x) || !(-2..18).contains(&y) || !(-2..18).contains(&z) {
            return false;
        }
        matches!(info[pad.blocks[pi(x, y, z)] as usize].fluid.kind(), FluidKind::Water | FluidKind::FlowingWater)
    };
    for v in &mut done.translucent {
        let y = (v.pos[1] - sway::STILL_TOP).round() as i32;
        if (v.pos[1] - y as f32 - sway::STILL_TOP).abs() > 4e-5 {
            continue;
        }
        let (cx, cz) = (v.pos[0].round() as i32, v.pos[2].round() as i32);
        if !(water(cx, y, cz) || water(cx - 1, y, cz) || water(cx, y, cz - 1) || water(cx - 1, y, cz - 1)) {
            continue;
        }
        let code = sway::water_code(cx, y, cz, water, -2);
        v.pos[1] += code as f32 * sway::MARK_STEP;
    }
    ::core::ops::ControlFlow::Break(done)
}
