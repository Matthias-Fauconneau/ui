use crate::{error::Result, vector::size, image::{Image, bgra8}};

pub type Target<'t> = Image<&'t mut[bgra8]>;
#[allow(non_upper_case_globals)] pub static black: bgra8 = bgra8{b:0x00,g:0x00,r:0x00,a:0xFF};
#[allow(non_upper_case_globals)] pub static white: bgra8 = bgra8{b:0xFF,g:0xFF,r:0xFF,a:0xFF};
#[allow(non_upper_case_globals)] pub static bg : bgra8 = black;
#[allow(non_upper_case_globals)] pub static fg: bgra8 = white;

pub trait Widget {
    fn size(&mut self, size : size) -> size { size }
    /*#[throws]*/ fn paint(&mut self, target : &mut Target) -> Result;
}
