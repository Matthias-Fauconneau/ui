use crate::vector::size2;

#[derive(Debug)]
pub struct Image<Data> {
    pub stride : u32,
    pub size : size2,
    pub data : Data,
}

use crate::{assert, slice::{Take,TakeMut}, vector::uint2};

impl<D> Image<D> {
    pub fn index(&self, uint2{x,y}: uint2) -> usize { assert!( x < self.size.x && y < self.size.y); (y * self.stride + x) as usize }
}

impl<D> std::ops::Deref for Image<D> {
    type Target = D;
    fn deref(&self) -> &Self::Target { &self.data }
}

impl<D> std::ops::DerefMut for Image<D> {
    fn deref_mut(&mut self) -> &mut Self::Target { &mut self.data }
}

impl<T, D:std::ops::Deref<Target=[T]>> std::ops::Index<usize> for Image<D> {
    type Output=T;
    fn index(&self, i:usize) -> &Self::Output { &self.deref()[i] }
}
impl<T, D:std::ops::DerefMut<Target=[T]>> std::ops::IndexMut<usize> for Image<D> {
    fn index_mut(&mut self, i:usize) -> &mut Self::Output { &mut self.deref_mut()[i] }
}

impl<T, D:std::ops::Deref<Target=[T]>> std::ops::Index<uint2> for Image<D> {
    type Output=T;
    fn index(&self, i:uint2) -> &Self::Output { &self[self.index(i)] }
}
impl<T, D:std::ops::DerefMut<Target=[T]>> std::ops::IndexMut<uint2> for Image<D> {
    fn index_mut(&mut self, i:uint2) -> &mut Self::Output { let i = self.index(i); &mut self[i] }
}

impl<T, D:std::ops::Deref<Target=[T]>> Image<D> {
    pub fn slice_lines(&self, lines: std::ops::Range<u32>) -> Image<&[T]> {
        assert!(lines.end <= self.size.y, self.size, lines);
        Image{size: size2{x:self.size.x, y:lines.len() as u32}, stride: self.stride, data: &self.data[(lines.start*self.stride) as usize..]}
    }
    pub fn slice(&self, offset: uint2, size: size2) -> Image<&[T]> {
        assert!(offset.x+size.x <= self.size.x && offset.y+size.y <= self.size.y, self.size, offset, size);
        Image{size, stride: self.stride, data: &self.data[(offset.y*self.stride+offset.x) as usize..]}
    }
}

impl<T, D:std::ops::DerefMut<Target=[T]>> Image<D> {
    pub fn slice_lines_mut(&mut self, lines: std::ops::Range<u32>) -> Image<&mut [T]> {
        assert!(lines.end <= self.size.y, self.size, lines);
        Image{size: size2{x:self.size.x, y:lines.len() as u32}, stride: self.stride, data: &mut self.data[(lines.start*self.stride) as usize..]}
    }
    pub fn slice_mut(&mut self, offset : uint2, size : size2) -> Image<&mut[T]> {
        assert!(offset.x+size.x <= self.size.x && offset.y+size.y <= self.size.y, self.size, offset, size);
        Image{size, stride: self.stride, data: &mut self.data[(offset.y*self.stride+offset.x) as usize..]}
    }
}

impl<'t, T> Image<&'t [T]> {
    pub fn take<'s>(&'s mut self, mid: u32) -> Image<&'t [T]> {
        assert!(mid <= self.size.y);
        self.size.y -= mid;
        Image{size: size2{x:self.size.x, y:mid}, stride: self.stride, data: self.data.take((mid*self.stride) as usize)}
    }
}

impl<'t, T> Image<&'t mut [T]> {
    pub fn take_mut<'s>(&'s mut self, mid: u32) -> Image<&'t mut[T]> {
        assert!(mid <= self.size.y);
        self.size.y -= mid;
        Image{size: size2{x:self.size.x, y:mid}, stride: self.stride, data: self.data.take_mut((mid*self.stride) as usize)}
    }
}

impl<'t, T> Iterator for Image<&'t [T]> {
    type Item = &'t [T];
    fn next(&mut self) -> Option<Self::Item> {
        if self.size.y > 0 { Some(&self.take(1).data[..self.size.x as usize]) }
        else { None }
    }
}

impl<'t, T> Iterator for Image<&'t mut [T]> {
    type Item = &'t mut[T];
    fn next(&mut self) -> Option<Self::Item> {
        if self.size.y > 0 { Some(&mut self.take_mut(1).data[..self.size.x as usize]) }
        else { None }
    }
}

impl<'t, T> Image<&'t mut [T]> {
    pub fn lines_mut(&mut self, range: std::ops::Range<u32>) -> impl Iterator<Item=(u32,&mut [T])> { range.clone().zip(self.slice_lines_mut(range)) }
}

/*impl<'t, T> Image<&'t [T]> {
    fn new(data: &'t [T], size : size2) -> Self {
        assert!(data.len() == (size.x*size.y) as usize);
        Self{stride: size.x, size, data}
    }
}*/

impl<'t, T> Image<&'t mut [T]> {
    fn new(data: &'t mut [T], size : size2) -> Self {
        assert_eq!(data.len(), (size.x*size.y) as usize, "{:?}", size);
        Self{stride: size.x, size, data}
    }
}

