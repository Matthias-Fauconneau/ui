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
		images: std::env::args().skip(1).enumerate().map(|(argument_index, ref path)| image(context, commands, if let Ok(Image{size, data, ..}) = f32(path) {
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
					assert_eq!(mulv(xyz_to_cam, [1.,1.,1.]), [1.,1.,1.]);
					let xyz_from = inverse::<3>(xyz_to_cam);
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
					if argument_index == 0 {
						let (bins, luma) = if true {
							let bins = 0x10000; // > white_level .next_power_of_two() as usize
							let luma = luma.map(|luma| (f32::ceil((luma-min)/(max-min)*(bins as f32))-1.) as u16);
							let mut histogram = vec![0; 0x10000];
							for &luma in &luma.data  { histogram[luma as usize] += 1; }
							let cdf = histogram.into_iter().scan(0, |a, h| { *a += h; Some(*a) }).collect::<Box<[u32]>>();
							let len = luma.data.len() as u32;
							assert_eq!(cdf[0xFFFF], len);
							let bins = 0x800; // TODO: multilevel histogram, SIMD
							let luma = luma.map(|luma| u16::try_from(cdf[luma as usize] as u64 * (bins-1) as u64 / (len-1) as u64).unwrap());
							(bins, luma)
						} else {
							let bins = 0x800; // TODO: multilevel histogram, SIMD
							let luma = luma.map(|luma| (f32::ceil((luma-min)/(max-min)*(bins as f32))-1.) as u16);
							(bins, luma)
						};
						let radius = 367i32; //std::cmp::min(luma.size.x, luma.size.y)/4;
						//assert!((radius+1+radius).pow(2) <= 0xFFFF);
						//assert!((radius+1+radius) as usize >= bins, "{bins} {radius}"); // TODO: multilevel histogram, SIMD
						assert!(luma.size.x as usize*bins as usize*2 <= 64*1024*1024);
						let mut column_histograms = vec![vec![0; bins as usize]; luma.size.x as usize]; // ~120M. More efficient to slide : packed add vs indirect scatter add
						let mut f = Image::<Box<[u32]>>::zero(luma.size);
						let mut y = 0;
						for y in -radius..=radius { for x in 0..luma.size.x { column_histograms[x as usize][luma[xy{x,y: y.max(0) as u32}] as usize] += 1; } }
						let [w, h] = luma.size.signed().into();
						let stride = luma.stride as i32; // Slightly less verbose sign casting
						assert_eq!(f.stride as i32, stride);
						let start = std::time::Instant::now();
						loop {
							if !(y < h) { break; }
							//println!("{y}");
							let mut histogram = vec![0; bins as usize];
							for x in -radius..=radius { for bin in 0..bins { histogram[bin as usize] += column_histograms[x.max(0) as usize][bin as usize]; } }
							for x in 0..w-1 {
								let luma = luma[(y*stride+x) as usize];
								f[(y*stride+x) as usize] = histogram[0..=luma as usize].iter().sum();
								// Slide right
								for bin in 0..bins { histogram[bin as usize] = (histogram[bin as usize] as i32
									+ column_histograms[(x+radius+1).min(w-1) as usize][bin as usize] as i32
									- column_histograms[(x-radius).max(0) as usize][bin as usize] as i32) as u32;
								}
							}
							{ // Last of row iteration (not sliding further right after)
								let x = w-1;
								let luma = luma[(y*stride+x) as usize];
								f[(y*stride+x) as usize] = histogram[0..=luma as usize].iter().sum();
							}
							// Slide down
							for x in 0..w {
								column_histograms[x as usize][luma[((y-radius).max(0)*stride+x) as usize] as usize] -= 1;
								column_histograms[x as usize][luma[((y+radius+1).min(h-1)*stride+x) as usize] as usize] += 1;
							}
							y += 1;
							if !(y < h) { break; }
							let mut histogram = vec![0; bins as usize];
							for x in -radius..=radius { for bin in 0..bins { histogram[bin as usize] += column_histograms[x.max(0) as usize][bin as usize]; } }
							for x in (1..w).into_iter().rev() {
								let luma = luma[(y*stride+x) as usize];
								f[(y*stride+x) as usize] = histogram[0..=luma as usize].iter().sum();
								// Slide left
								for bin in 0..bins { histogram[bin as usize] = (histogram[bin as usize] as i32
									+ column_histograms[(x-radius-1).max(0) as usize][bin as usize] as i32
									- column_histograms[(x+radius).min(w-1) as usize][bin as usize] as i32) as u32;
								}
							}
							{ // Back to first of row iteration (not sliding further left after)
								let x = 0;
								let luma = luma[(y*stride+x) as usize];
								f[(y*stride+x) as usize] = histogram[0..=luma as usize].iter().sum();
							}
							// Slide down
							for x in 0..w {
								column_histograms[x as usize][luma[((y-radius).max(0)*stride+x) as usize] as usize] -= 1;
								column_histograms[x as usize][luma[((y+radius+1).min(h-1)*stride+x) as usize] as usize] += 1;
							}
							y += 1;
						}
						println!("{}ms", start.elapsed().as_millis());
						let oetf = &sRGB8_OETF12;
						Image::from_xy(cfa.size/2, |p| {
							let [b, g10, g01, r] = [[0,0],[1,0],[0,1],[1,1]].map(|[x,y]| cfa[p*2+xy{x,y}]);
							let rgb = rgb{r, g: g01.strict_add(g10), b};
							let rgb = rgb::<f32>::from(rgb) ;
							pub fn apply(m: mat3, v: rgb<f32>) -> XYZ<f32> { XYZ::<f32>::from(mulv(m, v.into())) }
							let xyz = apply(xyz_from, rgb);
							let f = f[p] as f32 / (radius+1+radius).pow(2) as f32;
							let rgb = rgb::<f32>::from(f*xyz/xyz.Y);
							let rgb = rgb.map(|c| oetf8_12(oetf, c.clamp(0., 1.)));
							rgba8::from(rgb)
						})
					} else {
						let oetf = &sRGB8_OETF12;
						Image::from_xy(cfa.size/2, |p| {
							let [b, g10, g01, r] = [[0,0],[1,0],[0,1],[1,1]].map(|[x,y]| cfa[p*2+xy{x,y}]);
							let rgb = rgb{r, g: g01.strict_add(g10), b};
							let rgb = rgb::<f32>::from(rgb) ;
							pub fn apply(m: mat3, v: rgb<f32>) -> XYZ<f32> { XYZ::<f32>::from(mulv(m, v.into())) }
							let xyz = apply(xyz_from, rgb);
							let f = (xyz.Y-min)/(max-min);
							let rgb = rgb::<f32>::from(f*xyz/xyz.Y);
							let rgb = rgb.map(|c| oetf8_12(oetf, c.clamp(0., 1.)));
							rgba8::from(rgb)
						})
					}
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
