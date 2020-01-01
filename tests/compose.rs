#![feature(unboxed_closures,fn_traits)]
#![allow(incomplete_features)]#![feature(impl_trait_in_bindings)]
#[macro_use] extern crate framework;
#[derive(Clone,Copy)] pub struct Closure<F>(pub F); // local type for T op Fn
trait LocalFn<T> : Fn<T> {}
op!(Add add (i32,i32), f32);
op!(Sub sub (i32,i32), f32); Op_op!(Sub Add);
op!(Mul mul (i32,i32), f32);
#[test] fn test() {
    let x = Closure(|_:i32,_:i32| 1.);
    assert_eq!((x-1.*x+x)(0,0), 1.);
}
