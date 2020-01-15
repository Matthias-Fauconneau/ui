use std::rc::Rc;
pub struct RcFn<'a,Args,Output>(pub Rc<dyn Fn<Args,Output=Output> + 'a>);
impl<Args,Output> FnOnce<Args> for RcFn<'_,Args,Output> { 
    type Output=Output;
    extern "rust-call" fn call_once(self, args:Args) -> Self::Output { self.0.call(args) }
}
impl<Args,Output> FnMut<Args> for RcFn<'_,Args,Output> { extern "rust-call" fn call_mut(&mut self, args:Args) -> Self::Output { self.0.call(args) } }
impl<Args,Output> Fn<Args> for RcFn<'_,Args,Output> { extern "rust-call" fn call(&self, args:Args) -> Self::Output { self.0.call(args) } }
impl<'a,Args,Output> RcFn<'a,Args,Output> { pub fn new<F:Fn<Args,Output=Output>+'a>(f:F) -> Self { Self(Rc::new(f)) } } // type alias hides constructor
impl<'a,Args,Output> Clone for RcFn<'a,Args,Output> { fn clone(&self) -> Self { Self(Rc::clone(&self.0)) } } // #[derive(Clone)] fails

macro_rules! unary { ([$($Op:ident $op:ident),+]) => (
mod unary {$(
    pub struct $Op<'a,Args,A>(pub super::RcFn<'a,Args,A>);
    impl<Args,A> FnOnce<Args> for $Op<'_,Args,A> where A:std::ops::$Op {
        type Output = <A as std::ops::$Op>::Output;
        extern "rust-call" fn call_once(self, args:Args) -> Self::Output { Self::call(&self, args) }
    }
   impl<Args,A> FnMut<Args> for $Op<'_,Args,A> where A:std::ops::$Op { extern "rust-call" fn call_mut(&mut self, args:Args) -> Self::Output { Self::call(&self, args) } }
   impl<Args,A> Fn<Args> for $Op<'_,Args,A> where A:std::ops::$Op { extern "rust-call" fn call(&self, args:Args) -> Self::Output { self.0.call(args).$op() } }
)+}
$(
    impl<'a,Args:'static,A:'static> std::ops::$Op for RcFn<'a,Args,A> where A:std::ops::$Op {
        type Output = RcFn<'a,Args,<<Self as FnOnce<Args>>::Output as std::ops::$Op>::Output>;
        fn $op(self) -> Self::Output { RcFn::new(unary::$Op(self)) }
    }
)+
)}

macro_rules! binary { ([$($Op:ident $op:ident),+] [/*$(*/$Uniform:ty/*),+*/]) => ( // Uniform+ is possible but complicated, not needed for now
mod binary {$(
    pub struct $Op<'a,Args,A,B>(pub super::RcFn<'a,Args,A>, pub super::RcFn<'a,Args,B>);
    impl<Args:Copy,A,B> FnOnce<Args> for $Op<'_,Args,A,B> where A:std::ops::$Op<B> {
        type Output = <A as std::ops::$Op<B>>::Output;
        extern "rust-call" fn call_once(self, args:Args) -> Self::Output { Self::call(&self, args) }
    }
    impl<Args:Copy,A,B> FnMut<Args> for $Op<'_,Args,A,B> where A:std::ops::$Op<B> { extern "rust-call" fn call_mut(&mut self, args:Args) -> Self::Output { Self::call(&self, args) } }
    impl<Args:Copy,A,B> Fn<Args> for $Op<'_,Args,A,B> where A:std::ops::$Op<B> { extern "rust-call" fn call(&self, args:Args) -> Self::Output { self.0.call(args).$op(self.1.call(args)) } }
)+}
$(
    impl<'a,Args:Copy+'static,B:'static,A:'static> std::ops::$Op<RcFn<'a,Args,B>> for RcFn<'a,Args,A> where A:std::ops::$Op<B> {
        type Output = RcFn<'a,Args, <binary::$Op<'a,Args,A,B> as FnOnce<Args>>::Output>;
        fn $op(self, b:RcFn<'a,Args,B>) -> Self::Output { RcFn::new(binary::$Op(self,b)) }
    }
    impl<'a,Args:Copy+'static,B:'static,A:'static> std::ops::$Op<RcFn<'a,Args,B>> for &RcFn<'a,Args,A> where A:std::ops::$Op<B> {
        type Output = RcFn<'a,Args, <binary::$Op<'a,Args,A,B> as FnOnce<Args>>::Output>;
        fn $op(self, b:RcFn<'a,Args,B>) -> Self::Output { RcFn::new(binary::$Op(RcFn::clone(self),b)) }
    }
    impl<'a,Args:Copy+'static,B:'static,A:'static> std::ops::$Op<&RcFn<'a,Args,B>> for RcFn<'a,Args,A> where A:std::ops::$Op<B> {
        type Output = RcFn<'a,Args, <binary::$Op<'a,Args,A,B> as FnOnce<Args>>::Output>;
        fn $op(self, b:&RcFn<'a,Args,B>) -> Self::Output { RcFn::new(binary::$Op(self,RcFn::clone(&b))) }
    }
    impl<'a,Args:Copy+'static,B:'static,A:'static> std::ops::$Op<&RcFn<'a,Args,B>> for &RcFn<'a,Args,A> where A:std::ops::$Op<B> {
        type Output = RcFn<'a,Args, <binary::$Op<'a,Args,A,B> as FnOnce<Args>>::Output>;
        fn $op(self, b:&RcFn<'a,Args,B>) -> Self::Output { RcFn::new(binary::$Op(RcFn::clone(self),RcFn::clone(&b))) }
    }
)+
mod uniform_binary {$(
    pub struct $Op<'a,Args,A,B>(pub A, pub super::RcFn<'a,Args,B>);
    impl<Args,A,B> FnOnce<Args> for $Op<'_,Args,A,B> where A:std::ops::$Op<B>+Copy {
        type Output = <A as std::ops::$Op<B>>::Output;
        extern "rust-call" fn call_once(self, args:Args) -> Self::Output { Self::call(&self, args) }
    }
    impl<Args,A,B> FnMut<Args> for $Op<'_,Args,A,B> where A:std::ops::$Op<B>+Copy { extern "rust-call" fn call_mut(&mut self, args:Args) -> Self::Output { Self::call(&self, args) } }
    impl<Args,A,B> Fn<Args> for $Op<'_,Args,A,B> where A:std::ops::$Op<B>+Copy { extern "rust-call" fn call(&self, args:Args) -> Self::Output { (self.0).$op(self.1.call(args)) } }
)+}
$(//$(
    impl<'a,Args:'static,B:'static> std::ops::$Op<RcFn<'a,Args,B>> for $Uniform where Self:std::ops::$Op<B> {
        type Output = RcFn<'a,Args,<Self as std::ops::$Op<B>>::Output>;
        fn $op(self, b:RcFn<'a,Args,B>) -> Self::Output { RcFn::new(uniform_binary::$Op(self,b)) }
    }
    impl<'a,Args:'static,B:'static> std::ops::$Op<&RcFn<'a,Args,B>> for $Uniform where Self:std::ops::$Op<B> {
        type Output = RcFn<'a,Args,<Self as std::ops::$Op<B>>::Output>;
        fn $op(self, b:&RcFn<'a,Args,B>) -> Self::Output { RcFn::new(uniform_binary::$Op(self,RcFn::clone(&b))) }
    }
)+//)+
)}

unary!([Neg neg]);
binary!([Add add, Sub sub, Mul mul][f32]);
