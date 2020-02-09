pub trait Zero { fn zero() -> Self; }
pub fn mask<T:Zero>(m : bool, v : T) -> T { if m { v } else { Zero::zero() } }
impl Zero for u32 { fn zero() -> Self { 0 } }
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
pub fn cb<T:Copy+std::ops::Mul>(x: T) -> <T::Output as std::ops::Mul<T>>::Output where <T as std::ops::Mul>::Output : std::ops::Mul<T> { x*x*x }

pub fn div_rem(n : u32, d : u32) -> (u32, u32) { (n/d, n%d) }

pub fn floor(x : f32) -> f32 { x.floor() }
pub fn fract(x: f32) -> f32 { x.fract() }
pub fn sqrt(x: f32) -> f32 { x.sqrt() }
pub fn cos(x: f32) -> f32 { x.cos() }
pub fn sin(x: f32) -> f32 { x.sin() }
pub fn atan(y: f32, x: f32) -> f32 { y.atan2(x) }

#[cfg(feature="const_generics")] pub mod array {
    pub trait FromIterator<T> { //: std::iter::FromIterator<T> {
        fn from_iter<I:IntoIterator<Item=T>>(into_iter: I) -> Self;
    }
    impl<T, const N : usize> FromIterator<T> for [T; N] {
        fn from_iter<I>(into_iter:I) -> Self where I:IntoIterator<Item=T> {
            let mut array : [std::mem::MaybeUninit<T>; N] = unsafe { std::mem::MaybeUninit::uninit().assume_init() };
            let mut iter = into_iter.into_iter();
            for e in array.iter_mut() { *e = std::mem::MaybeUninit::new(iter.next().unwrap()); } // panic on short iter
            //unsafe { std::mem::transmute::<_, [T; N]>(array) } // cannot transmute between generic types
            let ptr = &mut array as *mut _ as *mut [T; N];
            let array_as_initialized = unsafe { ptr.read() };
            core::mem::forget(array);
            array_as_initialized // Self(array_as_initialized)
        }
    }
    pub trait Iterator : std::iter::Iterator {
        fn collect<B: FromIterator<Self::Item>>(self) -> B where Self:Sized { FromIterator::from_iter(self) }
    }
    impl<I:std::iter::Iterator> Iterator for I {}
    // ICE traits/codegen/mod.rs:57: `Unimplemented` selecting `Binder(<std::iter::Map... as std::iter::Iterator>)` during codegen
    //pub fn map<T, F:Fn(usize)->T, const N:usize>(f : F) -> [T; N] { Iterator::collect((0..N).map(f)) }
    pub fn map<T, F:Fn(usize)->T, const N:usize>(f : F) -> [T; N] {
        let mut array : [std::mem::MaybeUninit<T>; N] = unsafe { std::mem::MaybeUninit::uninit().assume_init() };
        for i in 0..N { array[i] = std::mem::MaybeUninit::new(f(i)) }
        let ptr = &mut array as *mut _ as *mut [T; N];
        let array_as_initialized = unsafe { ptr.read() };
        core::mem::forget(array);
        array_as_initialized
    }
    pub struct IntoIter<T, const N: usize> {
        data: [std::mem::MaybeUninit<T>; N],
        alive: std::ops::Range<usize>,
    }
    impl<T, const N: usize> IntoIter<T,N> {
        pub fn new(array: [T; N]) -> Self {
            //Self{data: std::mem::transmute::<[T;N], [std::mem::MaybeUninit<T>;N]>(array), alive: 0..N }
            Self{data: unsafe{let data = std::ptr::read(&array as *const [T; N] as *const [std::mem::MaybeUninit<T>; N]); std::mem::forget(array); data}, alive: 0..N}
        }
        fn as_mut_slice(&mut self) -> &mut [T] { unsafe { std::mem::transmute::<&mut [std::mem::MaybeUninit<T>], &mut [T]>(&mut self.data[self.alive.clone()]) } }
    }
    impl<T, const N: usize> std::iter::Iterator for IntoIter<T, N> {
        type Item = T;
        fn next(&mut self) -> Option<Self::Item> {
            if self.alive.start == self.alive.end { return None; }
            let idx = self.alive.start;
            self.alive.start += 1;
            Some(unsafe { self.data.get_unchecked(idx).read() })
        }
    }
    impl<T, const N: usize> Drop for IntoIter<T,N> { fn drop(&mut self) { unsafe { std::ptr::drop_in_place(self.as_mut_slice()) } } }
}
#[cfg(feature="const_generics")] impl<T:Zero, const N:usize> Zero for [T; N] { fn zero() -> Self { array::map(|_|Zero::zero()) } }
#[cfg(feature="const_generics")] pub use array::map;

pub fn log<T:std::fmt::Debug>(v: T) { println!("{:?}", v); }
#[macro_export] macro_rules! log { ($($A:expr),+) => ( $crate::core::log(($($A),+)) ) }

pub struct MessageError<M>(pub M);
impl<M:std::fmt::Debug> std::fmt::Debug for MessageError<M> { fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result { std::fmt::Debug::fmt(&self.0, f) } }
impl<M:std::fmt::Display> std::fmt::Display for MessageError<M> { fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result { std::fmt::Display::fmt(&self.0, f) } }
impl<M:std::fmt::Debug+std::fmt::Display> std::error::Error for MessageError<M> {}
pub trait Ok<T> { fn ok(self) -> Result<T>; }
impl<T> Ok<T> for Option<T> { fn ok(self) -> Result<T> { Ok(self.ok_or(MessageError("None"))?) } }

#[derive(Debug)] pub struct Error(Box<dyn std::error::Error>);
pub type Result<T=(), E=Error> = std::result::Result<T, E>;
impl<E:std::error::Error+'static/*Send+Sync*/> From<E> for Error { fn from(error: E) -> Self { Error(Box::new(error)) } }
//#[macro_export] macro_rules! ensure { ($cond:expr, $val:expr) => { (if $cond { Ok(())} else { Err(crate::core::MessageError(format!("{} = {:?}",stringify!($val),$val))) })? } }
#[macro_export] macro_rules! assert { ($cond:expr, $($val:expr),* ) => { std::assert!($cond,"{}. {:?}", stringify!($cond), ( $( format!("{} = {:?}", stringify!($val), $val), )* ) ); } }
