#![feature(slice_from_ptr_range)] // shader
use ui::{Result, run, new_trigger, size, int2, image::{Image, xy, rgba8}, Widget, vulkan, shader};
use vulkan::{Context, Commands, Arc, ImageView, PrimitiveTopology, image, WriteDescriptorSet, linear};
shader!{view}

struct App;

impl Widget for App { 
fn paint(&mut self, context/*@Context{device, memory_allocator, ..}*/: &Context, commands: &mut Commands, target: Arc<ImageView>, _: size, _: int2) -> Result {
	let mut pass = view::Pass::new(context, false, PrimitiveTopology::TriangleList)?;
	let image = image(context, commands, Image::from_xy(xy{x: 16, y: 16}, |xy{x,y}| rgba8{r: if x%2==0 { 0 } else { 0xFF }, g: if y%2==0 { 0 } else { 0xFF }, b: 0xFF, a: 0xFF}).as_ref())?;
	pass.begin_rendering(context, commands, target.clone(), None, true, &view::Uniforms::empty(), &[
		WriteDescriptorSet::image_view(0, ImageView::new_default(&image)?),
        WriteDescriptorSet::sampler(1, linear(context)),
	])?;
	unsafe{commands.draw(3, 1, 0, 0)}?;
	commands.end_rendering()?;
	Ok(())
}
}

fn main() -> Result { run(new_trigger().unwrap(), "view", Box::new(|_,_| Ok(Box::new(App)))) }
