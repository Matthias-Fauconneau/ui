use super::Result;
pub use vector::{size, int2, vec2, xy};
pub type Target<'t> = image::Image<&'t mut [u32]>;
#[derive(Default,Clone,Copy)] pub struct ModifiersState { pub shift: bool, pub ctrl: bool, pub logo: bool, pub alt: bool }

pub struct EventContext<'t, 's, 'c> {
	/*#[cfg(feature="wayland")] pub toplevel: &'t crate::app::wayland::toplevel::Toplevel<'s>,
	#[cfg(not(feature="wayland"))]*/ pub toplevel: &'t core::marker::PhantomData<&'s ()>,
	pub modifiers_state: ModifiersState,
	/*#[cfg(feature="wayland")] pub cursor: &'t mut crate::app::Cursor<'c>,
	#[cfg(not(feature="wayland"))]*/ pub cursor: &'t core::marker::PhantomData<&'c ()>,
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
    fn paint(&mut self, target: &mut Target, size: size, offset: int2) -> Result;
    fn event(&mut self, size: size, context: &mut EventContext, event: &Event) -> Result<bool> { (size, context, event); Ok(false) }
}
