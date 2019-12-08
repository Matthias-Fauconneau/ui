//pub type Result<T, E=Box<dyn std::error::Error>> = std::result::Result<T, E>;
pub type Result<T=(), E=anyhow::Error> = std::result::Result<T, E>;
#[allow(dead_code)] pub fn log<T:std::fmt::Debug>(v: T) { println!("{:?}", v); }
#[macro_export] macro_rules! ensure { ($cond:expr, $val:expr) => { anyhow::ensure!($cond,"{} = {:?}",stringify!($val),$val); } }
#[macro_export] macro_rules! assert {
    ($cond:expr) => { std::assert!($cond); };
    ($cond:expr, $val:expr) => { std::assert!($cond,"{}. {} = {:?}", stringify!($cond), stringify!($val), $val); }
}

pub fn abs(x : f32) -> f32 { x.abs() }
pub fn floor(x : f32) -> f32 { x.floor() }
pub fn fract(x: f32) -> f32 { x.fract() }
pub fn sq<T:Copy+std::ops::Mul>(x: T) -> T::Output { x*x }
