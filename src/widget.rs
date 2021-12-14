use super::Result;
pub use xy::{size, uint2};

/*use piet::{RenderContext, Text, TextAttribute, TextLayoutBuilder, kurbo::Point};
let layout = context.text().new_text_layout("Hello World!").default_attribute(TextAttribute::FontSize(64.0)).build()?;
context.draw_text(&layout, Point::new(0., 64.));*/

pub use piet_gpu::PietGpuRenderContext as RenderContext;
/*use image::{Image, bgra8}
type RenderContext<'t> = Image<&'t mut[bgra8]>
impl<'t> piet::RenderContext for RenderContext<'t>;*/

pub use client_toolkit::{seat::{keyboard::ModifiersState, pointer::ThemedPointer}, reexports::client::protocol::wl_pointer::ButtonState};

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
    fn paint(&mut self, context: &mut RenderContext) -> Result;
    fn event(&mut self, _size: size, _event_context: &EventContext, _event: &Event) -> Result<bool> { Ok(false) }
}
