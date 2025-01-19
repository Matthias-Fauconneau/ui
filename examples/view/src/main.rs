#![feature(slice_from_ptr_range)] // shader
#![feature(iterator_try_collect)]
use {ui::{Result, time, size, int2, Widget, EventContext, Event::{self, Key}, vulkan, shader}, ::image::{Image, rgb, rgb8, rgba8, f32, sRGB8_OETF12, oetf8_12}};
use vulkan::{Context, Commands, Arc, ImageView, Image as GPUImage, image, WriteDescriptorSet, linear};
shader!{view}

struct App {
	pass: view::Pass,
	images: Vec<Arc<GPUImage>>,
	index: usize,
}
impl App {
	fn new(context: &Context, commands: &mut Commands) -> Result<Self> { Ok(Self{
		pass: view::Pass::new(context, false)?,
		images: std::env::args().skip(1).map(|ref path| image(context, commands, if let Ok(Image{size, data, ..}) = f32(path) {
				fn minmax(values: &[f32]) -> [f32; 2] {
					let [mut min, mut max] = [f32::INFINITY, -f32::INFINITY];
					for &value in values { if value > f32::MIN && value < min { min = value; } if value > max { max = value; } }
					[min, max]
				}
				let [min, max] = minmax(&data);
				let oetf = &sRGB8_OETF12;
				Image::new(size, data.into_iter().map(|v| rgba8::from(rgb::from(oetf8_12(oetf, ((v-min)/(max-min)).clamp(0., 1.))))).collect())
			} else { time!(rgb8(path)).map(|v| rgba8::from(v)) }.as_ref())
		).try_collect()?,
		index: 0,
	})}
}
impl Widget for App { 
fn paint(&mut self, context: &Context, commands: &mut Commands, target: Arc<ImageView>, _: size, _: int2) -> Result<()> {
	let Self{pass, images, index} = self;
	pass.begin_rendering(context, commands, target.clone(), None, true, &view::Uniforms::empty(), &[
		WriteDescriptorSet::image_view(0, ImageView::new_default(images[*index].clone())?),
		WriteDescriptorSet::sampler(1, linear(context)),
	])?;
	unsafe{commands.draw(3, 1, 0, 0)}?;
	commands.end_rendering()?;
	Ok(())
}
fn event(&mut self, _size: size, _: &mut EventContext, event: &Event) -> Result<bool> {
	Ok(match event {
		Key('←') => { self.index = (self.index+self.images.len()-1)%self.images.len(); true },
		Key('→') => { self.index = (self.index+1)%self.images.len(); true },
		_ => false,
	})
}
}

fn main() -> Result { ui::run("view", Box::new(|context, commands| Ok(Box::new(App::new(context, commands)?)))) }
