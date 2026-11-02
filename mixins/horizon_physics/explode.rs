// ClientPacketListener.handleExplosion: the blast throws the blocks it breaks, with a fireball,
// sparks and smoke. A wind charge's burst (no block particles) is left alone.
fn horizon_physics_explode(game: &mut crate::game::Game<'_>, p: &play::Explode) {
    use horizon_client::native_horizon_physics as hp;
    if !game.native_window() || !game.native_on(hp::ID) || p.block_particles.is_empty() {
        return;
    }
    hp::explosion(p.center, p.radius);
}
