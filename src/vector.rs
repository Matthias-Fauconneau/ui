pub trait ComponentWiseMinMax {
	fn component_wise_min(self, other: Self) -> Self;
	fn component_wise_max(self, other: Self) -> Self;
}
pub fn component_wise_min<V: ComponentWiseMinMax>(a: V, b: V) -> V { a.component_wise_min(b) }
pub fn component_wise_max<V: ComponentWiseMinMax>(a: V, b: V) -> V { a.component_wise_max(b) }

impl<T:Ord> ComponentWiseMinMax for T { // /!\ semantic break if impl Ord for Vector
	fn component_wise_min(self, other: Self) -> Self { self.min(other) }
	fn component_wise_max(self, other: Self) -> Self { self.max(other) }
}

pub struct MinMax<T> { pub min: T, pub max: T }
pub trait Bounds<T> { fn bounds(self) -> Option<MinMax<T>>; }
impl<T: ComponentWiseMinMax+Copy, I:Iterator<Item=MinMax<T>>> Bounds<T> for I {
	fn bounds(self) -> Option<MinMax<T>> { self.fold_first(|MinMax{min,max}, e| MinMax{
		min: component_wise_min(min, e.min),
		max: component_wise_max(max, e.max)
	}) }
}

macro_rules! impl_Op { { $v:ident $($c:ident)+: $Op:ident $op:ident $OpAssign:ident $op_assign:ident } => {
	impl<T:$Op> $Op for $v<T> { type Output=$v<T::Output>; fn $op(self, b: Self) -> Self::Output { Self::Output{$($c: self.$c.$op(b.$c)),+} } }
	impl<T:$OpAssign> $OpAssign for $v<T> { fn $op_assign(&mut self, b: Self) { $(self.$c.$op_assign(b.$c);)+ } }
}}

#[macro_export] macro_rules! vector { ($n:literal $v:ident $($tuple:ident)+, $($c:ident)+, $($C:ident)+) => {
use std::ops::{Add,Sub,Mul,Div,AddAssign,SubAssign,MulAssign,DivAssign};
#[allow(non_camel_case_types)] #[derive(Clone, Copy, Debug, PartialEq, Eq, Default)] pub struct $v<T> { $( pub $c: T ),+ }

impl<T> From<($($tuple),+)> for $v<T> { fn from(($($c),+): ($($tuple),+)) -> Self { $v{$($c),+} } } // $tuple from $n
impl<T> From<$v<T>> for ($($tuple),+) { fn from(v : $v<T>) -> Self { ($(v.$c),+) } }
impl<T> From<[T; $n]> for $v<T> { fn from([$($c),+]: [T; $n]) -> Self { $v{$($c),+} } }
impl<T> From<$v<T>> for [T; $n] { fn from(v : $v<T>) -> Self { [$(v.$c),+] } }

impl<'t, T> From<&'t $v<T>> for [&'t T; $n] { fn from(v : &'t $v<T>) -> Self { [$(&v.$c),+] } }
impl<T> $v<T> { pub fn iter(&self) -> impl Iterator<Item=&T> { use crate::array::IntoIterator; <&Self as Into::<[&T; $n]>>::into(self).into_iter() } }
impl<T> std::iter::FromIterator<T> for $v<T> { fn from_iter<I:std::iter::IntoIterator<Item=T>>(into_iter: I) -> Self {
	use crate::array::FromIterator; <[T; $n]>::from_iter(into_iter).into()
} }

#[derive(Clone, Copy)] pub enum Component { $($C),+ }
impl Component {
	pub fn enumerate() -> impl Iterator<Item=Self> { use crate::array::IntoIterator; [$(Self::$C),+].into_iter() }
}
impl<T> std::ops::Index<Component> for $v<T> {
    type Output = T;
    fn index(&self, component: Component) -> &Self::Output {
        match component {
            $(Component::$C => &self.$c),+
        }
    }
}
pub fn $v<B>(f: impl FnMut(Component) -> B) -> $v<B> { Component::enumerate().map(f).collect() }

impl<T:Eq> PartialEq<T> for $v<T> { fn eq(&self, b: &T) -> bool { $( self.$c==*b )&&+ } }

impl<T:PartialOrd> PartialOrd for $v<T> { fn partial_cmp(&self, b: &Self) -> Option<std::cmp::Ordering> {
	Component::enumerate().map(|i| self[i].partial_cmp(&b[i])).fold_first(|c,x| if c == x { c } else { None }).flatten()
} }

