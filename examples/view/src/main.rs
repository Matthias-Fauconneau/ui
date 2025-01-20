#![feature(slice_from_ptr_range)] // shader
#![feature(iterator_try_collect)]
#![feature(strict_overflow_ops)]
#![feature(generic_arg_infer)] // <[_;_]>::from
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
					use {vector::{xy, inverse, mat3, dot, mulv, MinMax}, image::{rgbf, XYZ}, rawloader::{decode_file, RawImageData}};
					let image = decode_file(path)?;
					assert_eq!(image.cpp, 1);
					assert_eq!(image.cfa.name, "BGGR");
					assert_eq!(image.blacklevels, [0,0,0,0]);
					let [white_level, ..] = image.whitelevels;
					assert!(image.whitelevels.into_iter().all(|w| w==white_level));
					let RawImageData::Integer(data) = image.data else {unimplemented!()};
					let cfa = Image::new(xy{x: image.width as u32, y: image.height as u32}, data);
					let xyz_to_cam : [_; 3] = *image.xyz_to_cam.first_chunk()/*RGB*/.unwrap();
					let xyz_to_cam = xyz_to_cam.map(|row| XYZ::<f32>::from(row)).map(|row| row/row.sum()).map(<[_;_]>::from);
					//let xyz_to_cam = transpose(transpose(xyz_to_cam).map(|column| XYZ::<f32>::from(column)).map(|column| column/column.sum()).map(<[_;_]>::from));
					assert_eq!(mulv(xyz_to_cam, [1.,1.,1.]), [1.,1.,1.]);
					let xyz_from = inverse::<3>(xyz_to_cam);
					//assert_eq!(mulv(xyz_from, [1.,1.,1.]), [1.,1.,1.]);
					let wb_coeffs = rgbf::from(*image.wb_coeffs.first_chunk().unwrap()); // 1/AsShotNeutral
					let xyz_from = xyz_from
						.map(|row| rgbf::from(row))
						.map(|row| row * wb_coeffs / white_level as f32)
						.map(|rgb{r,g,b}| rgb{r, g: g/2. /*g01+g10*/, b})
						.map(<[_;_]>::from);
					
					let luma = Image::from_xy(cfa.size/2, |p| {
						let [b, g10, g01, r] = [[0,0],[1,0],[0,1],[1,1]].map(|[x,y]| cfa[p*2+xy{x,y}]);
						let rgb = rgb{r, g: g01.strict_add(g10), b};
						{let rgb{r,g,b} = rgb; assert!(r <= white_level); assert!(g <= white_level*2); assert!(b <= white_level);}
						let rgb = rgb::<f32>::from(rgb) ;
						dot(rgb::from(xyz_from[1]), rgb)
					});
					let MinMax{min, max} = vector::minmax(luma.data.iter().copied()).unwrap();
					let luma = luma.map(|luma| (f32::ceil((luma-min)/(max-min)*(0x10000 as f32))-1.) as u16); // FIXME: 14bit/16K should be enough
					let mut histogram = vec![0; 0x10000];
					for &luma in &luma.data  { histogram[luma as usize] += 1; }
					let cdf = histogram.into_iter().scan(0, |a, h| { *a += h; Some(*a) }).collect::<Box<_>>();
					let len = luma.data.len();
					assert_eq!(cdf[0xFFFF], len);
					let oetf = &sRGB8_OETF12;
					Image::from_xy(cfa.size/2, |p| {
						let [b, g10, g01, r] = [[0,0],[1,0],[0,1],[1,1]].map(|[x,y]| cfa[p*2+xy{x,y}]);
						let rgb = rgb{r, g: g01.strict_add(g10), b};
						let rgb = rgb::<f32>::from(rgb) ;
						pub fn apply(m: mat3, v: rgb<f32>) -> XYZ<f32> { XYZ::<f32>::from(mulv(m, v.into())) }
						let xyz = apply(xyz_from, rgb);
						let luma = (f32::ceil((xyz.Y-min)/(max-min)*(0x10000 as f32))-1.) as u16;
						let f = (cdf[luma as usize] as f32 / len as f32) / xyz.Y;
						let rgb = rgb::<f32>::from(f*xyz);
						let rgb = rgb.map(|c| oetf8_12(oetf, c.clamp(0., 1.)));
						rgba8::from(rgb)
					})
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
