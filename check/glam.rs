    use std::ops::*;
#[derive(Clone, Copy, Debug, Default)]
pub struct DVec3 { pub x: f64, pub y: f64, pub z: f64 }
impl DVec3 {
    pub fn new(x: f64, y: f64, z: f64) -> Self { Self { x, y, z } }
    pub fn from_array(a: [f64; 3]) -> Self { Self::new(a[0], a[1], a[2]) }
    pub fn length_squared(self) -> f64 { self.x * self.x + self.y * self.y + self.z * self.z }
    pub fn as_vec3(self) -> Vec3 { Vec3::new(self.x as f32, self.y as f32, self.z as f32) }
}
impl Sub for DVec3 { type Output = DVec3; fn sub(self, o: DVec3) -> DVec3 { DVec3::new(self.x - o.x, self.y - o.y, self.z - o.z) } }
#[derive(Clone, Copy, Debug, Default)]
pub struct Vec3 { pub x: f32, pub y: f32, pub z: f32 }
impl Vec3 { pub fn new(x: f32, y: f32, z: f32) -> Self { Self { x, y, z } } }
impl Add for Vec3 { type Output = Vec3; fn add(self, o: Vec3) -> Vec3 { Vec3::new(self.x + o.x, self.y + o.y, self.z + o.z) } }
impl Sub for Vec3 { type Output = Vec3; fn sub(self, o: Vec3) -> Vec3 { Vec3::new(self.x - o.x, self.y - o.y, self.z - o.z) } }
impl Mul<f32> for Vec3 { type Output = Vec3; fn mul(self, k: f32) -> Vec3 { Vec3::new(self.x * k, self.y * k, self.z * k) } }
impl Neg for Vec3 { type Output = Vec3; fn neg(self) -> Vec3 { Vec3::new(-self.x, -self.y, -self.z) } }
#[derive(Clone, Copy, Debug)]
pub struct Quat(pub [f32; 4]);
impl Quat { pub fn from_xyzw(x: f32, y: f32, z: f32, w: f32) -> Self { Quat([x, y, z, w]) } }
impl Mul<Vec3> for Quat { type Output = Vec3; fn mul(self, v: Vec3) -> Vec3 { v } }
#[derive(Clone, Copy, Debug)]
pub struct Mat4;
impl Mat4 {
    pub fn from_translation(_: Vec3) -> Self { Mat4 }
    pub fn from_quat(_: Quat) -> Self { Mat4 }
    pub fn from_rotation_y(_: f32) -> Self { Mat4 }
}
impl Mul for Mat4 { type Output = Mat4; fn mul(self, _: Mat4) -> Mat4 { Mat4 } }
impl Add for DVec3 { type Output = DVec3; fn add(self, o: DVec3) -> DVec3 { DVec3::new(self.x + o.x, self.y + o.y, self.z + o.z) } }
impl Vec3 { pub const ZERO: Vec3 = Vec3 { x: 0.0, y: 0.0, z: 0.0 }; }
#[derive(Clone, Copy, Debug)]
pub enum EulerRot { ZYX }
impl Quat {
    pub fn from_rotation_x(_: f32) -> Self { Quat([0.0; 4]) }
    pub fn from_rotation_y(_: f32) -> Self { Quat([0.0; 4]) }
    pub fn from_rotation_z(_: f32) -> Self { Quat([0.0; 4]) }
    pub fn from_euler(_: EulerRot, a: f32, b: f32, c: f32) -> Self { Quat([0.0; 4]) }
    pub fn to_euler(self, _: EulerRot) -> (f32, f32, f32) { (0.0, 0.0, 0.0) }
}
impl Mul for Quat { type Output = Quat; fn mul(self, _: Quat) -> Quat { self } }
impl Mat4 { pub fn transform_point3(&self, v: Vec3) -> Vec3 { v } }
