// The terrain shader of a Horizon shader pack, with water that waves and plants that sway. The
// game's own shader text is built first (this hook runs again inside that call and lets it
// through), then the waves and sway are put into it. If the result does not parse, the pack keeps
// the game's own shader: this must never cost the pack.
fn horizon_physics_shader(
    source: &str,
    tags: &[(u32, u32)],
    materials: &[(u32, u32)],
    vertex: bool,
    displaced: bool,
) -> ::core::ops::ControlFlow<String> {
    use crate::native_horizon_physics_sway as sway;
    if sway::shader_enter() {
        return ::core::ops::ControlFlow::Continue(());
    }
    let text = shader_source(source, tags, materials, vertex, displaced);
    sway::shader_leave();
    // The text is the pack's source, then the game's terrain: only the game's part is patched.
    let Some(rest) = text.strip_prefix(source) else {
        return ::core::ops::ControlFlow::Break(text);
    };
    let Some(patched) = sway::patch(rest, source) else {
        return ::core::ops::ControlFlow::Break(text);
    };
    let patched = format!("{source}{patched}");
    if naga::front::wgsl::parse_str(&patched).is_err() {
        return ::core::ops::ControlFlow::Break(text);
    }
    ::core::ops::ControlFlow::Break(patched)
}
