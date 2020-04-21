use crate::{vector::size2, image::{Image, bgra8}};

pub type Target<'t> = Image<&'t mut[bgra8]>;
pub trait Widget {
    fn size(&mut self, size : size2) -> size2 { size }
    fn render(&mut self, target : &mut Target);
}
