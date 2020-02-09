use {std::assert, crate::{core::{array::{map,IntoIter}}, vector::{xy,size2,uint2}}};

pub struct Image<Container> {
    pub stride : u32,
    pub size : size2,
    pub buffer : Container,
}

pub trait IntoImage {
    type Image;
    fn image(self, size : size2) -> Self::Image;
}
macro_rules! impl_into_image { ($T:ty) => {
impl<'t, T> IntoImage for $T {
    type Image = Image<$T>;
    fn image(self, size : size2) -> Self::Image {
        assert!(self.len() == (size.x*size.y) as usize);
        Self::Image{stride: size.x, size, buffer: self}
    }
}
}}
impl_into_image!(&'t [T]);
impl_into_image!(&'t mut [T]);

const N : usize = 1;
impl<T, C:std::ops::DerefMut<Target=[T]>> Image<C> {
    pub fn slice_mut(&mut self, offset : uint2, size : size2) -> Image<&mut[T]> {
        assert!(offset.x+size.x <= self.size.x && offset.y+size.y <= self.size.y, (self.size, offset, size));
        Image{size, stride: self.stride, buffer: &mut self.buffer[(offset.y*self.stride+offset.x) as usize..]}
    }
    #[cfg(feature="thread")] pub fn set<F:Fn(uint2)->T+Copy+Send>(&mut self, f:F) where T:Send {
        const N : usize = self::N;
        let ptr = self.buffer.as_mut_ptr();
        IntoIter::new(map::<_,_,N>(|i| {
        //for i in 0..N {
            let (y0,y1) = ((i as u32)*self.size.y/(N as u32), ((i as u32)+1)*self.size.y/(N as u32));
            let (i0,i1) = ((y0*self.stride) as usize, (y1*self.stride) as usize);
            let mut target_row = &mut unsafe{std::slice::from_raw_parts_mut(ptr, self.buffer.len())}[i0..(i1+self.size.x as usize-self.stride as usize)];
            //let mut target_row = &mut self.buffer[i0..i1];
            let (width, stride) = (self.size.x, self.stride);
            unsafe{std::thread::Builder::new().spawn_unchecked(move || {
                for y in y0..y1 {
                    for x in 0..width {
                        target_row[x as usize] = f(xy{x,y});
                    }
                    use std::slice::SliceIndex;
                    target_row = (stride as usize..).get_unchecked_mut(target_row);
                }
            })}.unwrap()
        })).for_each(|t| t.join().unwrap());
    }
    /*// fixme: factorize with set
    #[cfg(feature="thread")] pub fn map<U, S:std::ops::Deref<Target=[U]>+Send, F:Fn(uint2,U)->T+Copy+Send>(&mut self, source:Image<S>, f:F)
    where T:Send, U:Copy+Send+Sync {
        const N : usize = self::N;
        assert!(self.size == source.size);
        let target_buffer = self.buffer.as_mut_ptr();
        let source_buffer = source.buffer.as_ptr();
        IntoIter::new(map::<_,_,N>(|i| {
            let (y0,y1) = ((i as u32)*self.size.y/(N as u32), ((i as u32)+1)*self.size.y/(N as u32));
            let mut target_row = {
                let (i0,i1) = ((y0*self.stride) as usize, (y1*self.stride) as usize);
                unsafe { std::slice::from_raw_parts_mut(target_buffer.add(i1), i1-i0) }
            };
            let mut source_row = {
                let (i0,i1) = ((y0*source.stride) as usize, (y1*source.stride) as usize);
                unsafe { std::slice::from_raw_parts(source_buffer.add(i1), i1-i0) }
            };
            let (width, target_stride, source_stride) = (self.size.x, self.stride, source.stride);
            unsafe { std::thread::Builder::new().spawn_unchecked(move || {
                for y in y0..y1 {
                    for x in 0..width {
                        target_row[x as usize] = f(xy{x,y}, source_row[x as usize])
                    }
                    target_row = &mut target_row[target_stride as usize..];
                    source_row = &source_row[source_stride as usize..];
                }
            })}.unwrap()
        })).for_each(|t| t.join().unwrap());
    }*/
}

#[allow(non_camel_case_types)] #[derive(Clone, Copy)] pub struct bgra8 { pub b : u8, pub g : u8, pub r : u8, pub a: u8  }

impl<T> Image<Vec<T>> {
    pub fn new(size: size2, buffer: Vec<T>) -> Self { Self{stride:size.x, size, buffer} }
    //#[allow(dead_code)] pub fn zero(size: size2) -> Self { Self::new(size, vec![T::default(); (size.x*size.y) as usize]) }
    #[allow(dead_code)] pub fn uninitialized(size: size2) -> Self {
        let len = (size.x * size.y) as usize;
        let mut buffer = Vec::with_capacity(len);
        unsafe{ buffer.set_len(len) };
        Self{stride:size.x, size, buffer}
    }
    pub fn as_ref(&self) -> Image<&[T]> { Image{stride:self.stride, size:self.size, buffer: self.buffer.as_ref()} }
    pub fn as_mut(&mut self) -> Image<&mut [T]> { Image{stride:self.stride, size:self.size, buffer: self.buffer.as_mut()} }
}

#[cfg(feature="sRGB")] #[allow(non_snake_case)] pub mod sRGB {
macro_rules! lazy_static { ($name:ident : $T:ty = $e:expr;) => {
    #[allow(non_camel_case_types)] struct $name {}
    #[allow(non_upper_case_globals)] static $name : $name = $name{};
    impl std::ops::Deref for $name {
        type Target = $T;
        fn deref(&self) -> &Self::Target {
            #[allow(non_upper_case_globals)] static mut array : std::mem::MaybeUninit::<$T> = std::mem::MaybeUninit::<$T>::uninit();
            static INIT: std::sync::Once = std::sync::Once::new();
            unsafe{
                INIT.call_once(|| { array.write($e); });
                &array.get_ref()
            }
        }
    }
}}

lazy_static! { sRGB_forward12 : [u8; 0x1000] = crate::core::array::map(|i| {
    let linear = i as f64 / 0xFFF as f64;
    (0xFF as f64 * if linear > 0.0031308 {1.055*linear.powf(1./2.4)-0.055} else {12.92*linear}).round() as u8
}); }
#[allow(non_snake_case)] pub fn sRGB(v : f32) -> u8 { sRGB_forward12[(0xFFF as f32*v) as usize] } // 4K (fixme: interpolation of a smaller table might be faster)
}
