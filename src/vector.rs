pub trait Zero { fn zero() -> Self; }
impl Zero for i32 { fn zero() -> Self { 0 } }
impl Zero for f32 { fn zero() -> Self { 0. } }

#[allow(non_camel_case_types)] #[derive(Clone, Copy, Debug)] pub struct xy<T> { pub x: T, pub y : T }
impl<T> From<(T, T)> for xy<T> { fn from(v: (T, T)) -> Self { xy{x: v.0, y: v.1} } }
impl<T:Copy> From<T> for xy<T> { fn from(v: T) -> Self { xy{x: v, y: v} } }
impl<T:Copy+Zero> Zero for xy<T> { fn zero() -> Self { T::zero().into() } }

impl<T:Eq> PartialEq<T> for xy<T> { fn eq(&self, b: &T) -> bool { self.x==*b && self.y==*b } }

use std::ops::{Add,Sub,Mul};
impl<T:Add> Add<xy<T>> for xy<T> { type Output=xy<T::Output>; fn add(self, b: xy<T>) -> Self::Output { Self::Output{x: self.x+b.x, y: self.y+b.y} } }
impl<T:Sub> Sub<xy<T>> for xy<T> { type Output=xy<T::Output>; fn sub(self, b: xy<T>) -> Self::Output { Self::Output{x: self.x-b.x, y: self.y-b.y} } }
fn mul<T:Copy+Mul>(a: T, b: xy<T>) -> xy<T::Output> { xy::<T::Output>{x: a*b.x, y: a*b.y} }
fn dot<T:Mul>(a: xy<T>, b: xy<T>) -> <<T as Mul>::Output as Add<<T as Mul>::Output>>::Output  where <T as Mul>::Output:Add<<T as Mul>::Output> { a.x*b.x + a.y*b.y }
impl<T:Mul> Mul<xy<T>> for xy<T> where <T as Mul>::Output:Add<<T as Mul>::Output> { type Output=<<T as Mul>::Output as Add<<T as Mul>::Output>>::Output; fn mul(self, b: xy<T>) -> Self::Output { dot(self, b) } }
//pub fn sq<T:Mul>(a: xy<T>) -> <<T as Mul>::Output as Add<<T as Mul>::Output>>::Output  where <T as Mul>::Output:Add<<T as Mul>::Output> { dot(a, a) }

impl Mul<xy<f32>> for f32 { type Output=xy<f32>; fn mul(self, b: xy<f32>) -> Self::Output { mul(self, b) } }
pub fn lerp(t : f32, a : xy<f32>, b : xy<f32>) -> xy<f32> { (1.-t)*a + t*b }

#[allow(non_camel_case_types)] pub type int2 = xy<i32>;
#[allow(non_camel_case_types)] pub type uint2 = xy<u32>;
#[allow(non_camel_case_types)] pub type size2 = xy<u32>;
#[allow(non_camel_case_types)] pub type offset2 = xy<u32>;
#[allow(non_camel_case_types)] pub type vec2 = xy<f32>;
