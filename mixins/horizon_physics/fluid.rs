// FluidRenderer.tesselate, then each still water surface corner it made is raised by a mark far
// too small to see (ten-thousandths of a block): how open the water is there, from its depth and
// its shore (`water_code`). The terrain shader reads it back: the sea swells, rivers lap, the
// shore and puddles stay still.
fn horizon_physics_fluid(
    tables: &MeshTables,
    nether: bool,
    pad: &Padded,
    water_color: u32,
    out: &mut SectionMesh,
    cell: [i32; 3],
) -> ::core::ops::ControlFlow<()> {
    use crate::native_horizon_physics_sway as sway;
    if !sway::marks_water() || !sway::fluid_enter() {
        return ::core::ops::ControlFlow::Continue(());
    }
    let before = out.translucent.len();
    tesselate(tables, nether, pad, water_color, out, cell);
    sway::fluid_leave();
    let info = &tables.info;
    let water = |x: i32, y: i32, z: i32| -> bool {
        if !(-2..18).contains(&x) || !(-2..18).contains(&y) || !(-2..18).contains(&z) {
            return false;
        }
        matches!(info[pad.blocks[pi(x, y, z)] as usize].fluid.kind(), FluidKind::Water | FluidKind::FlowingWater)
    };
    let y = cell[1];
    for v in &mut out.translucent[before..] {
        let h = v.pos[1] - y as f32;
        if (h - sway::STILL_TOP).abs() > 4e-5 {
            continue;
        }
        let code = sway::water_code(v.pos[0].round() as i32, y, v.pos[2].round() as i32, water, -2);
        v.pos[1] += code as f32 * sway::MARK_STEP;
    }
    ::core::ops::ControlFlow::Break(())
}
