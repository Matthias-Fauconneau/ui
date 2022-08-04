use super::Result;
pub use vector::{size, int2, vec2, xy};
pub type Target<'t> = image::Image<&'t mut [image::bgra8]>;
#[derive(Default,Clone,Copy)] pub struct ModifiersState { pub shift: bool, pub ctrl: bool, pub logo: bool, pub alt: bool }

pub struct EventContext<'t> {
	pub modifiers_state: ModifiersState,
	pub server: &'t mut crate::app::Server,
	pub cursor: &'t mut crate::app::Cursor,
}

pub type MouseButtons = u32;

pub enum Event {
	Key (char),
	Button { position: int2, button: u8, state: u8 },
	Motion { position: int2, mouse_buttons: MouseButtons },
	Scroll (i32)
}

pub trait Widget {
    fn size(&mut self, size: size) -> size { size }
    fn paint(&mut self, context: &mut Target, size: size, offset: int2) -> Result;
    fn event(&mut self, _size: size, _event_context: &mut EventContext, _event: &Event) -> Result<bool> { Ok(false) }
}
