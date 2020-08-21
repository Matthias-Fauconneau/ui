use {core::error::Result, image::{Image, bgra8}};
pub use xy::{size, uint2};

pub type Target<'t> = Image<&'t mut[bgra8]>;
/*#[allow(non_upper_case_globals)] pub static black: bgra8 = bgra8{b:0x00,g:0x00,r:0x00,a:0xFF};
#[allow(non_upper_case_globals)] pub static white: bgra8 = bgra8{b:0xFF,g:0xFF,r:0xFF,a:0xFF};
#[allow(non_upper_case_globals)] pub static bg : bgra8 = black;
#[allow(non_upper_case_globals)] pub static fg: bgra8 = white;*/

pub use client_toolkit::{seat::{keyboard::ModifiersState, pointer::ThemedPointer}, reexports::client::protocol::wl_pointer::ButtonState};
pub struct EventContext<'t> {
	pub modifiers_state: ModifiersState,
	pub pointer: Option<&'t mut ThemedPointer>,
}
type MouseButtons = u32;
pub enum Event {
	Key { key: char },
	Button { button: u8, state: ButtonState, position: uint2 },
	Motion { position: uint2, mouse_buttons: MouseButtons }
}

pub trait Widget {
    fn size(&mut self, size : size) -> size { size }
    /*#[throws]*/ fn paint(&mut self, target : &mut Target) -> Result;
    fn event(&mut self, _size: size, _event_context: EventContext, _event: &Event) -> bool { false }
}
