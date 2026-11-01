// A broken block falls apart into pieces of its own texture instead of the usual particles. Only
// in the window's game: the server thread has no picture to draw them in.
fn horizon_physics_destroy(
    game: &mut crate::game::Game<'_>,
    pos: [i32; 3],
    state: BlockStateId,
) -> ::core::ops::ControlFlow<()> {
    use horizon_client::native_horizon_physics as hp;
    if !game.native_window() || !game.native_on(hp::ID) || !hp::settings().blocks {
        return ::core::ops::ControlFlow::Continue(());
    }
    let Some((sprite, tint)) = game.particle_sprite(state, pos) else {
        return ::core::ops::ControlFlow::Continue(());
    };
    let Some(r) = game.gui.atlas.sprites.get(&sprite).copied() else {
        return ::core::ops::ControlFlow::Continue(());
    };
    let boxes = game.particle_shape(pos, state);
    if boxes.is_empty() {
        return ::core::ops::ControlFlow::Continue(());
    }
    hp::spawn_block(pos, &boxes, [r.u0, r.v0, r.u1, r.v1], tint);
    ::core::ops::ControlFlow::Break(())
}
