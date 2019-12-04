#[allow(non_camel_case_types)] #[derive(Clone, Copy, Debug)] pub struct uint2 { pub x: u32, pub y : u32 }
impl From<(u32,u32)> for uint2 { fn from(o : (u32, u32)) -> Self { Self{x:o.0,y:o.1} } }
#[allow(non_camel_case_types)] pub type size2 = uint2;
#[allow(non_camel_case_types)] pub type offset2 = uint2;

pub struct Image<T> {
    pub stride : u32,
    pub size : size2,
    data : T,
}
pub trait IntoImage {
    type Image;
    fn image(self, size : size2) -> Option<Self::Image>;
}
impl<'t, T> IntoImage for &'t [T] {
    type Image = Image<&'t [T]>;
    fn image(self, size : size2) -> Option<Self::Image> {
        if self.len() == (size.x*size.y) as usize { Some(Self::Image{stride: size.x, size, data: self}) } else { None }
    }
}
impl<'t, T> IntoImage for &'t mut [T] {
    type Image = Image<&'t mut [T]>;
    fn image(self, size : size2) -> Option<Self::Image> {
        if self.len() == (size.x*size.y) as usize { Some(Self::Image{stride: size.x, size, data: self}) } else { None }
    }
}

impl<T> Image<&mut [T]> {
    pub fn slice_mut(&mut self, offset : offset2, size : size2) -> Option<Image<&mut[T]>> {
        if offset.x+size.x <= self.size.x && offset.y+size.y <= self.size.y { Some(Image{size, stride: self.stride, data: &mut self.data[(offset.y*self.stride+offset.x) as usize..] }) } else { None }
    }
}

pub trait Offset { fn offset(&self, offset : isize) -> Self; }
impl<T> Offset for *const T { fn offset(&self, offset : isize) -> Self { unsafe{(*self).offset(offset)} } }
impl<T> Offset for *mut T { fn offset(&self, offset : isize) -> Self { unsafe{(*self).offset(offset)} } }

pub struct Lines<T> { ptr: T, stride: isize }
impl<T:Offset> Lines<T> { fn next(&mut self) { self.ptr = self.ptr.offset(self.stride); } }

pub trait IntoLines {
    type Ptr : Offset;
    fn end(&self) -> Self::Ptr;
    fn lines(&mut self) -> Lines<Self::Ptr>;
    type Element : Sized;
    fn index(ptr : &Self::Ptr, x : usize) -> Self::Element;
}
impl<'t, T> IntoLines for Image<&'t [T]> {
    type Ptr = *const T;
    fn end(&self) -> Self::Ptr { unsafe{(self.data as *const [T] as Self::Ptr).offset((self.size.y*self.stride) as isize)} }
    fn lines(&mut self) -> Lines<Self::Ptr> { Lines::<Self::Ptr>{ptr: self.data as *const [T] as Self::Ptr, stride: self.stride as isize} }
    type Element = &'t T;
    fn index(ptr : &Self::Ptr, x : usize) -> Self::Element { unsafe{&*ptr.offset(x as isize)} }
}
impl<'t, T> IntoLines for Image<&'t mut [T]> {
    type Ptr = *mut T;
    fn end(&self) -> Self::Ptr { unsafe{(self.data as *const [T] as Self::Ptr).offset((self.size.y*self.stride) as isize)} }
    fn lines(&mut self) -> Lines<Self::Ptr> { Lines::<Self::Ptr>{ptr: self.data as *mut [T] as Self::Ptr, stride: self.stride as isize} }
    type Element = &'t mut T;
    fn index(ptr : &Self::Ptr, x : usize) -> Self::Element { unsafe{&mut *ptr.offset(x as isize)} }
}

pub struct PixelIterator1<T> where Image<T> : IntoLines {
    width : usize,
    end: <Image<T> as IntoLines>::Ptr,
    lines: Lines<<Image<T> as IntoLines>::Ptr>,
    x : usize,
}
impl<T> Iterator for PixelIterator1<T> where Image<T> : IntoLines, <Image<T> as IntoLines>::Ptr : Eq {
    type Item = <Image<T> as IntoLines>::Element;
    #[inline] // test mov add add test jcc (SIB) inc ~ 7
    fn next(&mut self) -> Option<Self::Item> {
        if self.x == self.width {
            self.x = 0;
            self.lines.next();
            if self.lines.ptr == self.end { None? }
        }
        let item = Some(Image::<T>::index(&self.lines.ptr, self.x));
        self.x += 1;
        item
    }
}

pub struct PixelIterator2<T0, T1> where Image<T0> : IntoLines, Image<T1> : IntoLines {
    width : usize,
    end: <Image<T0> as IntoLines>::Ptr,
    lines: (Lines<<Image<T0> as IntoLines>::Ptr>, Lines<<Image<T1> as IntoLines>::Ptr>), // (Lines<T::Ptr>...)
    x : usize,
}
impl<T0, T1> Iterator for PixelIterator2<T0, T1> where Image<T0> : IntoLines, Image<T1> : IntoLines, <Image<T0> as IntoLines>::Ptr : Eq {
    type Item = (<Image<T0> as IntoLines>::Element, <Image<T1> as IntoLines>::Element);
    #[inline] // test mov add add test jcc (SIB) inc ~ 7
    fn next(&mut self) -> Option<Self::Item> {
        if self.x == self.width {
            self.x = 0;
            self.lines.0.next(); self.lines.1.next(); // next(self.lines)...
            if self.lines.0.ptr == self.end { None? }
        }
        let item = Some((Image::<T0>::index(&self.lines.0.ptr, self.x), Image::<T1>::index(&self.lines.1.ptr, self.x))); // self.x.index(self.lines)...
        self.x += 1;
        item
    }
}

pub trait IntoPixelIterator { type PixelIterator; fn pixels(&mut self) -> Self::PixelIterator; }
impl<T> IntoPixelIterator for Image<T> where Image<T> : IntoLines {
    type PixelIterator = PixelIterator1<T>;
    fn pixels(&mut self) -> Self::PixelIterator {
        Self::PixelIterator{
            width : self.size.x as usize,
            end: self.end(),
            lines: self.lines(),
            x: 0
        }
    }
}
impl<T0, T1> IntoPixelIterator for (Image<T0>, Image<T1>) where Image<T0> : IntoLines, Image<T1> : IntoLines {
    type PixelIterator = PixelIterator2<T0, T1>;
    fn pixels(&mut self) -> Self::PixelIterator {
        Self::PixelIterator{
            width : self.0.size.x as usize,
            end: self.0.end(),
            lines: (self.0.lines(), self.1.lines()), // (self.lines()...)
            x: 0
        }
    }
}

#[allow(non_camel_case_types, dead_code)] #[derive(Clone, Copy)] pub struct argb8 { pub a: u8, pub r : u8, pub g : u8, pub b : u8 }
