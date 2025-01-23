#[path="dng.rs"] mod dng;

pub fn minmax(values: &[f32]) -> [f32; 2] {
	let [mut min, mut max] = [f32::INFINITY, -f32::INFINITY];
	for &value in values { if value < min { min = value; } if value > max { max = value; } }
	[min, max]
}

use image::{Image, rgb, rgba8, load_rgb8, load_exr, oetf8_12, sRGB8_OETF12};
pub fn load(ref path: impl AsRef<std::path::Path>) -> Result<Image<Box<[rgba8]>>, Box<dyn std::error::Error>> {
	if let Ok(image) = load_exr(path) {
		let [min, max] = minmax(&image.data);
		let oetf = &sRGB8_OETF12;
		return Ok(image.map(|v| rgba8::from(rgb::from(oetf8_12(oetf, ((v-min)/(max-min)).clamp(0., 1.))))));
	}
	
	//if &start == b"\x00\x00\x00\x0cJXL \x0d\x0a\x87\x0a"
	#[cfg(feature="jxl")] {
		use {image::{xy, rgb8}, jxl_oxide::{JxlImage, EnumColourEncoding, color}};
		if let Ok(mut image) = JxlImage::builder().open(path) {
			let size = xy{x: image.width(), y: image.height()};
			let mut target = Image::uninitialized(size);
			image.request_color_encoding(EnumColourEncoding::srgb(color::RenderingIntent::Relative));
			image.render_frame(0).unwrap().stream().write_to_buffer::<u8>(bytemuck::cast_slice_mut::<rgb8, _>(&mut target.data));
			return Ok(target.map(|v| rgba8::from(v)));
		}
	}
	use std::io::{Read, Seek};
	let mut file = std::fs::File::open(path)?;
	let mut start = [0; 12];
	file.read_exact(&mut start)?;
	file.seek(std::io::SeekFrom::Start(0))?;
	if start.starts_with(b"II*\x00") { return dng::load(path); }
	
	Ok(load_rgb8(path).map(|v| rgba8::from(v)))
}