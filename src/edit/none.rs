pub trait None { fn none() -> Self; }
pub fn none<T:None>() -> T { None::none() }
pub trait IsNone {
	fn is_none(&self) -> bool;
	fn is_some(&self) -> bool { !self.is_none() } // final
	fn to_option(self) -> Option<Self> where Self:Sized { if self.is_some() { Some(self) } else { None } } // final
}
impl<T:None+PartialEq> IsNone for T { fn is_none(&self) -> bool { self == &None::none() } }
impl<T:Default> None for T { fn none() -> Self { T::default() } }
