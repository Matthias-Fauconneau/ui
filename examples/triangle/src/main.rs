#![feature(slice_from_ptr_range)] // shader
use ui::{Result, size, int2, Widget, shader, vulkan::{Context, Commands, Arc, ImageView}};
shader!{triangle}

struct App;

impl Widget for App { 
fn paint(&mut self, context/*@Context{device, memory_allocator, ..}*/: &Context, commands: &mut Commands, target: Arc<ImageView>, _: size, _: int2) -> Result<()> {
	let triangle = triangle::Pass::new(context, false, ui::vulkan::PrimitiveTopology::TriangleList)?;
	triangle.begin_rendering(context, commands, target.clone(), None, true, &triangle::Uniforms::empty(), &[])?;
	unsafe{commands.draw(3, 1, 0, 0)}?;
	commands.end_rendering()?;
	Ok(())
}
}

fn main() -> Result { ui::run("triangle", Box::new(|_,_| Ok(Box::new(App)))) }
