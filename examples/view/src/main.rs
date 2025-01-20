#![feature(slice_from_ptr_range)] // shader
#![feature(iterator_try_collect)]
#![feature(strict_overflow_ops)]
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
			}
			else {
				use std::io::{Read, Seek};
				let mut file = std::fs::File::open(path)?;
				let mut start = [0; 12];
				file.read_exact(&mut start)?;
        		file.seek(std::io::SeekFrom::Start(0))?;
				if &start == b"\x00\x00\x00\x0cJXL \x0d\x0a\x87\x0a" {
					#[cfg(feature="jxl")] {
						use {image::xy, jxl_oxide::{JxlImage, EnumColourEncoding, color}};
						let mut image = JxlImage::builder().open(path).unwrap();
						let size = xy{x: image.width(), y: image.height()};
						let mut target = Image::uninitialized(size);
						image.request_color_encoding(EnumColourEncoding::srgb(color::RenderingIntent::Relative));
						image.render_frame(0).unwrap().stream().write_to_buffer::<u8>(bytemuck::cast_slice_mut::<rgb8, _>(&mut target.data));
						target.map(|v| rgba8::from(v))
					}
					#[cfg(not(feature="jxl"))] unimplemented!()
				}
				else if start.starts_with(b"II*\x00") {
					use {image::xy, rawloader::{decode_file, RawImageData}};
					let image = decode_file(path)?;
					assert_eq!(image.cpp, 1);
					assert_eq!(image.cfa.name, "BGGR");
					let RawImageData::Integer(data) = image.data else {unimplemented!()};
					let cfa = Image::new(xy{x: image.width as u32, y: image.height as u32}, data);
					let mut target = Image::uninitialized(cfa.size/2);
					for y in 0..target.size.y { for x in 0..target.size.x {
						let [b, g10, g01, r] = [[0,0],[1,0],[0,1],[1,1]].map(|[dx,dy]| cfa[xy{x: x*2 + dx, y: y*2 + dy}]);
						target[xy{x,y}] = rgb{r, g: g01.strict_add(g10)/2, b};
					}}
					let (min, max) = (97, 15343);
					target.map(|rgb| rgba8::from(rgb.map(|u16| ((u16-min) as u32 * 0xFF / max) as u8)))
				}
				else { time!(rgb8(path)).map(|v| rgba8::from(v)) }
			}.as_ref())
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
