pub trait Zero { fn zero() -> Self; }
pub fn mask<T:Zero>(m : bool, v : T) -> T { if m { v } else { Zero::zero() } }
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
pub fn cb<T:Copy+std::ops::Mul>(x: T) -> <T::Output as std::ops::Mul<T>>::Output where <T as std::ops::Mul>::Output : std::ops::Mul<T> { x*x*x }

pub fn floor_div(n : u32, d : u32) -> u32 { n/d }
pub fn ceil_div(n : u32, d : u32) -> u32 { (n+d-1)/d }
pub fn div_rem(n : u32, d : u32) -> (u32, u32) { (n/d, n%d) }

pub fn floor(x : f32) -> f32 { x.floor() }
pub fn fract(x: f32) -> f32 { x.fract() }
pub fn sqrt(x: f32) -> f32 { x.sqrt() }
pub fn cos(x: f32) -> f32 { x.cos() }
pub fn sin(x: f32) -> f32 { x.sin() }
pub fn atan(y: f32, x: f32) -> f32 { y.atan2(x) }

pub trait Single: Iterator+Sized { fn single(mut self) -> Option<Self::Item> { self.next().filter(|_| self.next().is_none()) } }
impl<I:Iterator> Single for I {}

pub struct PeekingTakeWhile<'a, I:Iterator, P> { iter: &'a mut std::iter::Peekable<I>, predicate: P }
impl<'a, I:Iterator, P: FnMut(&<I as Iterator>::Item) -> bool> Iterator for PeekingTakeWhile<'a, I, P> {
    type Item = <I as Iterator>::Item;
    fn next(&mut self) -> Option<Self::Item> {
        let Self{iter, predicate} = self;
        iter.peek().filter(|x| predicate(*x))?;
        iter.next()
    }
}
pub trait PeekableExt<'a, I:Iterator> : Iterator {
    fn peeking_take_while<P:FnMut(&<Self as Iterator>::Item) -> bool>(&'a mut self, predicate: P) -> PeekingTakeWhile<'a, I, P>;
}
impl<'a, I:Iterator> PeekableExt<'a, I> for std::iter::Peekable<I> {
    fn peeking_take_while<P:FnMut(&<Self as Iterator>::Item) -> bool>(&'a mut self, predicate: P) -> PeekingTakeWhile<I, P> { PeekingTakeWhile{iter: self, predicate} }
}

#[cfg(feature="array")] pub mod array {
    pub trait FromIterator<T> { //: std::iter::FromIterator<T> {
        fn from_iter<I:std::iter::IntoIterator<Item=T>>(into_iter: I) -> Self;
    }
    impl<T, const N : usize> FromIterator<T> for [T; N] {
        fn from_iter<I>(into_iter:I) -> Self where I:std::iter::IntoIterator<Item=T> {
            let mut array : [std::mem::MaybeUninit<T>; N] = std::mem::MaybeUninit::uninit_array();
            let mut iter = into_iter.into_iter();
            for e in array.iter_mut() { e.write(iter.next().unwrap()); } // panic on short iter
            //unsafe { std::mem::transmute::<_, [T; N]>(array) } // cannot transmute between generic types
            //let array_as_initialized = unsafe { (&mut array as *mut _ as *mut [T; N]).read() };
            let array_as_initialized = unsafe { (&array as *const _ as *const [T; N]).read() };
            //let array_as_initialized = unsafe { crate::ptr::read(&array as *const _ as *const [T; N]) }
            core::mem::forget(array);
            array_as_initialized // Self(array_as_initialized)
        }
    }
    pub trait Iterator : std::iter::Iterator {
        fn collect<B: FromIterator<Self::Item>>(self) -> B where Self:Sized { FromIterator::from_iter(self) }
    }
    impl<I:std::iter::Iterator> Iterator for I {}
    // ICE traits/codegen/mod.rs:57: `Unimplemented` selecting `Binder(<std::iter::Map... as std::iter::Iterator>)` during codegen
    pub fn map<T, F:Fn(usize)->T, const N:usize>(f : F) -> [T; N] { Iterator::collect((0..N).map(f)) }
    /*pub fn map<T, F:Fn(usize)->T, const N:usize>(f : F) -> [T; N] {
        let mut array : [std::mem::MaybeUninit<T>; N] = unsafe { std::mem::MaybeUninit::uninit().assume_init() };
        for i in 0..N { array[i] = std::mem::MaybeUninit::new(f(i)) }
        let ptr = &mut array as *mut _ as *mut [T; N];
        let array_as_initialized = unsafe { ptr.read() };
        core::mem::forget(array);
        array_as_initialized
    }*/
    pub trait IntoIterator {
        type Item;
        type IntoIter: Iterator<Item = Self::Item>;
        fn into_iter(self) -> Self::IntoIter;
    }
    impl<T, const N: usize> IntoIterator for [T;N] {
        type Item = T;
        type IntoIter = IntoIter<Self::Item, N>;
        fn into_iter(self) -> Self::IntoIter { Self::IntoIter::new(self) }
    }
    pub struct IntoIter<T, const N: usize> {
        data: [std::mem::MaybeUninit<T>; N],
        alive: std::ops::Range<usize>,
    }
    impl<T, const N: usize> IntoIter<T,N> {
        pub fn new(array: [T; N]) -> Self {
            //Self{data: unsafe{std::mem::transmute::<[T;N], [std::mem::MaybeUninit<T>;N]>(array)}, alive: 0..N }
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
#[cfg(feature="array")] impl<T:Zero, const N:usize> Zero for [T; N] { fn zero() -> Self { array::map(|_|Zero::zero()) } }
#[cfg(feature="array")] pub use array::map;

pub fn log<T:std::fmt::Debug>(v: T) { println!("{:?}", v); }
#[macro_export] macro_rules! log { ($($A:expr),+) => ( $crate::core::log(($($A),+)) ) }

#[cfg(feature="anyhow")] pub use anyhow::Error;
#[cfg(not(feature="anyhow"))] #[derive(Debug)] pub struct Error(Box<dyn std::error::Error>);
#[cfg(not(feature="anyhow"))] impl<E:std::error::Error+'static/*Send+Sync*/> From<E> for Error { fn from(error: E) -> Self { Error(Box::new(error)) } }
pub type Result<T=(), E=Error> = std::result::Result<T, E>;

pub struct MessageError<M>(pub M);
impl<M:std::fmt::Debug> std::fmt::Debug for MessageError<M> { fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result { std::fmt::Debug::fmt(&self.0, f) } }
impl<M:std::fmt::Display> std::fmt::Display for MessageError<M> { fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result { std::fmt::Display::fmt(&self.0, f) } }
impl<M:std::fmt::Debug+std::fmt::Display> std::error::Error for MessageError<M> {}
pub trait Ok<T> { fn ok(self) -> Result<T>; }
impl<T> Ok<T> for Option<T> { fn ok(self) -> Result<T> { Ok(self.ok_or(MessageError("None"))?) } }

#[macro_export] macro_rules! throw { ($val:expr) => { fehler::throw!(core::MessageError(format!("{:?}", $val))); } }
//#[macro_export] macro_rules! assert { ($cond:expr, $($val:expr),* ) => { std::assert!($cond,"{}. {:?}", stringify!($cond), ( $( format!("{} = {:?}", stringify!($val), $val), )* ) ); } }
//#[macro_export] macro_rules! ensure { ($cond:expr) => { (if !$cond { throw!($crate::core::MessageError(stringify!($cond))) } } }

use std::ops::Try;
pub trait TryExtend<R:Try> { fn try_extend<I:Iterator<Item=R>>(&mut self, iter: I) -> Result<(),R::Error>; }
/*impl<R:Try> TryExtend<R> for Vec<R::Ok> {
    fn try_extend<I:Iterator<Item=R>>(&mut self, mut iter: I) -> Result<(),R::Error> {
        //let mut iter = iter.into_iter();
        self.reserve(iter.size_hint().1.unwrap());
        iter.try_for_each(move |element| { self.push(element?); Ok(()) } ).into_result() //32155
    }
}*/
// Less generic but more convenient as nothing allows inferring a Result from the generic :Try version and so it forces user to explicitly annotate iterator to yield Result<_>
impl<T, E> TryExtend<Result<T,E>> for Vec<T> {
    fn try_extend<I:IntoIterator<Item=Result<T,E>>>(&mut self, iter: I) -> Result<(),E> {
        let mut iter = iter.into_iter();
        //self.reserve(iter.size_hint().1.unwrap());
        iter.try_for_each(move |element| { self.push(element?); Ok(()) } ).into_result() //32155
    }
}

#[macro_export] macro_rules! lazy_static { ($name:ident : $T:ty = $e:expr;) => {
    #[allow(non_camel_case_types)] struct $name {}
    #[allow(non_upper_case_globals)] static $name : $name = $name{};
    impl std::ops::Deref for $name {
        type Target = $T;
        fn deref(&self) -> &Self::Target {
            #[allow(non_upper_case_globals)] static mut value : std::mem::MaybeUninit::<$T> = std::mem::MaybeUninit::<$T>::uninit();
            static INIT: std::sync::Once = std::sync::Once::new();
            unsafe{
                INIT.call_once(|| { value.write($e); });
                &value.get_ref()
            }
        }
    }
}}
