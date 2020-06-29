pub trait FromIterator<T> {
    fn from_iter<I:std::iter::IntoIterator<Item=T>>(into_iter: I) -> Self;
}
impl<T, const N : usize> FromIterator<T> for [T; N] {
    #[track_caller] fn from_iter<I>(into_iter:I) -> Self where I:std::iter::IntoIterator<Item=T> {
        let mut array : [std::mem::MaybeUninit<T>; N] = std::mem::MaybeUninit::uninit_array();
        let mut iter = into_iter.into_iter();
        for e in array.iter_mut() { e.write(iter.next().unwrap()); } // panic on short iter
        //unsafe { std::mem::transmute::<_, [T; N]>(array) } // cannot transmute between generic types
        let array_as_initialized = unsafe { std::ptr::read(&array as *const _ as *const [T; N]) };
        std::mem::forget(array);
        array_as_initialized // Self(array_as_initialized)
    }
}
pub trait Iterator : std::iter::Iterator {
    #[track_caller] fn collect<B: FromIterator<Self::Item>>(self) -> B where Self:Sized { FromIterator::from_iter(self) }
}
impl<I:std::iter::Iterator> Iterator for I {}
pub fn map<T, F:Fn(usize)->T, const N:usize>(f : F) -> [T; N] { Iterator::collect((0..N).map(f)) }
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
    fn as_mut_slice(&mut self) -> &mut [T] {
        //unsafe { std::mem::transmute::<&mut [std::mem::MaybeUninit<T>], &mut [T]>(&mut self.data[self.alive.clone()]) }
        unsafe { &mut *(&mut self.data[self.alive.clone()] as *mut [std::mem::MaybeUninit<T>] as *mut [T]) }
    }
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
impl<T: crate::num::Zero, const N:usize> crate::num::Zero for [T; N] { fn zero() -> Self { map(|_| crate::num::Zero::zero()) } }
