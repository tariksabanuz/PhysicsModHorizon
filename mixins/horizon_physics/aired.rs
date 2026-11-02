// A block the server turns to air: if an explosion did it, its pieces are blown away. The block
// is still there here, so its sprite and shape can be read.
fn horizon_physics_aired(game: &mut crate::game::Game<'_>, pos: [i32; 3], state: i32) {
    use horizon_client::native_horizon_physics as hp;
    if !hp::explosions_on() || !game.native_window() {
        return;
    }
    let old = game.world.block(pos[0], pos[1], pos[2]);
    let facts = game.res.facts();
    if facts.is_air(old) || state < 0 || !facts.is_air(BlockStateId(state as u32)) {
        return;
    }
    let Some((sprite, tint)) = game.particle_sprite(old, pos) else {
        return;
    };
    let Some(r) = game.gui.atlas.sprites.get(&sprite).copied() else {
        return;
    };
    let boxes = game.particle_shape(pos, old);
    hp::block_aired(pos, &boxes, [r.u0, r.v0, r.u1, r.v1], tint);
}
