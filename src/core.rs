#![allow(dead_code)]
pub fn log<T:std::fmt::Debug>(v: T) { println!("{:?}", v); }

pub struct MessageError<M>(pub M);
impl<M:std::fmt::Debug> std::fmt::Debug for MessageError<M> { fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result { std::fmt::Debug::fmt(&self.0, f) } }
impl<M:std::fmt::Display> std::fmt::Display for MessageError<M> { fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result { std::fmt::Display::fmt(&self.0, f) } }
impl<M:std::fmt::Debug+std::fmt::Display> std::error::Error for MessageError<M> {}

#[derive(Debug)] pub struct Error(Box<dyn std::error::Error>);
/*impl std::error::Error for BoxedError {
    fn backtrace(&self) -> Option<&Backtrace> { self.0.backtrace() }
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> { self.0.source() }
}*/
pub type Result<T=(), E=Error> = std::result::Result<T, E>;
impl<E:std::error::Error+'static/*Send+Sync*/> From<E> for Error { fn from(error: E) -> Self { Error(Box::new(error)) } }
#[macro_export] macro_rules! ensure { ($cond:expr, $val:expr) => { if $cond { Ok(())} else { Err(crate::core::MessageError(format!("{} = {:?}",stringify!($val),$val))) } } }
#[macro_export] macro_rules! assert {
    ($cond:expr) => { std::assert!($cond); };
    ($cond:expr, $val:expr) => { std::assert!($cond,"{}. {} = {:?}", stringify!($cond), stringify!($val), $val); }
}

pub fn abs(x : f32) -> f32 { x.abs() }
pub fn floor(x : f32) -> f32 { x.floor() }
pub fn fract(x: f32) -> f32 { x.fract() }
pub fn sq<T:Copy+std::ops::Mul>(x: T) -> T::Output { x*x }
pub fn sign(x: i16) -> i16 { if x >= 0 {1} else {-1} }

#[allow(non_camel_case_types)] #[derive(Clone, Copy, Debug/*, parse_display::Display)] #[display("{x} {y}"*/)] pub struct uint2 { pub x: u32, pub y : u32 }
#[allow(non_camel_case_types)] pub type size2 = uint2;
#[allow(non_camel_case_types)] pub type offset2 = uint2;
