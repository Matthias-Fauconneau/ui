#![feature(slice_from_ptr_range)] // shader
#![feature(iterator_try_collect)]
use {ui::{Result, size, int2, Widget, EventContext, Event::{self, Key}, vulkan, shader}, ::image::{Image, f32, sRGB8_OETF12, oetf8_12, rgb, rgba8}};
use vulkan::{Context, Commands, Arc, ImageView, from_iter, BufferUsage, Image as GPUImage, default, ImageCreateInfo, ImageType, Format, ImageUsage, CopyBufferToImageInfo, Sampler, SamplerCreateInfo, Filter, WriteDescriptorSet};
shader!{view}

struct App {
	pass: view::Pass,
	images: Vec<Arc<GPUImage>>,
	index: usize,
}
impl App {
	fn new(context@Context{memory_allocator, ..}: &Context, commands: &mut Commands) -> Result<Self> { Ok(Self{
		pass: view::Pass::new(context, false)?,
		images: std::env::args().skip(1).map(|path| -> Result<_> {
			let Image{size, data, ..} = f32(path)?;
			fn minmax(values: &[f32]) -> [f32; 2] {
				let [mut min, mut max] = [f32::INFINITY, -f32::INFINITY];
				for &value in values { if value > f32::MIN && value < min { min = value; } if value > max { max = value; } }
				[min, max]
			}
			let [min, max] = minmax(&data);
			let image = GPUImage::new(
				memory_allocator.clone(),
				ImageCreateInfo{
					image_type: ImageType::Dim2d,
					format: {assert_eq!(std::mem::size_of::<rgba8>(), 4); Format::R8G8B8A8_SRGB},
					extent: [size.x, size.y, 1],
					usage: ImageUsage::TRANSFER_DST|ImageUsage::SAMPLED,
					..default()
				},
				default()
			)?;
			let oetf = &sRGB8_OETF12;
			let buffer = from_iter(context, BufferUsage::TRANSFER_SRC, data.into_iter().map(|v| rgba8::from(rgb::from(oetf8_12(oetf, ((v-min)/(max-min)).clamp(0., 1.))))))?;
			commands.copy_buffer_to_image(CopyBufferToImageInfo::buffer_image(buffer, image.clone()))?;
			Ok(image)
		}).try_collect()?,
		index: 0,
	})}
}
impl Widget for App { 
fn paint(&mut self, context@Context{device, ..}: &Context, commands: &mut Commands, target: Arc<ImageView>, _: size, _: int2) -> Result<()> {
	let Self{pass, images, index} = self;
	let sampler = Sampler::new(device.clone(), SamplerCreateInfo{mag_filter: Filter::Linear, min_filter: Filter::Linear, ..default()})?;
	pass.begin_rendering(context, commands, target.clone(), None, true, &view::Uniforms::empty(), &[
		WriteDescriptorSet::image_view(0, ImageView::new_default(images[*index].clone())?),
		WriteDescriptorSet::sampler(1, sampler.clone()),
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
