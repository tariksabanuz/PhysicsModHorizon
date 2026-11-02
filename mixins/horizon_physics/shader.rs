// The terrain shader of a Horizon shader pack, with water that waves and plants that sway. If
// the result does not parse, the pack keeps the game's own shader: this must never cost the pack.
fn horizon_physics_shader(source: &str) -> ::core::ops::ControlFlow<String> {
    use crate::native_horizon_physics_sway as sway;
    let Some(entries) = sway::patch(ENTRIES) else {
        return ::core::ops::ControlFlow::Continue(());
    };
    let text = format!("{source}\n// Horizon's terrain (terrain.wgsl)\n{}{entries}", vanilla_terrain());
    if naga::front::wgsl::parse_str(&text).is_err() {
        return ::core::ops::ControlFlow::Continue(());
    }
    ::core::ops::ControlFlow::Break(text)
}
