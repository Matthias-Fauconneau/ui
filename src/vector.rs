#[allow(non_camel_case_types)] #[derive(Clone, Copy, Debug, PartialEq, Eq)] pub struct xy<T> { pub x: T, pub y : T }

impl<T> From<(T, T)> for xy<T> { fn from(v: (T, T)) -> Self { xy{x: v.0, y: v.1} } }
impl<T:Eq> PartialEq<T> for xy<T> { fn eq(&self, b: &T) -> bool { self.x==*b && self.y==*b } }
impl<T:Copy> From<T> for xy<T> { fn from(v: T) -> Self { (v,v).into() } }

use {crate::core::Zero, std::ops::{Add,Sub,Mul,Div}};
impl<T:Copy+Zero> Zero for xy<T> { fn zero() -> Self { T::zero().into() } }
impl<T:Add> Add<xy<T>> for xy<T> { type Output=xy<T::Output>; fn add(self, b: xy<T>) -> Self::Output { Self::Output{x: self.x+b.x, y: self.y+b.y} } }
impl<T:Sub> Sub<xy<T>> for xy<T> { type Output=xy<T::Output>; fn sub(self, b: xy<T>) -> Self::Output { Self::Output{x: self.x-b.x, y: self.y-b.y} } }
//impl<T:std::ops::AddAssign> std::ops::AddAssign<xy<T>> for xy<T> { fn add_assign(&mut self, b: xy<T>) { self.x+=b.x; self.y+=b.y } }
//impl<T:Copy+Zero+Add<Output=T>> std::iter::Sum<xy<T>> for xy<T> { fn sum<I:Iterator<Item=xy<T>>>(iter: I) -> Self { iter.fold(Zero::zero(), Add::add) } }
impl<T:Mul> Mul<xy<T>> for xy<T> { type Output=xy<T::Output>; fn mul(self, b: xy<T>) -> Self::Output { Self::Output{x: self.x*b.x, y: self.y*b.y} } }
//impl<T:Mul> Mul<xy<T>> for xy<T> where T::Output:Add { type Output=<T::Output as Add>::Output; fn mul(self, b: xy<T>) -> Self::Output { self.x*b.x + self.y*b.y } }
impl<T:Div> Div<xy<T>> for xy<T> { type Output=xy<T::Output>; fn div(self, b: xy<T>) -> Self::Output { Self::Output{x: self.x/b.x, y: self.y/b.y} } }

fn mul<T:Copy+Mul>(a: T, b: xy<T>) -> xy<T::Output> { xy{x: a*b.x, y: a*b.y} }
fn div<T:Copy+Div>(a: T, b: xy<T>) -> xy<T::Output> { xy{x: a/b.x, y: a/b.y} }

impl Into<xy<f32>> for xy<u32> { fn into(self) -> xy<f32> { xy{x: self.x as f32, y: self.y as f32} } }
impl Mul<xy<f32>> for f32 { type Output=xy<f32>; fn mul(self, b: xy<f32>) -> Self::Output { mul(self, b) } }
impl Div<xy<f32>> for f32 { type Output=xy<f32>; fn div(self, b: xy<f32>) -> Self::Output { div(self, b) } }

impl xy<u32> { pub const fn as_f32(self) -> xy<f32> { xy{x: self.x as f32, y: self.y as f32} } }
pub const fn div_f32(a: f32, b: xy<f32>) -> xy<f32> { xy{x: a/b.x, y: a/b.y} }

#[allow(non_camel_case_types)] pub type uint2 = xy<u32>;
#[allow(non_camel_case_types)] pub type int2 = xy<i32>;
#[allow(non_camel_case_types)] pub type size2 = xy<u32>;
#[allow(non_camel_case_types)] pub type vec2 = xy<f32>;

pub fn lerp(t : f32, a : vec2, b : vec2) -> xy<f32> { (1.-t)*a + t*b }
pub fn dot(a:vec2, b:vec2) -> f32 { a.x*b.x + a.y*b.y }
pub fn sq(x:vec2) -> f32 { dot(x, x) }
