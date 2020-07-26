use {core::error::Result, image::{Image, bgra8}};
pub use xy::size;

pub type Target<'t> = Image<&'t mut[bgra8]>;
/*#[allow(non_upper_case_globals)] pub static black: bgra8 = bgra8{b:0x00,g:0x00,r:0x00,a:0xFF};
#[allow(non_upper_case_globals)] pub static white: bgra8 = bgra8{b:0xFF,g:0xFF,r:0xFF,a:0xFF};
#[allow(non_upper_case_globals)] pub static bg : bgra8 = black;
#[allow(non_upper_case_globals)] pub static fg: bgra8 = white;*/

#[derive(PartialEq,Clone,Copy,num_enum::TryFromPrimitive)] #[repr(u8)] pub enum Key { Escape = 1, Right = 0x6A }
pub type Event = Key;

pub trait Widget {
    fn size(&mut self, size : size) -> size { size }
    /*#[throws]*/ fn paint(&mut self, target : &mut Target) -> Result;
    fn event(&mut self, _event: &Event) -> bool { false }
}
