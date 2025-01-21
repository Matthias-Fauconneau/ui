fn minmax(values: &[f32]) -> [f32; 2] {
	let [mut min, mut max] = [f32::INFINITY, -f32::INFINITY];
	for &value in values { if value > f32::MIN && value < min { min = value; } if value > max { max = value; } }
	[min, max]
}

use ::image::{Image, rgb, rgb8, rgba8, f32, sRGB8_OETF12, oetf8_12};

pub fn load(ref path: impl AsRef<std::path::Path>) -> Result<Image<Box<[rgba8]>>, Box<dyn std::error::Error>> {
	if let Ok(image) = f32(path) {
		let [min, max] = minmax(&image.data);
		let oetf = &sRGB8_OETF12;
		return Ok(image.map(|v| rgba8::from(rgb::from(oetf8_12(oetf, ((v-min)/(max-min)).clamp(0., 1.))))));
	}
	
	//if &start == b"\x00\x00\x00\x0cJXL \x0d\x0a\x87\x0a"
	#[cfg(feature="jxl")] {
		use {image::xy, jxl_oxide::{JxlImage, EnumColourEncoding, color}};
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
	if start.starts_with(b"II*\x00") {
		use {vector::{xy, inverse, mat3, dot, mulv, MinMax}, image::{rgbf, XYZ}, rawloader::{decode_file, RawImage, RawImageData, Orientation}};
		let RawImage{cpp, cfa, blacklevels, whitelevels, data: RawImageData::Integer(data), width, height, xyz_to_cam: color_matrix_2, wb_coeffs: rcp_as_shot_neutral, orientation, ..}
			= decode_file(path)? else {unimplemented!()};
		assert_eq!(cpp, 1);
		assert_eq!(cfa.name, "BGGR");
		assert_eq!(blacklevels, [0,0,0,0]);
		let [white_level, ..] = whitelevels;
		assert!(whitelevels.into_iter().all(|w| w==white_level));
		let rcp_as_shot_neutral = rgbf::from(*rcp_as_shot_neutral.first_chunk().unwrap());
		
		// FIXME: fork rawloader
		let file = std::fs::File::open(path)?;
		let exif = exif::Reader::new().read_from_container(&mut std::io::BufReader::new(&file))?;
		use exif::Tag;
		#[allow(non_upper_case_globals)] pub const ForwardMatrix2: Tag = Tag(exif::Context::Tiff, 50965);
		//#[allow(non_upper_case_globals)] pub const ProfileToneCurve: Tag = Tag(exif::Context::Tiff, 50940);
		
		let xyz_from = if let Some(exif::Field{value: exif::Value::SRational(forward_matrix_2), ..}) = exif.get_field(ForwardMatrix2, exif::In::PRIMARY) {
			*forward_matrix_2.as_array::<{3*3}>().unwrap().map(|v| v.to_f32()).as_chunks().0.as_array().unwrap()
		} else {
			let color_matrix_2 : [_; 3] = *color_matrix_2.first_chunk()/*RGB*/.unwrap(); // normally D65
			let color_matrix = color_matrix_2; // FIXME: mix(color_matrix_1, color_matrix_2, as_shot_neutral) i.e interpolate color_matrix[] for shot illuminant
			let color_matrix = color_matrix.map(|row| XYZ::<f32>::from(row)).map(|row| row/row.sum()).map(<[_;_]>::from); // Not sure about this
			assert_eq!(mulv(color_matrix, [1.,1.,1.]), [1.,1.,1.]);
			inverse::<3>(color_matrix)
		};
		println!("{:?}", xyz_from.map(|r| r.map(|v| (v*10_000.) as i32)));
		let xyz_from = xyz_from.map(rgbf::from)
			.map(|row| rcp_as_shot_neutral * row)
			.map(|row| row / white_level as f32)
			.map(|rgb{r,g,b}| rgb{r, g: g/2. /*g01+g10*/, b})
			.map(<[_;_]>::from);

		let cfa = Image::new(xy{x: width as u32, y: height as u32}, data);

		//if let Some(exif::Field{value: exif::Value::Float(ref profile_tone_curve), ..} = exif.get_field(ProfileToneCurve, exif::In::PRIMARY) else {

		let luma = Image::from_xy(cfa.size/2, |p| {
			let [b, g10, g01, r] = [[0,0],[1,0],[0,1],[1,1]].map(|[x,y]| cfa[p*2+xy{x,y}]);
			let rgb = rgb{r, g: g01.strict_add(g10), b};
			{let rgb{r,g,b} = rgb; assert!(r <= white_level); assert!(g <= white_level*2); assert!(b <= white_level);}
			let rgb = rgb::<f32>::from(rgb) ;
			dot(rgb::from(xyz_from[1]), rgb)
		});
		let [min, max] = ui::time!(minmax(&luma.data));
		let MinMax{min, max} = ui::time!(vector::minmax(luma.data.iter().copied()).unwrap());
		let len = luma.data.len() as u32;
		let bins = 0x10000; // ~> white_level (dY ~ dG)
		let luma16 = luma.as_ref().map(|luma| (f32::ceil((luma-min)/(max-min)*(bins as f32))-1.) as u16);
		let mut histogram = vec![0; 0x10000];
		for &luma in &luma16.data  { histogram[luma as usize] += 1; }
		let cdf = histogram.into_iter().scan(0, |a, h| { *a += h; Some(*a) }).collect::<Box<[u32]>>();
		assert_eq!(cdf[0xFFFF], len);
		let adaptive_histogram_equalization = false; //argument_index > 1;
		let image = if adaptive_histogram_equalization {
			let (bins, luma) = if false {
				let bins = 0x800; // TODO: multilevel histogram, SIMD
				let luma = luma16.map(|luma| u16::try_from(cdf[luma as usize] as u64 * (bins-1) as u64 / (len-1) as u64).unwrap());
				(bins, luma)
			} else {
				let bins = 0x800; // TODO: multilevel histogram, SIMD
				let luma = luma.map(|luma| (f32::ceil((luma-min)/(max-min)*(bins as f32))-1.) as u16);
				(bins, luma)
			};
			let radius = ((std::cmp::min(luma.size.x, luma.size.y) - 1) / 2) as i32;
			//assert_eq!(radius, 733);
			//assert!((radius+1+radius) as usize >= bins);
			//assert!(luma.size.x as usize*bins as usize*2 <= 64*1024*1024);
			let mut column_histograms = vec![vec![0; bins as usize]; luma.size.x as usize]; // ~120M. More efficient to slide : packed add vs indirect scatter add
			let mut rank = Image::<Box<[u32]>>::zero(luma.size);
			let mut y = 0;
			for y in -radius..=radius { for x in 0..luma.size.x { column_histograms[x as usize][luma[xy{x,y: y.max(0) as u32}] as usize] += 1; } }
			let [w, h] = luma.size.signed().into();
			let stride = luma.stride as i32; // Slightly less verbose sign casting
			assert_eq!(rank.stride as i32, stride);
			let start = std::time::Instant::now();
			loop {
				if !(y < h) { break; }
				//println!("{y}");
				let mut histogram = vec![0; bins as usize];
				for x in -radius..=radius { for bin in 0..bins { histogram[bin as usize] += column_histograms[x.max(0) as usize][bin as usize]; } }
				for x in 0..w-1 {
					let luma = luma[(y*stride+x) as usize];
					rank[(y*stride+x) as usize] = histogram[0..=luma as usize].iter().sum();
					// Slide right
					for bin in 0..bins { histogram[bin as usize] = (histogram[bin as usize] as i32
						+ column_histograms[(x+radius+1).min(w-1) as usize][bin as usize] as i32
						- column_histograms[(x-radius).max(0) as usize][bin as usize] as i32) as u32;
					}
				}
				{ // Last of row iteration (not sliding further right after)
					let x = w-1;
					let luma = luma[(y*stride+x) as usize];
					rank[(y*stride+x) as usize] = histogram[0..=luma as usize].iter().sum();
				}
				// Slide down
				for x in 0..w {
					column_histograms[x as usize][luma[((y-radius).max(0)*stride+x) as usize] as usize] -= 1;
					column_histograms[x as usize][luma[((y+radius+1).min(h-1)*stride+x) as usize] as usize] += 1;
				}
				y += 1;
				if !(y < h) { break; }
				let mut histogram = vec![0; bins as usize];
				for x in (w-1)-radius..=(w-1)+radius { for bin in 0..bins { histogram[bin as usize] += column_histograms[x.min(w-1) as usize][bin as usize]; } }
				for x in (1..w).into_iter().rev() {
					let luma = luma[(y*stride+x) as usize];
					rank[(y*stride+x) as usize] = histogram[0..=luma as usize].iter().sum();
					// Slide left
					for bin in 0..bins { histogram[bin as usize] = (histogram[bin as usize] as i32
						+ column_histograms[(x-radius-1).max(0) as usize][bin as usize] as i32
						- column_histograms[(x+radius).min(w-1) as usize][bin as usize] as i32) as u32;
					}
				}
				{ // Back to first of row iteration (not sliding further left after)
					let x = 0;
					let luma = luma[(y*stride+x) as usize];
					rank[(y*stride+x) as usize] = histogram[0..=luma as usize].iter().sum();
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
				let f = rank[p] as f32 / (radius+1+radius).pow(2) as f32;
				let rgb = rgb::<f32>::from(f*xyz);
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
				let profile_tone_curve = false;
				let global_histogram_equalization = false;
				let xyz = if profile_tone_curve {
					unimplemented!("ProfileToneCurve (as used in on-board processed version)")
				} else if global_histogram_equalization {
					let luma = if true { luma16[p] as usize }
					else { // or better recompute (if bandwidth limited) ?
						let luma = (xyz.Y-min)/(max-min);
						(f32::ceil(luma*(bins as f32))-1.) as usize
					};
					let rank = cdf[luma as usize];
					let f = rank as f32 / (len-1) as f32;
					f * xyz / xyz.Y
				} else {
					let f = (xyz.Y-min)/(max-min);
					f * xyz / xyz.Y
				};
				let rgb = rgb::<f32>::from(xyz);
				let rgb = rgb.map(|c| oetf8_12(oetf, c.clamp(0., 1.)));
				rgba8::from(rgb)
			})
		};
		use Orientation::*;
		let image = match orientation {
			Normal => image,
			Rotate90 => Image::from_xy({let xy{x,y} = image.size; xy{x: y, y: x}}, |xy{x,y}| image[xy{x: y, y: image.size.y-1-x}]),
			o => { dbg!(o); image },
		};
		return Ok(image);
	}
	
	Ok(rgb8(path).map(|v| rgba8::from(v)))
}