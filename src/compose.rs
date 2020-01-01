pub trait FnRef<Args> { type Output; fn call(&self, args: Args) -> Self::Output; } // impl Fn/Mut/Once with a simpler FnRef trait
#[macro_export] macro_rules! FnRef { ($T:ident<$($Params:ident),+>) => (
    impl<$($Params),+, Args> FnOnce<Args> for $T<$($Params),+> where Self:$crate::compose::FnRef<Args> { 
        type Output=<Self as $crate::compose::FnRef<Args>>::Output;
        extern "rust-call" fn call_once(self, args: Args) -> Self::Output { <Self as Fn<Args>>::call(&self, args) } 
    }
    impl<$($Params),+, Args> FnMut<Args> for $T<$($Params),+> where Self:Fn<Args> {
        extern "rust-call" fn call_mut(&mut self, args: Args) -> Self::Output { <Self as Fn<Args>>::call(&self, args) } 
    }
    impl<$($Params),+, Args> Fn<Args> for $T<$($Params),+> where Self:$crate::compose::FnRef<Args> {
        extern "rust-call" fn call(&self, args: Args) -> Self::Output { <Self as $crate::compose::FnRef<Args>>::call(&self, args) } 
    }
)}

#[macro_export] macro_rules! Op { ($Op:ident) => (
pub struct $Op<A,B>(A,B);
impl<Args:Copy,A:Fn<Args>,B:Fn<Args>> $crate::compose::FnRef<Args> for $Op<A,B>
where Self:$crate::compose::StaticMethod<(<A as FnOnce<Args>>::Output,<B as FnOnce<Args>>::Output)> {
    type Output=<Self as $crate::compose::StaticMethod<(<A as FnOnce<Args>>::Output,<B as FnOnce<Args>>::Output)>>::Output;
    fn call(&self, args: Args) -> Self::Output {
        <Self as $crate::compose::StaticMethod<(<A as FnOnce<Args>>::Output,<B as FnOnce<Args>>::Output)>>::call((self.0.call(args), self.1.call(args)))
    }
}
FnRef!($Op<A,B>);
)}

pub trait StaticMethod<Args> {
    type Output;
    fn call(args:Args) -> Self::Output;
}
#[macro_export] macro_rules! Op_static_method { ( $Op:ident $op:ident ) => (
    impl<A:std::ops::$Op<B>,B,C,F> $crate::compose::StaticMethod<(A,B)> for $Op<C, F> { type Output=A::Output; fn call((a,b):(A,B)) -> Self::Output { a.$op(b) } }
)}

#[macro_export] macro_rules! Fn_op_Fn { ($Op:ident $op:ident $Args:ty) => (
    impl<A:Fn<$Args>,B:Fn<$Args>> std::ops::$Op<Closure<B>> for Closure<A> { 
        type Output = $Op<A,B>;
        fn $op(self, b:Closure<B>) -> Self::Output { $Op(self.0, b.0) } 
    }
    impl<A:Fn<$Args>,B:Fn<$Args>> std::ops::$Op<B> for Closure<A> { 
        type Output = $Op<A,B>;
        fn $op(self, b:B) -> Self::Output { $Op(self.0, b) } 
    }
)}

pub struct Uniform<T>(pub T);
impl<T:Copy,Args> FnRef<Args> for Uniform<T> { type Output=T; fn call(&self, _: Args) -> Self::Output { self.0 } }
FnRef!(Uniform<T>);

#[macro_export] macro_rules! Uniform_op_Closure { 
    ($Op:ident $op:ident $Args:ty, $A:ty) => (
        impl<B:Fn<$Args>> std::ops::$Op<Closure<B>> for $A {
            type Output = $Op<$crate::compose::Uniform<$A>,B>; 
            fn $op(self, b:Closure<B>) -> Self::Output {
                $Op($crate::compose::Uniform(self), b.0)
            }
        }
    )
}

#[macro_export] macro_rules! op { ($Op:ident $op:ident $Args:ty, $A:ty) => ( 
    Op!($Op);
    Op_static_method!($Op $op); 
    Fn_op_Fn!($Op $op $Args);
    Uniform_op_Closure!($Op $op $Args, $A); 
)}

#[macro_export] macro_rules! Op_op { ( $A:ident $Op:ident ) => (
    impl<AA,AB,B> std::ops::$Op<Closure<B>> for $A<AA,AB> { type Output=$Op<Self,B>; fn add(self, b: Closure<B>) -> Self::Output { $Op(self, b.0) } }  
)}
