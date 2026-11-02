// Before the game looks at its Horizon shader pack: the sprite layers and settings go to the
// renderer's sway table, and when they changed the pack is built again with them.
fn horizon_physics_shader_pack(game: &mut crate::game::Game<'_>) {
    use horizon_client::native_horizon_physics as hp;
    use horizon_client_render::native_horizon_physics_sway as sway;
    if !hp::loaded() {
        hp::load(crate::game::Game::native_config_dir().ok());
    }
    let on = game.native_on(hp::ID);
    let st = hp::settings();
    let render = game.pack.render.clone();
    let key = std::sync::Arc::as_ptr(&render) as usize;
    let changed = sway::set(
        key,
        || render.sprite_layers.iter().map(|(n, l)| (n.as_str(), *l)).collect(),
        on && st.waves,
        on && st.plants,
        st.wind() as f32,
    );
    if changed {
        game.native_shaders.stamp = None;
        game.native_shaders.looked = None;
    }
}