impl<T:Ord> $crate::vector::ComponentWiseMinMax for $v<T> {
	fn component_wise_min(self, other: Self) -> Self { $v{$($c: self.$c .min( other.$c ) ),+} }
	fn component_wise_max(self, other: Self) -> Self { $v{$($c: self.$c .max( other.$c ) ),+} }
}

impl_Op!{$v $($c)+: Add add AddAssign add_assign}
impl_Op!{$v $($c)+: Sub sub SubAssign sub_assign}
impl_Op!{$v $($c)+: Mul mul MulAssign mul_assign}
impl_Op!{$v $($c)+: Div div DivAssign div_assign}

impl<T:Div+Copy> Div<T> for $v<T> { type Output=$v<T::Output>; fn div(self, b: T) -> Self::Output { Self::Output{$($c: self.$c/b),+} } }

impl<T:Copy> From<T> for $v<T> { fn from(v: T) -> Self { $v{$($c:v),+} } }
impl<T:Copy+$crate::num::Zero> $crate::num::Zero for $v<T> { fn zero() -> Self { T::zero().into() } }
//impl<T:Copy+$crate::num::Zero> $v<T> { pub fn zero() -> Self { $crate::num::Zero::zero() } } // vec::zero without use Zero

fn mul<T:Copy+Mul>(a: T, b: $v<T>) -> $v<T::Output> { $v{$($c: a*b.$c),+} }
fn div<T:Copy+Div>(a: T, b: $v<T>) -> $v<T::Output> { $v{$($c: a/b.$c),+} }

impl Mul<$v<u32>> for u32 { type Output=$v<u32>; fn mul(self, b: $v<u32>) -> Self::Output { mul(self, b) } }
impl Div<$v<u32>> for u32 { type Output=$v<u32>; fn div(self, b: $v<u32>) -> Self::Output { div(self, b) } }
impl Mul<$v<f32>> for f32 { type Output=$v<f32>; fn mul(self, b: $v<f32>) -> Self::Output { mul(self, b) } }
impl Div<$v<f32>> for f32 { type Output=$v<f32>; fn div(self, b: $v<f32>) -> Self::Output { div(self, b) } }
}}

vector!(2 xy T T, x y, X Y);

pub use crate::num::Zero;

impl xy<u32> { pub const fn signed(self) -> xy<i32> { xy{x: self.x as i32, y: self.y as i32} } }
impl xy<i32> { pub const fn unsigned(self) -> xy<u32> { xy{x: self.x as u32, y: self.y as u32} } }
impl From<xy<i32>> for xy<u32> { fn from(i: xy<i32>) -> Self { i.unsigned() } }
impl From<xy<u32>> for xy<i32> { fn from(u: xy<u32>) -> Self { u.signed() } }
impl From<xy<u32>> for xy<f32> { fn from(f: xy<u32>) -> Self { xy{x: f.x as f32, y: f.y as f32} } }

#[allow(non_camel_case_types)] pub type uint2 = xy<u32>;
#[allow(non_camel_case_types)] pub type int2 = xy<i32>;
#[allow(non_camel_case_types)] pub type size = xy<u32>;
#[allow(non_camel_case_types)] pub type vec2 = xy<f32>;

pub fn lerp(t: f32, a: vec2, b: vec2) -> xy<f32> { (1.-t)*a + t*b }
pub fn dot(a: vec2, b: vec2) -> f32 { a.x*b.x + a.y*b.y }
pub fn cross(a: vec2, b: vec2) -> f32 { a.x*b.y - a.y*b.x }
pub fn sq(x:vec2) -> f32 { dot(x, x) }
pub fn norm(v:vec2) -> f32 { crate::num::sqrt(sq(v)) }
pub fn atan(v:vec2) -> f32 { crate::num::atan(v.y,v.x) }

impl std::ops::Mul<uint2> for crate::num::Ratio { type Output=uint2; #[track_caller] fn mul(self, b: uint2) -> Self::Output { xy{x:self*b.x, y:self*b.y} } }
impl std::ops::Div<crate::num::Ratio> for uint2 { type Output=uint2; #[track_caller] fn div(self, r: crate::num::Ratio) -> Self::Output { xy{x:self.x/r, y:self.y/r} } }
