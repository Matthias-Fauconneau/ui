#![allow(non_snake_case)]
use crate::{assert, core::{cos,sin,cb}, vector::{uv,xy}, image::{sRGB::sRGB, bgra8}};
pub struct LCh {pub L:f32,pub C:f32,pub h:f32}
struct Luv {L:f32,uv:uv<f32>}
impl From<LCh> for Luv { fn from(LCh{L,C,h}: LCh) -> Self { Self{L,uv:uv{u:C*cos(h),v:C*sin(h)}} } }
struct XYZ {X:f32,Y:f32,Z:f32}
impl From<Luv> for XYZ{ fn from(Luv{L,uv}: Luv) -> Self {
    if L==0. { return XYZ{X:0.,Y:0.,Z:0.} }
    std::assert!(L.is_finite() && uv.u.is_finite() && uv.v.is_finite());
    let n = xy{x:0.3127,y:0.3290}; // D65 white point (2° observer)
    let n = uv{u:4.*n.x/(-2.*n.x+12.*n.y+3.),v:9.*n.y/(-2.*n.x+12.*n.y+3.)};
    let uv{u,v} = n + (1./(13.*L)) * uv;
    assert!(u.is_finite() && v.is_finite() && L.is_finite(), u,v,L);
    let Y = if L<8. {L*cb(3./29f32)} else {cb((L+16.)/116.)};
    assert!(Y.is_finite(), Y);
    {let XYZ{X,Y,Z}=Self{X: Y*(9.*u)/(4.*v), Y, Z: Y*(12.-3.*u-20.*v)/(4.*v)}; assert!(X.is_finite() && Y.is_finite() && Z.is_finite(), X,Y,Z);}
    Self{X: Y*(9.*u)/(4.*v), Y, Z: Y*(12.-3.*u-20.*v)/(4.*v)}
}}
#[allow(non_camel_case_types)] struct bgr {b:f32,g:f32,r:f32}
impl From<XYZ> for bgr { fn from(XYZ{X,Y,Z}: XYZ) -> Self {
    std::assert!(X.is_finite() && Y.is_finite() && Z.is_finite());
Self{
    b:   0.0557 * X - 0.2040 * Y + 1.0570 * Z,
    g: - 0.9689 * X + 1.8758 * Y + 0.0415 * Z,
    r:    3.2406 * X - 1.5372 * Y - 0.4986 * Z
}}}
fn clamp(x:f32) -> f32 { if x > 1. {1.} else if x < 0. {0.} else { assert!(x>=0. && x<=1., x); x} }
impl From<bgr> for bgra8 { fn from(bgr{b,g,r}: bgr) -> Self { Self{b:sRGB(clamp(b)), g:sRGB(clamp(g)), r:sRGB(clamp(r)), a:0xFF} } }
impl From<LCh> for bgra8 { fn from(v: LCh) -> Self { (((v.into():Luv).into():XYZ).into():bgr).into() } }
