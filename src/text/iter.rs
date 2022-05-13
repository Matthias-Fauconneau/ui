pub struct PeekingTakeWhile<'t, I:Iterator, P> { iter: &'t mut std::iter::Peekable<I>, predicate: P }
impl<'t, I:Iterator, P: Fn(&<I as Iterator>::Item) -> bool> PeekingTakeWhile<'t, I, P> {
	fn peek(&mut self) -> Option<&I::Item> {
        let Self{iter, predicate} = self;
        iter.peek().filter(|x| predicate(*x))
    }
}
impl<'t, I:Iterator, P: Fn(&<I as Iterator>::Item) -> bool> Iterator for PeekingTakeWhile<'t, I, P> {
    type Item = <I as Iterator>::Item;
    fn next(&mut self) -> Option<Self::Item> {
        self.peek()?;
        self.iter.next()
    }
}

pub trait PeekableExt<'t, I:Iterator> : Iterator {
    #[must_use] fn peeking_take_while<P:Fn(&<Self as Iterator>::Item) -> bool>(&'t mut self, predicate: P) -> PeekingTakeWhile<'t, I, P>;
}
impl<'t, I:Iterator> PeekableExt<'t, I> for std::iter::Peekable<I> {
    fn peeking_take_while<P:Fn(&<Self as Iterator>::Item) -> bool>(&'t mut self, predicate: P) -> PeekingTakeWhile<I, P> { PeekingTakeWhile{iter: self, predicate} }
}

pub trait Single: Iterator+Sized { fn single(mut self) -> Option<Self::Item> { self.next().filter(|_| self.next().is_none()) } }
impl<I:Iterator> Single for I {}

pub trait NthOrLast : Iterator {
	fn nth_or_last(&mut self, n: usize) -> Result<Self::Item, Option<Self::Item>> {
		let mut last = None;
		for _ in 0..n { last = Some(self.next().ok_or(last)?); }
		Err(last)
	}
}
impl<I:Iterator> NthOrLast for I {}
