use {std::assert, crate::vector::{size2,uint2}};

pub struct Image<Container> {
    pub stride : u32,
    pub size : size2,
    pub buffer : Container,
}

impl<C> Image<C> {
    pub fn index(&self, uint2{x,y}: uint2) -> usize { assert!( x < self.size.x && y < self.size.y ); (y * self.stride + x) as usize }
}

impl<Container> std::ops::Deref for Image<Container> {
    type Target = Container;
    fn deref(&self) -> &Self::Target { &self.buffer }
}

impl<Container> std::ops::DerefMut for Image<Container> {
    fn deref_mut(&mut self) -> &mut Self::Target { &mut self.buffer }
}

pub trait IntoImage {
    type Image;
    fn image(self, size : size2) -> Self::Image;
}
macro_rules! impl_into_image { ($S:ty) => {
impl<'t, T> IntoImage for $S {
    type Image = Image<$S>;
    fn image(self, size : size2) -> Self::Image {
        assert!(self.len() == (size.x*size.y) as usize);
        Self::Image{stride: size.x, size, buffer: self}
    }
}
}}
impl_into_image!(&'t [T]);
impl_into_image!(&'t mut [T]);

impl<T, C:std::ops::Deref<Target=[T]>> std::ops::Index<usize> for Image<C> {
    type Output=T;
    fn index(&self, i:usize) -> &Self::Output { &self.deref()[i] }
}
impl<T, C:std::ops::DerefMut<Target=[T]>> std::ops::IndexMut<usize> for Image<C> {
    fn index_mut(&mut self, i:usize) -> &mut Self::Output { &mut self.deref_mut()[i] }
}

impl<T, C:std::ops::Deref<Target=[T]>> std::ops::Index<uint2> for Image<C> {
    type Output=T;
    fn index(&self, i:uint2) -> &Self::Output { &self[self.index(i)] }
}
impl<T, C:std::ops::DerefMut<Target=[T]>> std::ops::IndexMut<uint2> for Image<C> {
    fn index_mut(&mut self, i:uint2) -> &mut Self::Output { let i = self.index(i); &mut self[i] }
}

trait Divide { type Iterator; fn divide(self, count : u32) -> Self::Iterator; }
impl Divide for std::ops::Range<u32> {
    type Iterator = DivideRangeIterator;
    fn divide(self, count : u32) -> Self::Iterator { Self::Iterator{range: self, count, counter: 0} }
}
struct DivideRangeIterator { range : std::ops::Range<u32>, count : u32, counter : u32}
impl Iterator for DivideRangeIterator {
    type Item = std::ops::Range<u32>;
    fn next(&mut self) -> Option<Self::Item> {
        if self.counter == self.count { return None; }
        let start = self.range.start + self.counter*(self.range.end-self.range.start)/self.count;
        self.counter += 1;
        let end = self.range.start + self.counter*(self.range.end-self.range.start)/self.count;
        Some(start..end)
    }
}

// Take ranges from slice. \note Only supports 'in-order' slicing to simplify implementation.
struct TakeSliceRange<'t, T> { slice: &'t [T], consumed : usize }
impl<'t, T> TakeSliceRange<'t, T> {
    fn new(slice: &'t [T]) -> Self { Self{slice, consumed: 0} }
    fn take_slice(&mut self, range : std::ops::Range<usize>) -> &[T] {
        let (consumed, remaining) = self.slice.split_at(range.end-self.consumed);
        let slice = &consumed[range.start-self.consumed..];
        self.slice = remaining;
        self.consumed = range.end;
        slice
    }
}

trait TakeSliceMut<T> { fn take_slice_mut(&mut self, n: usize) -> &mut [T]; }
impl<T> TakeSliceMut<T> for &mut [T] {
    fn take_slice_mut(&mut self, mid: usize) -> &mut [T] {
        let (consumed, remaining) = std::mem::replace(self, Default::default()).split_at_mut(mid); //Default::default() &mut []
        *self = remaining;
        consumed
    }
}

struct TakeSliceRangeMut<'t, T> { slice: &'t mut [T], consumed : usize }
impl<'t, T> TakeSliceRangeMut<'t, T> {
    fn new(slice: &'t mut [T]) -> Self { Self{slice, consumed: 0} }
    fn take_slice_mut(&mut self, range : std::ops::Range<usize>) -> &mut [T] {
        let slice = &mut self.slice.take_slice_mut(range.end-self.consumed)[range.start-self.consumed..];
        self.consumed = range.end;
        slice
    }
}

impl<T, C:std::ops::DerefMut<Target=[T]>> Image<C> {
    pub fn slice_mut(&mut self, offset : uint2, size : size2) -> Image<&mut[T]> {
        assert!(offset.x+size.x <= self.size.x && offset.y+size.y <= self.size.y, (self.size, offset, size));
        Image{size, stride: self.stride, buffer: &mut self.buffer[(offset.y*self.stride+offset.x) as usize..]}
    }
    #[cfg(all(feature="array",feature="thread"))] pub fn set<F:Fn(uint2)->T+Copy+Send>(&mut self, f:F) where T:Send {
        use crate::{core::array::{Iterator, IntoIterator}, vector::xy};
        const N : usize = 8;
        let (width, target_stride) = (self.size.x, self.stride);
        let mut target_buffer = TakeSliceRangeMut::new(&mut self.buffer);
        Iterator::collect::<[_;N]>((0..self.size.y).divide(N as u32).map(|std::ops::Range{start:y0,end:y1}| {
            let target = target_buffer.take_slice_mut((y0*target_stride) as usize..(y1*target_stride) as usize);
            unsafe { std::thread::Builder::new().spawn_unchecked(move || {
                let mut target = target.as_mut_ptr();
                for y in y0..y1 {
                    for x in 0..width {
                        *target.add(x as usize) = f(xy{x,y});
                    }
                    target = target.add(target_stride as usize);
                }
            }) }.unwrap()
        })).into_iter().for_each(|t| t.join().unwrap());
    }
    // fixme: factorize with set
    #[cfg(all(feature="array",feature="thread"))] pub fn map<U, S:std::ops::Deref<Target=[U]>+Send, F:Fn(uint2,U)->T+Copy+Send>(&mut self, source:Image<S>, f:F)
    where T:Send, U:Copy+Send+Sync {
        use crate::{core::array::{Iterator, IntoIterator}, vector::xy};
        const N : usize = 8;
        assert!(self.size == source.size);
        let (width, target_stride, source_stride) = (self.size.x, self.stride, source.stride);
        let mut target_buffer = TakeSliceRangeMut::new(&mut self.buffer);
        let mut source_buffer = TakeSliceRange::new(&source.buffer);
        Iterator::collect::<[_;N]>((0..self.size.y).divide(N as u32).map(|std::ops::Range{start:y0,end:y1}| {
            let target = target_buffer.take_slice_mut((y0*target_stride) as usize..(y1*target_stride) as usize);
            let source = source_buffer.take_slice((y0*source_stride) as usize..(y1*source_stride) as usize);
            unsafe { std::thread::Builder::new().spawn_unchecked(move || {
                let mut target = target.as_mut_ptr();
                let mut source = source.as_ptr();
                for y in y0..y1 {
                    for x in 0..width {
                        *target.add(x as usize) = f(xy{x,y}, *source.add(x as usize))
                    }
                    target = target.add(target_stride as usize);
                    source = source.add(source_stride as usize);
                }
            }) }.unwrap()
        })).into_iter().for_each(|t| t.join().unwrap());
    }
}

#[allow(non_camel_case_types)] #[derive(Clone, Copy)] pub struct bgra8 { pub b : u8, pub g : u8, pub r : u8, pub a: u8  }

impl<T> Image<Vec<T>> {
    pub fn new(size: size2, buffer: Vec<T>) -> Self { assert!(buffer.len() == (size.x*size.y) as usize); Self{stride:size.x, size, buffer} }
    pub fn from_iter<I:IntoIterator<Item=T>>(size : size2, iter : I) -> Self {
        let mut buffer = Vec::with_capacity((size.y*size.x) as usize);
        buffer.extend( iter.into_iter() );
        Image::new(size, buffer)
    }
    pub fn uninitialized(size: size2) -> Self {
        let len = (size.x * size.y) as usize;
        let mut buffer = Vec::with_capacity(len);
        unsafe{ buffer.set_len(len) };
        Self{stride:size.x, size, buffer}
    }
    pub fn as_ref(&self) -> Image<&[T]> { Image{stride:self.stride, size:self.size, buffer: self.buffer.as_ref()} }
    pub fn as_mut(&mut self) -> Image<&mut [T]> { Image{stride:self.stride, size:self.size, buffer: self.buffer.as_mut()} }
}

impl<T:Default+Clone> Image<Vec<T>> {
    pub fn zero(size: size2) -> Self { Self::new(size, vec![T::default(); (size.x*size.y) as usize]) }
}

#[cfg(feature="sRGB")] #[allow(non_snake_case)] pub mod sRGB {
crate::lazy_static! { sRGB_forward12 : [u8; 0x1000] = crate::core::array::map(|i| {
    let linear = i as f64 / 0xFFF as f64;
    (0xFF as f64 * if linear > 0.0031308 {1.055*linear.powf(1./2.4)-0.055} else {12.92*linear}).round() as u8
}); }
#[allow(non_snake_case)] pub fn sRGB(v : f32) -> u8 { sRGB_forward12[(0xFFF as f32*v) as usize] } // 4K (fixme: interpolation of a smaller table might be faster)
}
