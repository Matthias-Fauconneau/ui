pub trait Zero { fn zero() -> Self; }
//pub fn mask<T:Zero>(m : bool, v : T) -> T { if m { v } else { Zero::zero() } }
impl Zero for u32 { fn zero() -> Self { 0 } }
impl Zero for u64 { fn zero() -> Self { 0 } }
impl Zero for usize { fn zero() -> Self { 0 } }
impl Zero for i32 { fn zero() -> Self { 0 } }
impl Zero for f32 { fn zero() -> Self { 0. } }
impl Zero for f64 { fn zero() -> Self { 0. } }
impl<T0:Zero,T1:Zero> Zero for (T0,T1) { fn zero() -> Self { (Zero::zero(),Zero::zero()) } }

pub trait Signed { fn signum(&self) -> Self; fn abs(&self) -> Self; }
macro_rules! signed_impl { ($($T:ty)+) => ($( impl Signed for $T { fn signum(&self) -> Self { <$T>::signum(*self) } fn abs(&self) -> Self { <$T>::abs(*self) } } )+) }
signed_impl!(i16 i32 f32);
#[cfg(feature="font")]
pub fn sign<T:Signed>(x : T) -> T { x.signum() }
//pub fn abs<T:Signed>(x : T) -> T { x.abs() }
//pub fn sq<T:Copy+std::ops::Mul>(x: T) -> T::Output { x*x }
//pub fn cb<T:Copy+std::ops::Mul>(x: T) -> <T::Output as std::ops::Mul<T>>::Output where <T as std::ops::Mul>::Output : std::ops::Mul<T> { x*x*x }

#[cfg(feature="font")]
pub fn floor_div(n : u32, d : u32) -> u32 { n/d }
pub fn div_truncate_towards_0(n : i32, d : i32) -> i32 { n/d }
#[cfg(any(feature="font",feature="window"))]
pub fn ceil_div(n : u32, d : u32) -> u32 { (n+d-1)/d }
pub fn div_round_towards_inf(n : i32, d : i32) -> i32 { (n+d-1)/d }
//pub fn div_rem(n : u32, d : u32) -> (u32, u32) { (n/d, n%d) }

//pub fn floor(x : f32) -> f32 { x.floor() }
//pub fn fract(x: f32) -> f32 { x.fract() }
pub fn sqrt(x: f32) -> f32 { x.sqrt() }
//pub fn cos(x: f32) -> f32 { x.cos() }
//pub fn sin(x: f32) -> f32 { x.sin() }
pub fn atan(y: f32, x: f32) -> f32 { y.atan2(x) }

#[cfg(all(feature="color",feature="sRGB"))]
pub fn clamp(x:f32) -> f32 { if x > 1. {1.} else if x < 0. {0.} else {x} }

cfg_if::cfg_if! { if #[cfg(feature="font")] {
#[derive(Clone,Copy)] pub struct Ratio { pub num: u32, pub div: u32 }
impl Ratio {
    pub fn floor(self, x: i32) -> i32 { sign(x)*floor_div(x.abs() as u32*self.num, self.div) as i32 }
    pub fn ceil(self, x: i32) -> i32 { sign(x)*ceil_div(x.abs() as u32*self.num, self.div) as i32 }
}
impl From<Ratio> for f32 { fn from(r: Ratio) -> Self { r.num as f32 / r.div as f32 } }
impl std::ops::Mul<u32> for Ratio { type Output=u32; #[track_caller] fn mul(self, b: u32) -> Self::Output { b * self.num / self.div /*-> 0*/ } }
impl std::ops::Mul<i32> for Ratio { type Output=i32; #[track_caller] fn mul(self, b: i32) -> Self::Output { b * self.num as i32 / self.div as i32 /*-> 0*/ } }
}}