pub fn segment(total_length: u32, segment_count: u32) -> impl Iterator<Item=std::ops::Range<u32>> {
    (0..segment_count)
    .map(move |i| (i+1)*total_length/segment_count)
    .scan(0, |start, end| { let next = (*start, end); *start = end; Some(next) })
    .map(|(start, end)| start..end)
}

//#[cfg(not(feature="thread"))] trait Execute : Iterator<Item:FnOnce()>+Sized { fn execute(self) { self.for_each(|task| task()) } }
/*#[cfg(feature="thread")] trait Execute : Iterator<Item:FnOnce()>+Sized { fn execute(self) {
    use crate::{core::array::{Iterator, IntoIterator}};
    Iterator::collect::<[_;N]>( iter.map(|task| unsafe { std::thread::Builder::new().spawn_unchecked(task) } ) ).into_iter().for_each(|t| t.join().unwrap())
}}*/
//impl<I:Iterator<Item:FnOnce()>> Execute for I {}

impl<T:Send> Image<&mut [T]> {
    pub fn set<F:Fn(uint2)->T+Copy+Send>(&mut self, f:F) {
        /*let mut target = self.take_mut(0);
        segment(target.size.y, 1/*8*/)
        .map(|segment| {
            let target = target.take_mut(segment.len() as u32);
            move || {
                for (y, target) in segment.zip(target) {
                    for (x, target) in target.iter_mut().enumerate() {
                        *target = f(uint2{x: x as u32,y});
                    }
                }
            }
        })
        .execute()*/
        for y in 0..self.size.y { for x in 0..self.size.x { self[uint2{x,y}] = f(uint2{x,y}); } }
    }
    pub fn set_map<U:Copy+Send+Sync, D:std::ops::Deref<Target=[U]>+Send, F:Fn(uint2,U)->T+Copy+Send>(mut self, source: Image<D>, f: F) {
        assert!(self.size == source.size);
        /*segment(self.size.y, 1/*8*/)
        .map(|segment| {
            let target = self.take_mut(segment.len() as u32);
            let source = source.slice(uint2{x:0, y:segment.start}, size2{x:source.size.x, y:segment.len() as u32});
            move || {
                for (y, (target, source)) in segment.zip(target.zip(source)) {
                    for (x, (target, source)) in target.iter_mut().zip(source).enumerate() {
                        *target = f(uint2{x:x as u32, y}, *source);
                    }
                }
            }
        })
        .execute()*/
        for y in 0..self.size.y { for x in 0..self.size.x { self[uint2{x,y}] = f(uint2{x,y}, source[uint2{x,y}]); } }
    }
}

impl<T> Image<Vec<T>> {
    pub fn new(size: size2, data: Vec<T>) -> Self {
		assert_eq!(data.len(), (size.x*size.y) as usize);
		Self{stride: size.x, size, data}
	}
    pub fn from_iter<I:IntoIterator<Item=T>>(size : size2, iter : I) -> Self {
        let mut buffer = Vec::with_capacity((size.y*size.x) as usize);
        buffer.extend( iter.into_iter() );
        Image::<Vec<T>>::new(size, buffer)
    }
    pub fn uninitialized(size: size2) -> Self {
        let len = (size.x * size.y) as usize;
        let mut buffer = Vec::with_capacity(len);
        unsafe{ buffer.set_len(len) };
        Image::<Vec<T>>::new(size, buffer)
    }
    pub fn as_ref(&self) -> Image<&[T]> { Image{stride:self.stride, size:self.size, data: self.data.as_ref()} }
    pub fn as_mut(&mut self) -> Image<&mut [T]> { Image{stride:self.stride, size:self.size, data: self.data.as_mut()} }
}

impl<T:Default+Clone> Image<Vec<T>> {
    pub fn zero(size: size2) -> Self { Self::new(size, vec![T::default(); (size.x*size.y) as usize]) }
}

vector!(3 bgr T T T, b g r);
#[allow(non_camel_case_types)] pub type bgrf = bgr<f32>;

#[allow(non_camel_case_types)] #[derive(Clone, Copy, Debug)] pub struct bgra8 { pub b : u8, pub g : u8, pub r : u8, pub a: u8  }
impl std::convert::From<u8> for bgra8 { fn from(v: u8) -> Self { bgra8{b:v,g:v,r:v,a:v} } }

impl<'t> Image<&'t mut [bgra8]> {
    pub fn from_bytes(slice: &'t mut [u8], size: size2) -> Self { Self::new(unsafe{crate::slice::cast_mut(slice)}, size) }
}

#[cfg(feature="sRGB")] #[allow(non_snake_case)] pub mod sRGB {
	#![allow(non_snake_case,non_upper_case_globals)]
    lazy_static::lazy_static!{ static ref sRGB_forward12 : [u8; 0x1000] = crate::array::map(|i| {
        let linear = i as f64 / 0xFFF as f64;
        (0xFF as f64 * if linear > 0.0031308 {1.055*linear.powf(1./2.4)-0.055} else {12.92*linear}).round() as u8
    }); }
    pub fn sRGB(v : f32) -> u8 { sRGB_forward12[(0xFFF as f32*v) as usize] } // 4K (fixme: interpolation of a smaller table might be faster)
    impl From<super::bgrf> for super::bgra8 { fn from(v: super::bgrf) -> Self { Self{b:sRGB(v.b), g:sRGB(v.g), r:sRGB(v.r), a:0xFF} } }
}
