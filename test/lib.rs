pub mod mesher {
    #[repr(C)]
    #[derive(Clone, Copy, Debug)]
    pub struct Vertex {
        pub pos: [f32; 3],
        pub uv: [f32; 2],
        pub layer: u32,
        pub color: u32,
        pub light: u32,
        pub normal: [i8; 4],
        pub glint_uv: [f32; 2],
    }
}
pub mod native_horizon_physics {
    include!("../mixins/horizon_physics/runtime.rs");
}
pub mod native_horizon_physics_sway {
    include!("../mixins/horizon_physics/sway.rs");
}
#[cfg(test)]
mod tests;
