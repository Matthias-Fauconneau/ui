use {error::Result, image::{Image, bgra8}};
pub use xy::{size, uint2};

pub type Target<'t> = Image<&'t mut[bgra8]>;

pub use client_toolkit::{seat::{keyboard::ModifiersState, pointer::ThemedPointer}, reexports::client::protocol::wl_pointer::ButtonState};

pub struct EventContext<'t> {
	pub modifiers_state: ModifiersState,
	pub pointer: Option<&'t mut ThemedPointer>,
}

pub type MouseButtons = u32;

pub enum Event {
	Key { key: char },
	Button { button: u8, state: ButtonState, position: uint2 },
	Motion { position: uint2, mouse_buttons: MouseButtons },
	Scroll (f32)
}

pub trait Widget {
    fn size(&mut self, size: size) -> size { size }
    fn paint(&mut self, target: &mut Target) -> Result<()>;
    fn event(&mut self, _size: size, _event_context: &EventContext, _event: &Event) -> Result<bool> { Ok(false) }
}
