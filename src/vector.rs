macro_rules! vec { ($v:ident $($c:ident)+) => {
use {crate::core::Zero, std::ops::{Add,Sub,Mul,Div}};
#[allow(non_camel_case_types)] #[derive(Clone, Copy, Debug, PartialEq, Eq)] pub struct $v<T> { $( pub $c: T ),+ }
impl<T:Eq> PartialEq<T> for $v<T> { fn eq(&self, b: &T) -> bool { $( self.$c==*b )&&+ } }
impl<T:Copy> From<T> for $v<T> { fn from(v: T) -> Self { $v{$($c:v),+} } }
impl<T:Copy+Zero> Zero for $v<T> { fn zero() -> Self { T::zero().into() } }
impl<T:Add> Add<$v<T>> for $v<T> { type Output=$v<T::Output>; fn add(self, b: $v<T>) -> Self::Output { Self::Output{$($c: self.$c+b.$c),+} } }
impl<T:Sub> Sub<$v<T>> for $v<T> { type Output=$v<T::Output>; fn sub(self, b: $v<T>) -> Self::Output { Self::Output{$($c: self.$c-b.$c),+} } }
impl<T:Mul> Mul<$v<T>> for $v<T> { type Output=$v<T::Output>; fn mul(self, b: $v<T>) -> Self::Output { Self::Output{$($c: self.$c*b.$c),+} } }
impl<T:Div> Div<$v<T>> for $v<T> { type Output=$v<T::Output>; fn div(self, b: $v<T>) -> Self::Output { Self::Output{$($c: self.$c/b.$c),+} } }
impl<T:Div+Copy> Div<T> for $v<T> { type Output=$v<T::Output>; fn div(self, b: T) -> Self::Output { Self::Output{$($c: self.$c/b),+} } }

fn mul<T:Copy+Mul>(a: T, b: $v<T>) -> $v<T::Output> { $v{$($c: a*b.$c),+} }
fn div<T:Copy+Div>(a: T, b: $v<T>) -> $v<T::Output> { $v{$($c: a/b.$c),+} }

impl Mul<$v<f32>> for f32 { type Output=$v<f32>; fn mul(self, b: $v<f32>) -> Self::Output { mul(self, b) } }
impl Div<$v<f32>> for f32 { type Output=$v<f32>; fn div(self, b: $v<f32>) -> Self::Output { div(self, b) } }
}}

mod vec_xy {
    vec!(xy x y);
    impl<T:Ord> PartialOrd for xy<T> { fn partial_cmp(&self, b: &xy<T>) -> Option<std::cmp::Ordering> { Some(self.cmp(b)) } }
    impl<T:Ord> Ord for xy<T> { fn cmp(&self, b: &xy<T>) -> std::cmp::Ordering { // reverse lexicographic (i.e lexicographic yx)
        let ordering = self.y.cmp(&b.y);
        if ordering != std::cmp::Ordering::Equal { ordering } else { self.x.cmp(&b.x) }
    } }

    impl Into<xy<f32>> for xy<u32> { fn into(self) -> xy<f32> { xy{x: self.x as f32, y: self.y as f32} } }

    impl xy<u32> { pub const fn as_f32(self) -> xy<f32> { xy{x: self.x as f32, y: self.y as f32} } }
    pub const fn div_f32(a: f32, b: xy<f32>) -> xy<f32> { xy{x: a/b.x, y: a/b.y} }

    #[allow(non_camel_case_types)] pub type uint2 = xy<u32>;
    #[allow(non_camel_case_types)] pub type int2 = xy<i32>;
    #[allow(non_camel_case_types)] pub type size2 = xy<u32>;
    #[allow(non_camel_case_types)] pub type vec2 = xy<f32>;

    pub fn lerp(t : f32, a : vec2, b : vec2) -> xy<f32> { (1.-t)*a + t*b }
    pub fn dot(a:vec2, b:vec2) -> f32 { a.x*b.x + a.y*b.y }
    pub fn sq(x:vec2) -> f32 { dot(x, x) }
    pub fn norm(v:vec2) -> f32 { crate::core::sqrt(sq(v)) }
    pub fn atan(v:vec2) -> f32 { crate::core::atan(v.y,v.x) }
}
pub use vec_xy::*;

mod vec_uv {
    vec!(uv u v);
}
pub use vec_uv::*;
