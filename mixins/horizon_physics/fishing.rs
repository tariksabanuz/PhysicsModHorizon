// FishingHookRenderer's line as a rope: from a quarter block above the hook to the rod, it sags,
// swings and lies on blocks.
fn horizon_physics_fishing(
    game: &crate::game::Game<'_>,
    alpha: f64,
    size: [u32; 2],
) -> ::core::ops::ControlFlow<Vec<Line>> {
    use horizon_client::native_horizon_physics as hp;
    if !hp::ropes_on() {
        return ::core::ops::ControlFlow::Continue(());
    }
    let ends: Vec<(glam::DVec3, glam::DVec3)> = game
        .hook_position(alpha)
        .zip(game.rod_hand_position(alpha, size))
        .into_iter()
        .chain(game.remote_hook_ends(alpha))
        .collect();
    let width = horizon_client::gfx::outline_width(size[0]);
    let facts = game.res.facts();
    let cell = |x: i32, y: i32, z: i32| -> hp::Cell {
        let state = game.world.block(x, y, z);
        if game.world.is_air(state) {
            return hp::Cell::Air;
        }
        let f = facts.get(state);
        if f.full_collision {
            return hp::Cell::Full;
        }
        if f.legacy_solid && !f.outline.is_empty() {
            return hp::Cell::Boxes(f.outline.clone());
        }
        hp::Cell::Air
    };
    let world = hp::FnWorld(cell, |_: i32, _: i32, _: i32| 0u32);
    let mut lines = Vec::new();
    for (hook, hand) in ends {
        let a = hook + glam::DVec3::new(0.0, 0.25, 0.0);
        let pts = hp::rope(1, [a.x, a.y, a.z], [hand.x, hand.y, hand.z], 0.05, &world);
        for w in pts.windows(2) {
            lines.push(horizon_client::gizmos::Line { start: w[0], end: w[1], color: 0xFF00_0000, width });
        }
    }
    ::core::ops::ControlFlow::Break(lines)
}
