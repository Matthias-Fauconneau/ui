#![feature(unboxed_closures,fn_traits)]
#[macro_use] extern crate framework;
#[derive(Clone,Copy)] pub struct Closure<F>(pub F); // local type for T op Fn
op!(Mul mul ((i32,i32),), f32);
op!(Sub sub ((i32,i32),), f32);
#[test] fn test() {
    let x = Closure(|(_,_):(i32,i32)| 1.);
    assert_eq!((x-1.*x)((0,0)), 0.);
}
