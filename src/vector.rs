#[allow(non_camel_case_types)] #[derive(Clone, Copy, Debug, PartialEq, Eq)] pub struct xy<T> { pub x: T, pub y : T }

//impl<T, U> From<xy<U>> for xy<T> { fn from(xy{x,y}: xy<U>) -> Self { xy{x: x.into(), y: y.into()} } } // conflict with impl<T> From<T> for T
//impl<T, U> Into<xy<U>> for xy<T> { fn into(self) -> xy<U> { xy{x: self.x.into(), y: self.y.into()} } } // conflict with impl<T,U> Into<U> for T where U:From<T>
//impl<T> xy<T> { fn into<U>(self) -> xy<U> { xy{x: self.x as U, y: self.y as U} } }
//impl<T> xy<T> { fn into(self) -> xy<f32> { xy{x: self.x as f32, y: self.y as f32} } }
//impl xy<u32> { fn into<U>(self) -> xy<U> { xy{x: self.x as U, y: self.y as U} } }
//impl<T> Into<xy<f32>> for xy<T> { fn into(self) -> xy<f32> { self.into::<f32>() } }
//impl<U> Into<xy<U>> for xy<u32> { fn into(self) -> xy<U> { self.into::<U>() } }
//impl Into<xy<f32>> for xy<u32> { fn into(self) -> xy<f32> { self.into::<f32>() } }
//impl Into<xy<f32>> for xy<u32> { fn into(self) -> xy<f32> { self.into::<f32>() } }
impl Into<xy<f32>> for xy<u32> { fn into(self) -> xy<f32> { xy{x: self.x as f32, y: self.y as f32} } }

impl<T> From<(T, T)> for xy<T> { fn from(v: (T, T)) -> Self { xy{x: v.0, y: v.1} } }
impl<T:Copy> From<T> for xy<T> { fn from(v: T) -> Self { (v,v).into() } }
impl<T:Copy+crate::core::Zero> crate::core::Zero for xy<T> { fn zero() -> Self { T::zero().into() } }

impl<T:Eq> PartialEq<T> for xy<T> { fn eq(&self, b: &T) -> bool { self.x==*b && self.y==*b } }

use std::ops::{Add,Sub,Mul,Div};
impl<T:Add> Add<xy<T>> for xy<T> { type Output=xy<T::Output>; fn add(self, b: xy<T>) -> Self::Output { Self::Output{x: self.x+b.x, y: self.y+b.y} } }
impl<T:Sub> Sub<xy<T>> for xy<T> { type Output=xy<T::Output>; fn sub(self, b: xy<T>) -> Self::Output { Self::Output{x: self.x-b.x, y: self.y-b.y} } }
impl<T:Mul> Mul<xy<T>> for xy<T> { type Output=xy<T::Output>; fn mul(self, b: xy<T>) -> Self::Output { Self::Output{x: self.x*b.x, y: self.y*b.y} } }
impl<T:Div> Div<xy<T>> for xy<T> { type Output=xy<T::Output>; fn div(self, b: xy<T>) -> Self::Output { Self::Output{x: self.x/b.x, y: self.y/b.y} } }

fn mul<T:Copy+Mul>(a: T, b: xy<T>) -> xy<T::Output> { xy::<T::Output>{x: a*b.x, y: a*b.y} }
fn div<T:Copy+Div>(a: T, b: xy<T>) -> xy<T::Output> { xy::<T::Output>{x: a/b.x, y: a/b.y} }

pub fn dot<T:Mul>(a: xy<T>, b: xy<T>) -> <<T as Mul>::Output as Add<<T as Mul>::Output>>::Output  where <T as Mul>::Output:Add<<T as Mul>::Output> { a.x*b.x + a.y*b.y }
/*impl<T:Mul> Mul<xy<T>> for xy<T> where <T as Mul>::Output:Add<<T as Mul>::Output> { 
    type Output=<<T as Mul>::Output as Add<<T as Mul>::Output>>::Output; 
    fn mul(self, b: xy<T>) -> Self::Output { dot(self, b) } 
}*/
//pub fn sq<T:Mul>(a: xy<T>) -> <<T as Mul>::Output as Add<<T as Mul>::Output>>::Output  where <T as Mul>::Output:Add<<T as Mul>::Output> { dot(a, a) }
        
impl Mul<xy<f32>> for f32 { type Output=xy<f32>; fn mul(self, b: xy<f32>) -> Self::Output { mul(self, b) } }
impl Div<xy<f32>> for f32 { type Output=xy<f32>; fn div(self, b: xy<f32>) -> Self::Output { div(self, b) } }

impl xy<u32> { pub fn as_f32(self) -> xy<f32> { xy{x: self.x as f32, y: self.y as f32} } }

pub fn lerp(t : f32, a : xy<f32>, b : xy<f32>) -> xy<f32> { (1.-t)*a + t*b }

#[allow(non_camel_case_types)] pub type int2 = xy<i32>;
#[allow(non_camel_case_types)] pub type uint2 = xy<u32>;
#[allow(non_camel_case_types)] pub type size2 = xy<u32>;
#[allow(non_camel_case_types)] pub type offset2 = xy<u32>;
#[allow(non_camel_case_types)] pub type vec2 = xy<f32>;

//pub fn cross<T:std::ops::Neg>(v : &xy<T>) -> xy<T> { xy{x: v.y, y: -v.x} }
