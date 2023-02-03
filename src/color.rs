#![allow(non_snake_case)]
pub use image::{bgr, bgrf};
//let [black, white] : [Color; 2]  = [0., 1.].map(Into::into);
#[allow(non_upper_case_globals)] pub const black : bgrf = (0.).into();
#[allow(non_upper_case_globals)] pub const white : bgrf = (1.).into();
#[allow(non_upper_case_globals)] pub static dark : bool = true;
//const [background, foreground] : [Color; 2] = if dark { [black, white] } else { [white, black] };
#[allow(non_upper_case_globals)] pub fn background() -> bgrf { if dark { black } else { white } }
#[allow(non_upper_case_globals)] pub fn foreground() -> bgrf { if dark { white } else { black } }

use {num::{cos,sin,cb}, vector::xy};
pub struct LCh { pub L: f32, pub C: f32, pub h: f32}
mod vector_uv { vector::vector!(2 uv T T, u v, U V); } use vector_uv::uv;
struct Luv { L: f32, uv: uv<f32> }
impl From<LCh> for Luv { fn from(LCh{L, C, h}: LCh) -> Self { Self{L, uv: uv{u: C*cos(h), v: C*sin(h)}} } }
struct XYZ { X: f32, Y: f32, Z: f32 }
impl From<Luv> for XYZ{ fn from(Luv{L, uv}: Luv) -> Self {
	if L == 0. { return XYZ{X: 0., Y: 0., Z: 0.} }
	let n = xy{x: 0.3127, y: 0.3290}; // D65 white point (2° observer)
	let n = uv{u: 4.*n.x / (-2.*n.x + 12.*n.y + 3.), v: 9.*n.y / (-2.*n.x + 12.*n.y + 3.)};
	let uv{u,v} = n + (1./(13.*L)) * uv;
	let Y = if L < 8. { L*cb(3./29f32) } else { cb((L+16.)/116.) };
	Self{X: Y*(9.*u)/(4.*v), Y, Z: Y*(12.-3.*u-20.*v)/(4.*v)}
}}
impl From<XYZ> for bgrf { fn from(XYZ{X,Y,Z}: XYZ) -> Self { Self{
	b:   0.0557 * X - 0.2040 * Y + 1.0570 * Z,
	g: - 0.9689 * X + 1.8758 * Y + 0.0415 * Z,
	r:    3.2406 * X - 1.5372 * Y - 0.4986 * Z
}}}
impl From<LCh> for bgrf { fn from(v: LCh) -> Self { bgrf::from(XYZ::from(Luv::from(v))).clamp() } }