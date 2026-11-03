// Options' first page: a wide "Physics Settings..." button above the others, which move down a
// row. Its press is the mouse hook's (it knows where the button is from here).
fn horizon_physics_options_layout(
    ui: &mut OptionsUi,
    s: &Settings,
    w: f32,
    h: f32,
) -> ::core::ops::ControlFlow<(Vec<Widget>, f32, f32, f32)> {
    use crate::native_horizon_physics_sway as sway;
    if ui.page() != Page::Main || !sway::physics_button_wanted() || !sway::layout_enter() {
        return ::core::ops::ControlFlow::Continue(());
    }
    let (mut widgets, top, bottom, content) = ui.layout(s, w, h);
    sway::layout_leave();
    let grid: Vec<usize> = widgets
        .iter()
        .enumerate()
        .filter(|e| e.1.w == 150.0 && e.1.y > 40.0 && e.1.y < h - 30.0)
        .map(|e| e.0)
        .collect();
    let gy = grid.iter().map(|i| widgets[*i].y).fold(f32::INFINITY, f32::min);
    if gy.is_finite() {
        for i in &grid {
            widgets[*i].y += 24.0;
        }
        let cx = (w / 2.0).floor();
        widgets.push(button(cx - 154.0, gy, 308.0, Text::literal("Physics Settings..."), Act::None));
        sway::set_physics_button([cx - 154.0, gy, 308.0, 20.0]);
    }
    ::core::ops::ControlFlow::Break((widgets, top, bottom, content))
}
