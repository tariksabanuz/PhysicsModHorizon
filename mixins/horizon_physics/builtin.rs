// Without a shader pack picked, Horizon draws terrain with a shader no mod can reach. So, while
// the waves or the sway are on and no pack is picked, the game runs the mod's own pack: vanilla's
// look through the material stage, which the mod's shader hook gives the waves, the water's light
// and the swaying plants. Picking any pack hands the world back to that pack.
fn horizon_physics_builtin(game: &mut crate::game::Game<'_>) {
    use horizon_client::native_horizon_physics as hp;
    use horizon_client_render::native_horizon_physics_sway as sway;
    static DEV: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    if *DEV.get_or_init(|| std::env::var_os("HORIZON_SHADER_PACK").is_some()) {
        return;
    }
    if !game.settings.horizon.shader_pack.is_empty() || !game.native_shaders.picked.is_empty() {
        hp::set_builtin_running(false);
        return;
    }
    if !hp::loaded() {
        hp::load(crate::game::Game::native_config_dir().ok());
    }
    let st = hp::settings();
    let wanted = game.native_on(hp::ID) && (st.waves || st.plants);
    if !wanted {
        if hp::builtin_running() {
            hp::set_builtin_running(false);
            game.native_shaders = super::native_shaders::NativeShaders::picking("");
            if let Some(renderer) = &mut game.renderer {
                renderer.drop_shader_packs();
            }
        }
        return;
    }
    let Some(dir) = hp::builtin_pack_dir(crate::game::Game::native_config_dir().ok()) else {
        return;
    };
    if !hp::builtin_running() {
        // Written once a session: the files on disk are the mod's, kept as this version made them.
        let files = [("horizon-shaders.toml", sway::BUILTIN_MANIFEST), ("material.wgsl", sway::BUILTIN_MATERIAL)];
        let ok = std::fs::create_dir_all(&dir).is_ok()
            && files.iter().all(|(name, text)| {
                let path = dir.join(name);
                std::fs::read_to_string(&path).is_ok_and(|t| t == *text) || std::fs::write(&path, text).is_ok()
            });
        if !ok {
            return;
        }
        hp::set_builtin_running(true);
        game.native_shaders = super::native_shaders::NativeShaders::picking("");
    }
    game.tick_native_shader_pack(&dir);
}
