use super::Result;
pub use vector::{size, vec2};
pub use wayland_client::protocol::wl_pointer::{WlPointer as Pointer, ButtonState};
#[derive(Default,Clone,Copy)] pub struct ModifiersState { pub shift: bool, pub ctrl: bool, pub logo: bool, pub alt: bool }
//pub use piet_gpu::PietGpuRenderContext as RenderContext;
pub type RenderContext<'t> = image::Image<&'t mut [image::bgra8]>;

pub struct EventContext<'t> {
	pub modifiers_state: ModifiersState,
	pub pointer: Option<&'t Pointer>,
}

pub type MouseButtons = u32;

pub enum Event {
	Key { key: char },
	Button { position: vec2, button: u8, state: ButtonState },
	Motion { position: vec2, mouse_buttons: MouseButtons },
	Scroll (f32)
}

pub trait Widget {
    fn size(&mut self, size: size) -> size { size }
    fn paint(&mut self, context: &mut RenderContext, size: size) -> Result;
    fn event(&mut self, _size: size, _event_context: &EventContext, _event: &Event) -> Result<bool> { Ok(false) }
}
