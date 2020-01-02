#[macro_export] macro_rules! op { ($Args:ty, $Output:ty, $Op:ident $op:ident) => (
    impl<const B : fn($Args)->$Output> std::ops::$Op<ConstFn<B>> for Box<Fn($Args)->$Output> {
        type Output = Box<Fn($Args)->$Output>;
        fn $op(self, _:ConstFn<B>) -> Self::Output { box move |args:$Args|->f32 { self(args).$op(B(args)) } }
    }
    impl<B:Fn($Args)->$Output+'static, const A : fn($Args)->$Output> std::ops::$Op<B> for ConstFn<A> {
        type Output = Box<Fn($Args)->$Output>;
        fn $op(self, b:B) -> Self::Output { box move |args:$Args| A(args).$op(b(args)) }
    }
)}

#[macro_export] macro_rules! A_op { ($Args:ty, $Output:ty, $Op:ident $op:ident, $A:ty) => (
   impl<const B : fn($Args)->$Output> std::ops::$Op<ConstFn<B>> for $A {
        type Output = Box<Fn($Args)->$Output>;
        fn $op(self, _:ConstFn<B>) -> Self::Output { box move |args:$Args|->f32 { self.$op(B(args)) } }
    }
)}

#[macro_export] macro_rules! compose { ( $Args:ty, $Output:ty, [$($Op:ident $op:ident),+] ) => (
    #[derive(Clone,Copy)] struct ConstFn<const F:fn($Args)->$Output>();
    $( op!($Args, $Output, $Op $op); A_op!($Args, $Output, $Op $op, $Output); )+
)}

pub type Operand<T> = std::marker::PhantomData<T>;
#[macro_export] macro_rules! defer_op { ( $Args:ty, $Output:ty, $Op:ident $op:ident, $A:ty) => (
        impl<const B : fn($Args)->$Output> std::ops::$Op<ConstFn<B>> for $crate::compose::Operand<$A> {
            type Output = ConstFnA<{ |a:$A, args:$Args|->$Output { a.$op(B(args)) } }>;
            fn $op(self, _:ConstFn<B>) -> Self::Output { ConstFnA() }
        }
        impl<const A : fn($Args)->$A,const B : fn($A, $Args)->$A> std::ops::$Op<ConstFnA<B>> for ConstFn<A> {
            type Output = ConstFnA<{ |a:$A, args:$Args|->$Output { A(args).$op(B(a,args)) } }>;
            fn $op(self, _:ConstFnA<B>) -> Self::Output { ConstFnA() }
        }
)}

#[macro_export] macro_rules! compose_with_defer_op { ( $Args:ty, $Output:ty, [$($Op:ident $op:ident),+], $A:ty) => (
    compose!($Args, $Output, [$($Op $op),+]);
    #[derive(Clone,Copy)] struct ConstFnA<const F:fn($A, $Args)->$Output>();
    impl<const F:fn($A, $Args)->$Output> std::ops::Deref for ConstFnA<F> { type Target = fn($A, $Args)->$Output; fn deref(&self) -> &Self::Target { &F }  }
    $( defer_op!($Args, $Output, $Op $op, $A); )+
)}
