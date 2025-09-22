#![feature(slice_from_ptr_range)] // shader
use ui::{Result, run, new_trigger, Widget, Context, Commands, Arc, ImageView, size, int2, fit, text, xy};
struct App;
impl Widget for App { 
fn paint(&mut self, context: &Context, commands: &mut Commands, target: Arc<ImageView>, size: size, _: int2) -> Result {
	let mut text = text("Text");
	let text_size = fit(size, text.size());
	text.paint_fit(context, commands, target, size, xy{x: 0, y: (size.y as i32-text_size.y as i32)/2});
	Ok(())
}
}

fn main() -> Result { run(new_trigger().unwrap(), "text", Box::new(|_,_| Ok(Box::new(App)))) }
