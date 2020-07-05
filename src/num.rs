pub trait Zero { fn zero() -> Self; }
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
pub fn sign<T:Signed>(x : T) -> T { x.signum() }
pub fn abs<T:Signed>(x : T) -> T { x.abs() }
pub fn sq<T:Copy+std::ops::Mul>(x: T) -> T::Output { x*x }

pub fn div_floor(n : u32, d : u32) -> u32 { n/d }
pub fn div_ceil(n : u32, d : u32) -> u32 { (n+d-1)/d }

pub fn idiv_rem(n : i32, d : u32) -> (i32, i32) { (n/d as i32, n%d as i32) }
pub fn idiv_floor(n: i32, d: u32) -> i32 {
	let (q, r) = idiv_rem(n, d);
	if r < 0 { q - 1 } else { q }
}
pub fn idiv_ceil(n: i32, d: u32) -> i32 {
	let (q, r) = idiv_rem(n, d);
	if r > 0 { q + 1 } else { q }
}

pub fn floor(x : f32) -> f32 { x.floor() }
pub fn fract(x: f32) -> f32 { x.fract() }
pub fn sqrt(x: f32) -> f32 { x.sqrt() }
pub fn atan(y: f32, x: f32) -> f32 { y.atan2(x) }

pub fn clamp(x:f32) -> f32 { if x > 1. {1.} else if x < 0. {0.} else {x} }

#[derive(Clone,Copy,Debug)] pub struct Ratio { pub num: u32, pub div: u32 }
impl Ratio {
	pub fn ceil(&self, x: u32) -> u32 { div_ceil(x * self.num, self.div) }
	pub fn ifloor(&self, x: i32) -> i32 { idiv_floor(x * self.num as i32, self.div) }
	pub fn iceil(&self, x: i32) -> i32 { idiv_ceil(x * self.num as i32, self.div) }
}
impl From<Ratio> for f32 { fn from(r: Ratio) -> Self { r.num as f32 / r.div as f32 } }
impl std::ops::Mul<u32> for Ratio { type Output=u32; #[track_caller] fn mul(self, b: u32) -> Self::Output { div_floor(b * self.num, self.div) } }
impl std::ops::Div<Ratio> for u32 { type Output=u32; #[track_caller] fn div(self, r: Ratio) -> Self::Output { div_floor(self * r.div, r.num) } }
