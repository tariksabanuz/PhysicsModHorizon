// Type-checks the bundle's helpers against stubs that copy the real signatures they touch
// (Horizon 9d979caa). Each helper sits in its target's module, as horizon-mixins puts it.
#![allow(dead_code, unused_variables, clippy::all)]
extern crate self as horizon_client;

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
    //RUNTIME//
}

pub mod blocks {
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum Fluid { None, Water, Lava }
    pub struct StateFacts {
        pub full_collision: bool,
        pub legacy_solid: bool,
        pub fluid: Fluid,
        pub outline: std::sync::Arc<[[f64; 6]]>,
    }
    pub struct BlockFacts(pub Vec<StateFacts>);
    impl BlockFacts { pub fn get(&self, id: super::BlockStateId) -> &StateFacts { &self.0[id.0 as usize] } }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BlockStateId(pub u32);

pub mod geometry {
    #[derive(Clone, Copy, Debug, Default)]
    pub struct Vec3 { pub x: f64, pub y: f64, pub z: f64 }
}

pub mod mobs {
    use crate::geometry::Vec3;
    pub struct Mob { pub id: u32, pub position: Vec3, pub previous: Vec3, pub health: f32 }
    impl Mob { pub fn alive(&self) -> bool { self.health > 0.0 } }
    pub struct MobData;
    pub struct Part;
    pub struct ModelNormals;
    pub struct Poses;
    pub mod render {
        use super::{Mob, MobData, ModelNormals, Part, Poses};
        use crate::mesher::Vertex;
        pub type Atlas<'a> = &'a dyn Fn(&str) -> Option<[f32; 4]>;
        pub fn render_with(
            m: &Mob,
            alpha: f32,
            camera: glam::DVec3,
            atlas: Atlas,
            light: u32,
            data: &MobData,
            extra: &[(&Part, [f32; 4], u32, u32)],
            out: &mut Vec<Vertex>,
        ) {
            //MOB//
            match __h_mob(m, alpha, camera, atlas, light, data, extra, &mut *out) {
                ::core::ops::ControlFlow::Break(()) => return,
                ::core::ops::ControlFlow::Continue(()) => {}
            }
            draw_normals(&Part, ModelNormals, &Poses, [0.0; 4], 0.0, 0, light, 0, out);
        }
        fn draw_normals(
            part: &Part,
            normals: ModelNormals,
            poses: &Poses,
            uv: [f32; 4],
            scroll: f32,
            color: u32,
            light: u32,
            overlay: u32,
            out: &mut Vec<Vertex>,
        ) {
            //PART//
            __h_part(&*out);
        }
    }
}

pub mod hud {
    #[derive(Clone, Copy, Debug)]
    pub struct Rect { pub u0: f32, pub v0: f32, pub u1: f32, pub v1: f32, pub w: u32, pub h: u32 }
}

pub mod item_render {
    use glam::{Mat4, Vec3};
    pub struct ItemRenderState;
    impl ItemRenderState {
        pub fn is_empty(&self) -> bool { false }
        pub fn bounding_box(&self) -> (Vec3, Vec3) { (Vec3::default(), Vec3::default()) }
    }
    pub fn item_spin(age: f32, bob: f32) -> f32 { age + bob }
    pub fn rendered_amount(count: i32) -> usize { count as usize }
    pub fn cluster_poses(state: &ItemRenderState, pose: Mat4, amount: usize, seed: i64) -> Vec<Mat4> { vec![pose; amount] }
}

pub mod game {
    use crate::{BlockStateId, blocks::BlockFacts, hud::Rect, mesher::Vertex};
    use std::collections::HashMap;
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    enum Dim { Overworld, Nether, End }
    pub struct Resources { facts: BlockFacts }
    impl Resources { fn facts(&self) -> &BlockFacts { &self.facts } }
    pub struct HudAtlas { pub sprites: HashMap<String, Rect> }
    pub struct UiResources { pub atlas: HudAtlas }
    pub struct World;
    impl World {
        pub fn block(&self, x: i32, y: i32, z: i32) -> BlockStateId { BlockStateId(0) }
        pub fn is_air(&self, s: BlockStateId) -> bool { s.0 == 0 }
        pub fn light(&self, x: i32, y: i32, z: i32) -> u8 { 0xF0 }
    }
    pub struct ItemStack { pub item: String, pub damage: i32, pub count: i32 }
    pub struct ItemEntity {
        pub id: u32,
        pub stack: ItemStack,
        pub position: crate::geometry::Vec3,
        pub previous: crate::geometry::Vec3,
        pub velocity: crate::geometry::Vec3,
        pub on_ground: bool,
        pub age: u32,
        pub bob_offset: f32,
    }
    pub struct ItemRenderer;
    impl ItemRenderer { pub fn item_id(&self, item: &str) -> i32 { 0 } }
    pub struct PackResources { pub item_renderer: ItemRenderer }
    pub struct Game<'a> {
        res: std::sync::Arc<Resources>,
        gui: UiResources,
        world: World,
        dim: Dim,
        dropped: Vec<ItemEntity>,
        pickups: pickups::Pickups,
        pack: PackResources,
        _l: std::marker::PhantomData<&'a ()>,
    }
    impl Game<'_> {
        fn particle_sprite(&self, state: BlockStateId, pos: [i32; 3]) -> Option<(String, u32)> { None }
        fn particle_shape(&self, pos: [i32; 3], state: BlockStateId) -> Vec<[f64; 6]> { Vec::new() }
        fn destroy_particles(&mut self, pos: [i32; 3], state: BlockStateId) {
            //DESTROY//
            match __h_destroy(&mut *self, pos, state) {
                ::core::ops::ControlFlow::Break(()) => return,
                ::core::ops::ControlFlow::Continue(()) => {}
            }
        }
    }
    pub mod pickups {
        use super::ItemEntity;
        pub(super) enum Taken { Item(Box<ItemEntity>), Orb(u32) }
        pub(super) struct Pickup { pub(super) taken: Taken }
        #[derive(Default)]
        pub(super) struct Pickups { pub(super) live: Vec<Pickup> }
        pub(super) fn position(p: &Pickup, partial: f64) -> [f64; 3] { [0.0; 3] }
    }
    pub mod glowing {
        pub(in crate::game) fn team_color(t: Option<u32>) -> u32 { 0 }
    }
    pub mod native_hooks {
        use super::Game;
        pub(crate) struct NativeMod {
            pub(crate) id: String,
            pub(crate) title: String,
            pub(crate) description: String,
            pub(crate) keys: Vec<[String; 3]>,
        }
        impl Game<'_> {
            pub(super) fn native_mods(&mut self, mods: &mut Vec<NativeMod>) {
                //LISTED//
                __h_listed(&mut *mods);
            }
            pub(super) fn native_settings(&mut self, mod_id: &str, buttons: &mut Vec<String>) {
                //SETTINGS//
                __h_settings(mod_id, &mut *buttons);
            }
            pub(super) fn native_settings_click(&mut self, mod_id: &str, button: usize) {
                //SETTINGS_CLICK//
                __h_settings_click(mod_id, button);
            }
            pub(super) fn native_on(&self, mod_id: &str) -> bool { true }
            pub(super) fn native_window(&self) -> bool { true }
            pub(super) fn native_config_dir() -> Result<std::path::PathBuf, String> { Err(String::new()) }
            pub(super) fn entity_glowing(&self, id: u32) -> bool { false }
            pub(super) fn mob_team(&self, id: u32) -> Option<u32> { None }
            pub(super) fn mark_outline(&self, v: &[crate::mesher::Vertex], rgb: u32) {}
            pub(super) fn cull_entity_vertices(&self, v: &mut Vec<crate::mesher::Vertex>, start: usize) {}
        }
    }
    pub mod end {
        use super::{Dim, Game};
        use horizon_client::mesher::Vertex;
        impl Game<'_> {
            pub(super) fn end_geometry(&self, camera: glam::DVec3, out: &mut Vec<Vertex>) {
                //WORLD//
                __h_world(self, camera, &mut *out);
            }
        }
    }
    pub mod item_draw {
        use super::{Dim, Game};
        use horizon_client::{item_render::{self, ItemRenderState}, mesher::Vertex};
        pub(super) struct DisplayContext;
        #[allow(non_upper_case_globals)]
        impl DisplayContext { pub const Ground: DisplayContext = DisplayContext; }
        pub(super) struct OwnerState;
        pub(super) fn owner_at(position: [f64; 3], yaw: f32, t: &str) -> OwnerState { OwnerState }
        pub fn vertex_light(packed: u8) -> u32 { 0 }
        impl Game<'_> {
            pub(super) fn resolve_item(&self, s: &super::ItemStack, c: DisplayContext, o: Option<&OwnerState>, seed: i32) -> ItemRenderState { ItemRenderState }
            pub(super) fn submit_item_world(&self, s: &ItemRenderState, pose: glam::Mat4, light: u32, out: &mut Vec<Vertex>) {}
            pub(super) fn dropped_item_geometry(&self, camera: glam::DVec3, alpha: f64, out: &mut Vec<Vertex>) {
                //ITEMS//
                match __h_items(self, camera, alpha, &mut *out) {
                    ::core::ops::ControlFlow::Break(()) => return,
                    ::core::ops::ControlFlow::Continue(()) => {}
                }
            }
        }
    }
}
