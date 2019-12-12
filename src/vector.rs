#[allow(non_camel_case_types)] #[derive(Clone, Copy, Debug/*, parse_display::Display)] #[display("{x} {y}"*/)] pub struct uint2 { pub x: u32, pub y : u32 }
#[allow(non_camel_case_types)] pub type size2 = uint2;
#[allow(non_camel_case_types)] pub type offset2 = uint2;

#[allow(non_camel_case_types)] #[derive(Clone,Copy,Debug/*,parse_display::Display)] #[display("{x} {y}"*/)] pub struct vec2{pub x: f32, pub y: f32}
impl From<(f32, f32)> for vec2 { fn from(v: (f32, f32)) -> Self { vec2{x: v.0, y: v.1} } }
fn mul(a: f32, b: vec2) -> vec2 { vec2{x: a*b.x, y: a*b.y} }
fn add(a: vec2, b: vec2) -> vec2 { vec2{x: a.x+b.x, y: a.y+b.y} }
fn sub(a: vec2, b: vec2) -> vec2 { vec2{x: a.x-b.x, y: a.y-b.y} }
fn dot(a: vec2, b: vec2) -> f32 { a.x*b.x + a.y*b.y }
#[allow(non_camel_case_types)] pub struct float(pub f32); // scalar
impl From<f32> for float { fn from(s: f32) -> Self { float(s) } }
impl std::ops::Mul<vec2> for float { type Output=vec2; fn mul(self, b: vec2) -> Self::Output { mul(self.0, b) } }
impl std::ops::Mul<f32> for vec2 { type Output=Self; fn mul(self, b: f32) -> Self::Output { mul(b, self) } }
impl std::ops::Add<vec2> for vec2 { type Output=Self; fn add(self, b: vec2) -> Self::Output { add(self, b) } }
impl std::ops::Sub<vec2> for vec2 { type Output=Self; fn sub(self, b: vec2) -> Self::Output { sub(self, b) } }
impl std::ops::Mul<vec2> for vec2 { type Output=f32; fn mul(self, b: vec2) -> Self::Output { dot(self, b) } }
pub fn lerp(t : f32, a : vec2, b : vec2) -> vec2 { float(1.-t)*a + float(t)*b }
