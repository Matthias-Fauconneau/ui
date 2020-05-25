use std::ops::Try;
pub trait TryExtend<R:Try> { fn try_extend<I:Iterator<Item=R>>(&mut self, iter: I) -> Result<(),R::Error>; }
impl<T, E> TryExtend<Result<T,E>> for Vec<T> {
    fn try_extend<I:IntoIterator<Item=Result<T,E>>>(&mut self, iter: I) -> Result<(),E> {
        let mut iter = iter.into_iter();
        self.reserve(iter.size_hint().1.unwrap());
        iter.try_for_each(move |element| { self.push(element?); Ok(()) } ).into_result() //32155
    }
}
