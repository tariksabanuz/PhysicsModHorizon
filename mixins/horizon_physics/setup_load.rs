// Once a frame: the settings are read as soon as the game runs (the title screen's quick setup
// needs to know whether it was done).
fn horizon_physics_setup_load(game: &mut crate::game::Game<'_>) {
    use horizon_client::native_horizon_physics as hp;
    let _ = game;
    if !hp::loaded() {
        hp::load(crate::game::Game::native_config_dir().ok());
    }
}
