pub fn default<T: Default>() -> T { Default::default() }
fn minmax(values: &[f32]) -> [f32; 2] {
	let [mut min, mut max] = [f32::INFINITY, -f32::INFINITY];
	for &value in values { if value > f32::MIN && value < min { min = value; } if value > max { max = value; } }
	[min, max]
}

use {image::{bilinear_sample, f32, oetf8_12, rgb, rgb8, rgba8, sRGB8_OETF12, uint2, Image}, std::f32, vector::vec2};

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
		use {vector::{xy, mat3, dot, mulv, MinMax}, image::{rgbf, XYZ}};
		use rawler::{rawsource::RawSource, get_decoder, RawImage, RawImageData, Orientation, decoders::{Camera, WellKnownIFD}, formats::tiff::{Entry, Value}};
		use rawler::{imgop::xyz::Illuminant::D65, tags::DngTag::{/*ForwardMatrix2,*/ OpcodeList2}};
		let ref file = RawSource::new(path.as_ref())?;
		let decoder = get_decoder(file)?;
		let RawImage{cpp, whitelevel, data: RawImageData::Integer(data), width, height, wb_coeffs: rcp_as_shot_neutral, orientation, 
			camera: Camera{forward_matrix, ..}, ..} = decoder.raw_image(file, &default(), false)? else {unimplemented!()};
		assert_eq!(cpp, 1);
		//assert_eq!(cfa.name, "BGGR");
		//assert_eq!(blacklevel, [0,0,0,0]);
		let &[white_level] = whitelevel.0.as_array().unwrap();
		let as_shot_neutral = 1./rgbf::from(*rcp_as_shot_neutral.first_chunk().unwrap()); // FIXME: rawler: expose non-inverted
		assert_eq!(as_shot_neutral.g, 1.);
		
		fn xy(XYZ{X,Y,Z}: XYZ<f64>) -> xy<f64> { xy{x: X/(X+Y+Z), y: Y/(X+Y+Z)}  }
		fn CCT_from_xy(xy{x,y}: xy<f64>) -> f64 {
			let [xe, ye] = [0.3320, 0.1858];
			let n = -(x-xe)/(y-ye);
			449.*n.powi(3) + 3525.*n.powi(2) + 6823.3*n + 5520.33
		}
		fn CCT_from_XYZ(XYZ: XYZ<f64>) -> f64 { CCT_from_xy(xy(XYZ)) }
		
		let tags = decoder.ifd(WellKnownIFD::VirtualDngRawTags)?.unwrap();

		let Some(Entry{value: Value::Undefined(code), ..}) = tags.get_entry(OpcodeList2) else {panic!()};
		use bytemuck::{Zeroable, Pod, cast, cast_slice};
		let ref code = cast_slice::<_,u32>(code);
		let [len, mut ref code @ ..] = code[..] else {panic!()};
		let len = u32::from_be(len);
		let gain = (0..len).map(|_| {
			#[repr(transparent)] #[allow(non_camel_case_types)] #[derive(Clone,Copy,Zeroable,Pod)] struct u32be(u32);
			impl From<u32be> for u32 { fn from(u32be(be): u32be) -> Self { Self::from_be(be) } }
			impl std::fmt::Debug for u32be { fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { u32::from(*self).fmt(f) } }
			#[repr(transparent)] #[allow(non_camel_case_types)] #[derive(Clone,Copy,Zeroable,Pod)] struct f32be([u8; 4]);
			impl From<f32be> for f32 { fn from(f32be(be): f32be) -> Self { Self::from_be_bytes(be) } }
			impl std::fmt::Debug for f32be { fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { f32::from(*self).fmt(f) } }
			#[repr(transparent)] #[allow(non_camel_case_types)] #[derive(Clone,Copy,Zeroable,Pod)] struct f64be([u8; 8]);
			impl From<f64be> for f64 { fn from(f64be(be): f64be) -> Self { Self::from_be_bytes(be) } }
			impl std::fmt::Debug for f64be { fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { f64::from(*self).fmt(f) } }
			#[repr(C,packed)] #[derive(Debug,Clone,Copy,Zeroable,Pod)] struct GainMap {
				id: u32be,
				version: u32be,
				flags: u32be,
				len: u32be,
				top: u32be,
				left: u32be,
				bottom: u32be,
				right: u32be,
				plane: u32be,
				planes: u32be,
				row_pitch: u32be,
				column_pitch: u32be,
				size_y: u32be,
				size_x: u32be,
				map_spacing_vertical: f64be,
				map_spacing_horizontal: f64be,
				map_origin_vertical: f64be,
				map_origin_horizontal: f64be,
				map_planes: u32be,
			}
			let opcode : &[_; 23]; (opcode, code) = code.split_first_chunk().unwrap();
			let GainMap{id, map_planes, size_x, size_y, ..} = cast::<_,GainMap>(*opcode);
			assert_eq!(u32::from(id), 9);
			assert_eq!(u32::from(map_planes), 1);
			let size = uint2{x: size_x.into(), y: size_y.into()};
			let gain; (gain, code) = code.split_at((size.y*size.x) as usize);
			Image::new(size, cast_slice::<_, f32be>(gain)).map(|&gain| f32::from(gain))
		}).collect::<Box<_>>();

		// FIXME: darktable always load D65 and has a dedicated color temperature (chromatic adaptation) much later in the pipeline
		let ref forward = forward_matrix[&D65];
		let forward = *forward.as_array::<{3*3}>().unwrap().map(|v| v.try_into().unwrap()).as_chunks().0.as_array().unwrap();
		let CCT = CCT_from_XYZ(XYZ::<f32>::from(mulv(forward, <[_;_]>::from(as_shot_neutral))).into());
		//println!("CCT: {CCT}");

		const D50 : [[f32; 3]; 3] = [[3.1338561, -1.6168667, -0.4906146], [-0.9787684, 1.9161415, 0.0334540], [0.0719450, -0.2289914, 1.4052427]];
		let R = vector::mul(D50, forward.map(rgbf::from)
			.map(|row| row / as_shot_neutral) // ~ * 1/D
			.map(|row| row / white_level as f32)
			.map(<[_;_]>::from));

		let cfa = Image::new(xy{x: width as u32, y: height as u32}, data);

		//if let Some(exif::Field{value: exif::Value::Float(ref profile_tone_curve), ..} = exif.get_field(ProfileToneCurve, exif::In::PRIMARY) else {

		assert!(gain.iter().all(|&Image{size,..}| size==gain[0].size));
		let scale = vec2::from(gain[0].size-uint2::from(1))/vec2::from(cfa.size/2-uint2::from(1))-vec2::from(f32::EPSILON);
		let luma = Image::from_xy(cfa.size/2, |p| {
			let [b, g10, g01, r] = [[0,0],[1,0],[0,1],[1,1]].map(|[x,y]| cfa[p*2+xy{x,y}]);
			//{assert!(b <= white_level as u16); assert!(g10 <= white_level as u16); assert!(g01 <= white_level as u16); assert!(r <= white_level as u16);}
			let [b, g10, g01, r] = [b, g10, g01, r].map(f32::from);
			let p = scale*vec2::from(p);
			let b = bilinear_sample(&gain[0], p) * b;
			let g10 = bilinear_sample(&gain[1], p) * g10;
			let g01 = bilinear_sample(&gain[2], p) * g01;
			let r = bilinear_sample(&gain[3], p) * r;
			let rgb = rgb{r, g: (g01+g10)/2., b};
			dot(rgb::from(forward[1]), rgb)
		});
		let [_min, _max] = ui::time!(minmax(&luma.data));
		let MinMax{min, max} = ui::time!(vector::minmax(luma.data.iter().copied()).unwrap());
		let len = luma.data.len() as u32;
		let bins = 0x10000; // ~> white_level (dY ~ dG)
		let luma16 = luma.as_ref().map(|luma| (f32::ceil((luma-min)/(max-min)*(bins as f32))-1.) as u16);
		let mut histogram = vec![0; 0x10000];
		for &luma in &luma16.data  { histogram[luma as usize] += 1; }
		let cdf = histogram.into_iter().scan(0, |a, h| { *a += h; Some(*a) }).collect::<Box<[u32]>>();
		assert_eq!(cdf[0xFFFF], len);
		let _adaptive_histogram_equalization = false; //argument_index > 1;
		let image = /*if adaptive_histogram_equalization {
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
				let rgb = apply(R, rgb);
				let f = rank[p] as f32 / (radius+1+radius).pow(2) as f32;
				let rgb = f*rgb;
				let rgb = rgb.map(|c| oetf8_12(oetf, c.clamp(0., 1.)));
				rgba8::from(rgb)
			})
		} else*/ {
			let oetf = &sRGB8_OETF12;
			Image::from_xy(cfa.size/2, |p| {
				let [b, g10, g01, r] = [[0,0],[1,0],[0,1],[1,1]].map(|[x,y]| cfa[p*2+xy{x,y}]);
				//{assert!(b <= white_level as u16); assert!(g10 <= white_level as u16); assert!(g01 <= white_level as u16); assert!(r <= white_level as u16);}
				let [b, g10, g01, r] = [b, g10, g01, r].map(f32::from);
				let p = scale*vec2::from(p);
				let b = bilinear_sample(&gain[0], p) * b;
				let g10 = bilinear_sample(&gain[1], p) * g10;
				let g01 = bilinear_sample(&gain[2], p) * g01;
				let r = bilinear_sample(&gain[3], p) * r;
				let rgb = rgb{r, g: (g01+g10)/2., b};
				pub fn rotate(R: mat3, v: rgb<f32>) -> rgb<f32> { rgb::<f32>::from(mulv(R, v.into())) }
				let rgb = rotate(R, rgb);
				/*let profile_tone_curve = false;
				let global_histogram_equalization = false;
				let normalize = false;
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
				} else if normalize {
					let f = (xyz.Y-min)/(max-min);
					f * xyz / xyz.Y
				} else {
					xyz
				};
				let rgb = D50(xyz);*/
				let rgb = rgb.map(|c| oetf8_12(oetf, c.clamp(0., 1.)));
				rgba8::from(rgb)
			})
		};
		use Orientation::*;
		let image = match orientation {
			Normal => image,
			Rotate90 => Image::from_xy({let xy{x,y} = image.size; xy{x: y, y: x}}, |xy{x,y}| image[xy{x: y, y: image.size.y-1-x}]),
			o => { eprintln!("{o:#?}"); image },
		};
		return Ok(image);
	}
	
	Ok(rgb8(path).map(|v| rgba8::from(v)))
}