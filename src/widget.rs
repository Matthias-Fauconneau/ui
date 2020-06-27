use crate::{vector::size2, image::{Image, bgra8}};

pub type Target<'t> = Image<&'t mut[bgra8]>;
pub static BLACK: bgra8 = bgra8{b:0x00,g:0x00,r:0x00,a:0xFF};
pub static WHITE: bgra8 = bgra8{b:0xFF,g:0xFF,r:0xFF,a:0xFF};

pub trait Widget {
    fn size(&mut self, size : size2) -> size2 { size }
    fn paint(&mut self, target : &mut Target);
}
