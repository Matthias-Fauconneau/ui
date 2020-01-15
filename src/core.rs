#![allow(dead_code)]

pub trait Zero { fn zero() -> Self; }
pub fn mask<T:Zero>(m : bool, v : T) -> T { if m { v } else { Zero::zero() } }
impl Zero for i32 { fn zero() -> Self { 0 } }
impl Zero for f32 { fn zero() -> Self { 0. } }
//impl<T:Zero> Zero for (T, T) { fn zero() -> Self { (Zero::zero(), Zero::zero()) } }

pub trait Signed { fn signum(&self) -> Self; fn abs(&self) -> Self; }
macro_rules! signed_impl { ($($T:ty)+) => ($( impl Signed for $T { fn signum(&self) -> Self { <$T>::signum(*self) } fn abs(&self) -> Self { <$T>::abs(*self) } } )+) }
signed_impl!(i16 i32 f32);
pub fn sign<T:Signed>(x : T) -> T { x.signum() }
pub fn abs<T:Signed>(x : T) -> T { x.abs() }

pub fn floor(x : f32) -> f32 { x.floor() }
pub fn fract(x: f32) -> f32 { x.fract() }
pub fn sqrt(x: f32) -> f32 { x.sqrt() }
pub fn sq<T:Copy+std::ops::Mul>(x: T) -> T::Output { x*x }
pub fn cb<T:Copy+std::ops::Mul>(x: T) -> <T::Output as std::ops::Mul<T>>::Output where <T as std::ops::Mul>::Output : std::ops::Mul<T> { x*x*x }

#[cfg(feature="const_generics")] pub mod array {
    struct Type<T>(T);
    impl<T, const N : usize> std::iter::FromIterator<T> for Type<[T; N]> {
        fn from_iter<I>(into_iter: I) -> Self where I: IntoIterator<Item=T> {
            let mut array : [std::mem::MaybeUninit<T>; N] = unsafe { std::mem::MaybeUninit::uninit().assume_init() };
            let mut iter = into_iter.into_iter();
            for e in array.iter_mut() { *e = std::mem::MaybeUninit::new(iter.next().unwrap()); } // panic on short iter
            //unsafe { std::mem::transmute::<_, [T; N]>(array) } // cannot transmute between generic types
            let ptr = &mut array as *mut _ as *mut [T; N];
            let array_as_initialized = unsafe { ptr.read() };
            core::mem::forget(array);
            Self(array_as_initialized)
        }
    }
    pub fn collect<T, F : Fn(usize)->T, const N : usize>(f : F) -> [T; N] { (0..N).map(f).collect::<Type<[T;N]>>().0 }
}
#[cfg(feature="const_generics")] impl<T:Zero, const N:usize> Zero for [T; N] { fn zero() -> Self { array::collect(|_|Zero::zero()) } }
//#[cfg(feature="const_generics")] pub use array::collect;
//pub fn default<T : Default>(len : usize) -> Vec<T> { let mut v=Vec::new(); v.resize_with(len, T::default); v }

//pub trait FnRef<Args> { type Output; fn call(&self, args: Args) -> Self::Output; } // impl Fn/Mut/Once with a simpler FnRef trait

pub fn log<T:std::fmt::Debug>(v: T) { println!("{:?}", v); }

pub struct MessageError<M>(pub M);
impl<M:std::fmt::Debug> std::fmt::Debug for MessageError<M> { fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result { std::fmt::Debug::fmt(&self.0, f) } }
impl<M:std::fmt::Display> std::fmt::Display for MessageError<M> { fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result { std::fmt::Display::fmt(&self.0, f) } }
impl<M:std::fmt::Debug+std::fmt::Display> std::error::Error for MessageError<M> {}
pub trait Ok<T> { fn ok(self) -> Result<T>; }
impl<T> Ok<T> for Option<T> { fn ok(self) -> Result<T> { Ok(self.ok_or(MessageError("None"))?) } }

#[derive(Debug)] pub struct Error(Box<dyn std::error::Error>);
pub type Result<T=(), E=Error> = std::result::Result<T, E>;
impl<E:std::error::Error+'static/*Send+Sync*/> From<E> for Error { fn from(error: E) -> Self { Error(Box::new(error)) } }
#[macro_export] macro_rules! ensure { ($cond:expr, $val:expr) => { if $cond { Ok(())} else { Err(crate::core::MessageError(format!("{} = {:?}",stringify!($val),$val))) } } }
#[macro_export] macro_rules! assert { ($cond:expr, $($val:expr),* ) => { std::assert!($cond,"{}. {:?}", stringify!($cond), ( $( format!("{} = {:?}", stringify!($val), $val), )* ) ); } }
