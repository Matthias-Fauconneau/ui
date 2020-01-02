#![allow(incomplete_features)]#![feature(const_generics,const_compare_raw_pointers,box_syntax)]
#[macro_use] extern crate framework;
compose_with_defer_op!((i32,i32), f32, [Add add, Sub sub, Mul mul], f32);
#[test] fn test() {
    let x = ConstFn::<{|_|1.}>();
    assert_eq!((x-1.*x+x)((0,0)), 1f32);
    let c = framework::compose::Operand::<f32>::default();
    assert_eq!((x+c*x)(1f32,(0,0)), 2f32);
}
