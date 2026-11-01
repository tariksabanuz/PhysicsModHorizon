// Once a frame: the pieces take a step against the blocks around them and are drawn with the
// other entities, lit by the light where each piece is.
fn horizon_physics_world(game: &crate::game::Game<'_>, camera: glam::DVec3, out: &mut Vec<Vertex>) {
    use horizon_client::native_horizon_physics as hp;
    if !hp::loaded() {
        hp::load(crate::game::Game::native_config_dir().ok());
    }
    let on = game.native_on(hp::ID);
    hp::set_enabled(on);
    if !on {
        hp::clear();
        return;
    }
    let cell = |x: i32, y: i32, z: i32| -> hp::Cell {
        let state = game.world.block(x, y, z);
        if game.world.is_air(state) {
            return hp::Cell::Air;
        }
        let f = game.res.facts().get(state);
        if f.full_collision {
            return hp::Cell::Full;
        }
        if f.legacy_solid && !f.outline.is_empty() {
            return hp::Cell::Boxes(f.outline.clone());
        }
        if f.fluid != horizon_client::blocks::Fluid::None {
            return hp::Cell::Fluid;
        }
        hp::Cell::Air
    };
    let light = |x: i32, y: i32, z: i32| -> u32 {
        let l = game.world.light(x, y, z);
        (u32::from(l >> 4) * 16) | ((u32::from(l & 15) * 16) << 8)
    };
    let world = hp::FnWorld(cell, light);
    hp::frame(game.dim as u8, [camera.x, camera.y, camera.z], &world, out);
}
