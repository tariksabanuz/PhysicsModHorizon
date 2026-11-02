// Once a frame: the pieces take a step against the blocks around them and are drawn with the
// other entities, lit by the light where each piece is. Also looks for fire to smoke and puts
// footprints where the player walks on snow.
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
    let facts = game.res.facts();
    // Smoke: one layer of blocks around the camera each frame.
    if hp::smoke_on() {
        let (y, x0, x1, z0, z1) = hp::scan_layer([camera.x, camera.y, camera.z]);
        let mut found: Vec<([i32; 3], u8)> = Vec::new();
        for x in x0..=x1 {
            for z in z0..=z1 {
                let state = game.world.block(x, y, z);
                if game.world.is_air(state) {
                    continue;
                }
                let kind: u8 = match &*facts.get(state).name {
                    "minecraft:fire" => 1,
                    "minecraft:soul_fire" => 2,
                    "minecraft:campfire" => 3,
                    "minecraft:soul_campfire" => 4,
                    "minecraft:lava" => 5,
                    _ => continue,
                };
                if kind == 3 || kind == 4 {
                    let lit = game
                        .pack
                        .assets
                        .registry
                        .state(state)
                        .and_then(|s| s.properties.get("lit"))
                        .is_some_and(|v| v == "true");
                    if !lit {
                        continue;
                    }
                }
                if kind == 5 && !game.world.is_air(game.world.block(x, y + 1, z)) {
                    continue;
                }
                found.push(([x, y, z], kind));
            }
        }
        hp::scan_found(y, &found);
    }
    // Footprints: the player on the ground, on snow.
    if hp::snow_on()
        && let Some(p) = game.player.as_ref()
    {
        let feet = p.position;
        let mut top: Option<f64> = None;
        if p.ground.on_ground && !p.flying && !p.spectator && !p.swimming {
            let (x, z) = (feet.x.floor() as i32, feet.z.floor() as i32);
            for y in [feet.y.floor() as i32, (feet.y - 0.05).floor() as i32] {
                let state = game.world.block(x, y, z);
                let t = match &*facts.get(state).name {
                    "minecraft:snow" => {
                        let layers = game
                            .pack
                            .assets
                            .registry
                            .state(state)
                            .and_then(|s| s.properties.get("layers"))
                            .and_then(|v| v.parse::<f64>().ok())
                            .unwrap_or(1.0);
                        Some(f64::from(y) + layers / 8.0)
                    }
                    "minecraft:snow_block" | "minecraft:powder_snow" => Some(f64::from(y) + 1.0),
                    _ => None,
                };
                if let Some(t) = t
                    && (t - feet.y).abs() < 0.14
                {
                    top = Some(t);
                    break;
                }
            }
        }
        hp::footstep([feet.x, feet.y, feet.z], p.yaw, top);
    }
    // Splashes: the player's feet going into water fast.
    if hp::liquids_on()
        && let Some(p) = game.player.as_ref()
    {
        let feet = p.position;
        let state = game.world.block(feet.x.floor() as i32, (feet.y + 0.1).floor() as i32, feet.z.floor() as i32);
        let wet = facts.get(state).fluid == horizon_client::blocks::Fluid::Water;
        // Velocity is in blocks a tick.
        hp::player_water([feet.x, feet.y, feet.z], wet, -p.velocity.y * 20.0);
    }
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
    let w = game.gui.atlas.font.white();
    hp::frame(game.dim as u8, [camera.x, camera.y, camera.z], [w.u0, w.v0, w.u1, w.v1], &world, out);
}
