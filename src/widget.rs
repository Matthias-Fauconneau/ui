use super::Result;
pub use xy::size;
pub use client_toolkit::{seat::{keyboard::ModifiersState, pointer::ThemedPointer}, reexports::client::protocol::wl_pointer::ButtonState};
pub use piet_gpu::PietGpuRenderContext as RenderContext;
use xy::uint2;

pub struct EventContext<'t> {
	pub modifiers_state: ModifiersState,
	pub pointer: Option<&'t mut ThemedPointer>,
}

pub type MouseButtons = u32;

pub enum Event {
	Key { key: char },
	Button { position: uint2, button: u8, state: ButtonState },
	Motion { position: uint2, mouse_buttons: MouseButtons },
	Scroll (f32)
}

pub trait Widget {
    fn size(&mut self, size: size) -> size { size }
    fn paint(&mut self, context: &mut RenderContext, size: size) -> Result;
    fn event(&mut self, _size: size, _event_context: &EventContext, _event: &Event) -> Result<bool> { Ok(false) }
}
