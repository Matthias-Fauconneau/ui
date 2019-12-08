#![allow(dead_code)]
//pub type Result<T, E=Box<dyn std::error::Error>> = std::result::Result<T, E>;
pub type Result<T=(), E=anyhow::Error> = std::result::Result<T, E>;
pub fn log<T:std::fmt::Debug>(v: T) { println!("{:?}", v); }
#[macro_export] macro_rules! ensure { ($cond:expr, $val:expr) => { anyhow::ensure!($cond,"{} = {:?}",stringify!($val),$val); } }
#[macro_export] macro_rules! assert {
    ($cond:expr) => { std::assert!($cond); };
    ($cond:expr, $val:expr) => { std::assert!($cond,"{}. {} = {:?}", stringify!($cond), stringify!($val), $val); }
}

pub fn abs(x : f32) -> f32 { x.abs() }
pub fn floor(x : f32) -> f32 { x.floor() }
pub fn fract(x: f32) -> f32 { x.fract() }
pub fn sq<T:Copy+std::ops::Mul>(x: T) -> T::Output { x*x }
pub fn sign(x: i16) -> i16 { if x >= 0 {1} else {-1} }

#[allow(non_camel_case_types)] #[derive(Clone, Copy, Debug, parse_display::Display)] #[display("{x} {y}")] pub struct uint2 { pub x: u32, pub y : u32 }
//impl From<(u32,u32)> for uint2 { fn from(v : (u32, u32)) -> Self { Self{x:v.0,y:v.1} } }
#[allow(non_camel_case_types)] pub type size2 = uint2;
#[allow(non_camel_case_types)] pub type offset2 = uint2;
