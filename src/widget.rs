use super::Result;
pub use vector::{size, int2};
pub use {std::sync::Arc, crate::vulkan::{Context, Commands, ImageView}};

#[derive(Default,Clone,Copy)] pub struct ModifiersState { pub shift: bool, pub ctrl: bool, pub logo: bool, pub alt: bool }

pub struct EventContext {
	pub modifiers_state: ModifiersState,
}

pub type MouseButtons = u32;

pub enum Event {
	Key (char),
	Button { position: int2, button: u8, state: u8 },
	Motion { position: int2, mouse_buttons: MouseButtons },
	Scroll (i32),
	Stale,
	Idle,
	Trigger,
}

pub trait Widget {
    fn size(&mut self, size: size) -> size { size }
    fn paint(&mut self, context: &Context, commands: &mut Commands, target: Arc<ImageView>, size: size, offset: int2) -> Result;
    fn event(&mut self, size: size, context: &mut EventContext, event: &Event) -> Result<bool> { (size, context, event); Ok(false) }
}
