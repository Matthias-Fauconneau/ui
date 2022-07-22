use super::Result;
pub use vector::{size, int2, vec2, xy};
//pub use {wayland_client::protocol::wl_pointer::ButtonState, crate::app::input::Cursor};
#[derive(Default,Clone,Copy)] pub struct ModifiersState { pub shift: bool, pub ctrl: bool, pub logo: bool, pub alt: bool }
//pub use piet_gpu::PietGpuRenderContext as RenderContext;
pub type RenderContext<'t> = image::Image<&'t mut [image::bgra8]>;

pub struct EventContext {//<'t> {
	pub modifiers_state: ModifiersState,
	//pub cursor: Option<Cursor<'t>>,
}

pub type MouseButtons = u32;

pub enum Event {
	Key (char),
	Button { position: vec2, button: u8 },//, state: ButtonState },
	Motion { position: vec2, mouse_buttons: MouseButtons },
	Scroll (f32)
}

pub trait Widget {
    fn size(&mut self, size: size) -> size { size }
    fn paint(&mut self, context: &mut RenderContext, size: size, offset: int2) -> Result;
    fn event(&mut self, _size: size, _event_context: &mut EventContext, _event: &Event) -> Result<bool> { Ok(false) }
}
