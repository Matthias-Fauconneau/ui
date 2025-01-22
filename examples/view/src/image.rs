fn minmax(values: &[f32]) -> [f32; 2] {
	let [mut min, mut max] = [f32::INFINITY, -f32::INFINITY];
	for &value in values { if value > f32::MIN && value < min { min = value; } if value > max { max = value; } }
	[min, max]
}

use {num::lerp, image::{Image, rgb, rgb8, rgba8, f32, sRGB8_OETF12, oetf8_12}};

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
		use {vector::{xy, /*inverse,*/ mat3, dot, mulv, MinMax}, image::{rgbf, XYZ}, rawloader::{decode_file, RawImage, RawImageData, Orientation}};
		let RawImage{cpp, cfa, blacklevels, whitelevels, data: RawImageData::Integer(data), width, height, xyz_to_cam: _color_matrix_2, wb_coeffs: rcp_as_shot_neutral, orientation, ..}
			= decode_file(path)? else {unimplemented!()};
		assert_eq!(cpp, 1);
		assert_eq!(cfa.name, "BGGR");
		assert_eq!(blacklevels, [0,0,0,0]);
		let [white_level, ..] = whitelevels;
		assert!(whitelevels.into_iter().all(|w| w==white_level));
		let as_shot_neutral = 1./rgbf::from(*rcp_as_shot_neutral.first_chunk().unwrap()); // FIXME: fork rawloader to remove confusing inversion
		// FIXME: fork rawloader
		let file = std::fs::File::open(path)?;
		let exif = exif::Reader::new().read_from_container(&mut std::io::BufReader::new(&file))?;
		use exif::Tag;
		#[allow(non_upper_case_globals)] pub const ForwardMatrix1: Tag = Tag(exif::Context::Tiff, 50964);
		#[allow(non_upper_case_globals)] pub const ForwardMatrix2: Tag = Tag(exif::Context::Tiff, 50965);
		//#[allow(non_upper_case_globals)] pub const ProfileToneCurve: Tag = Tag(exif::Context::Tiff, 50940);
		
		let mut xyz_from = [[1.,0.,0.],[0.,1.,0.],[0.,0.,1.]];
		fn xy(XYZ{X,Y,Z}: XYZ<f64>) -> xy<f64> { xy{x: X/(X+Y+Z), y: Y/(X+Y+Z)}  }
		fn xy_from_temperature(T: f64) -> xy<f64> {
			fn g(µ: f64, σ: f64, λ: f64) -> f64 { f64::exp(-((λ-µ)/σ).powi(2)/2.) }
			fn x(λ: f64) -> f64 { 1.065*g(595.8, 33.33, λ) + 0.366*g(446.8, 19.44, λ) }
			fn gln(µ: f64, σ: f64, λ: f64) -> f64 { f64::exp(-((f64::ln(λ)-f64::ln(µ))/σ).powi(2)/2.) }
			fn y(λ: f64) -> f64 { 1.014*gln(556.3, 0.075, λ) }
			fn z(λ: f64) -> f64 { 1.839*gln(449.8, 0.051, λ) }
			use std::f64::consts::PI as π;
			const h: f64 = 6.62607015e-34;
			const c: f64 = 299792458.;
			const k: f64 = 1.380649e-23;
			fn M(T: f64, λ: f64) -> f64 { 2.*π*h*c.powi(2)/λ.powi(5) / (f64::exp(h*c/k / (λ*T))-1.) }
			let ref domain = 400..700;
			let M = domain.clone().map(|λ| M(T, λ as f64)).collect::<Box<_>>();
			let [X,Y,Z] = [x,y,z].map(|cmf| domain.clone().zip(&M).map(|(λ, Mλ)| cmf(λ as f64)*Mλ).sum::<f64>());
			xy{x: X/(X+Y+Z), y: Y/(X+Y+Z)}
		}
		fn CCT_from_xy(xy{x,y}: xy<f64>) -> f64 {
			let [xe, ye] = [0.3320, 0.1858];
			let n = -(x-xe)/(y-ye);
			let T = 449.*n.powi(3) + 3525.*n.powi(2) + 6823.3*n + 5520.33;
			assert_eq!(xy_from_temperature(T), xy{x,y});
			T
		}
		let mut CCT = CCT_from_xy(xy(XYZ::<f32>::from(mulv(xyz_from, <[_;_]>::from(as_shot_neutral))).into()));
		
		for _ in  0..2 {
			xyz_from = if let Some([exif::Field{value: exif::Value::SRational(forward_matrix_1), ..}, exif::Field{value: exif::Value::SRational(forward_matrix_2), ..}])
			= [ForwardMatrix1,ForwardMatrix2].try_map(|tag| exif.get_field(tag, exif::In::PRIMARY)) {
				let illuminants = [/*A*/2856., /*D*/6500.]; // FIXME: assert DNG tags
				let forward_matrix = [forward_matrix_1, forward_matrix_2].map(|fm| *fm.as_array::<{3*3}>().unwrap().map(|v| v.to_f32()).as_chunks().0.as_array().unwrap());
				//let forward_matrix = forward_matrix.map(|forward_matrix| forward_matrix.map(|row| XYZ::<f32>::from(row)).map(|row| row/row.sum()).map(<[_;_]>::from)); // Not sure
				let t = (CCT -illuminants[0])/(illuminants[1]-illuminants[0]);
				let T = lerp(t as f32, forward_matrix[0], forward_matrix[1]);
				let T = T.map(|row| XYZ::<f32>::from(row)).map(|row| row/row.sum()).map(<[_;_]>::from); // Just to be sure
				//println!("{CCT} {t}\n{:?}\n{:?}\n{:?}", forward_matrix[0], forward_matrix[1], T);
				T
			} else {
				unimplemented!("missing color matrix 1: get directly from exif or expose through rawloader (fork)");
				/*let color_matrix_2 : [_; 3] = *color_matrix_2.first_chunk()/*RGB*/.unwrap(); // normally D65
				//let t = (1./CCT-1./illuminants[0])/(1./illuminants[1]-1./illuminants[0]);
				let color_matrix = color_matrix_2; // FIXME: lerp(t, color_matrix_1, color_matrix_2) i.e interpolate color_matrix[] for shot illuminant
				//let color_matrix = color_matrix.map(|row| XYZ::<f32>::from(row)).map(|row| row/row.sum()).map(<[_;_]>::from); // Not sure about this
				assert_eq!(mulv(color_matrix, [1.,1.,1.]), [1.,1.,1.]);
				inverse::<3>(color_matrix)*/
			};
			CCT = CCT_from_xy(xy(XYZ::<f32>::from(mulv(xyz_from, <[_;_]>::from(as_shot_neutral))).into()));
			println!("{CCT}");
		}
		assert_eq!(CCT, 5728.);
		const D50 : [[f32; 3]; 3] = [[3.1338561, -1.6168667, -0.4906146], [-0.9787684, 1.9161415, 0.0334540], [0.0719450, -0.2289914, 1.4052427]];
		let R = vector::mul(D50, xyz_from.map(rgbf::from)
			.map(|row| row / as_shot_neutral) // ~ * 1/D
			.map(|row| row / white_level as f32)
			.map(|rgb{r,g,b}| rgb{r, g: g/2. /*g01+g10*/, b})
			.map(<[_;_]>::from));
		//assert_eq!(rgb::from(mulv(R, (rgb{r: white_level as f32, g: 2.*white_level as f32, b: white_level as f32}*as_shot_neutral).into())), rgb{r: 1., g: 1., b: 1.});

		let cfa = Image::new(xy{x: width as u32, y: height as u32}, data);

		//if let Some(exif::Field{value: exif::Value::Float(ref profile_tone_curve), ..} = exif.get_field(ProfileToneCurve, exif::In::PRIMARY) else {

		let luma = Image::from_xy(cfa.size/2, |p| {
			let [b, g10, g01, r] = [[0,0],[1,0],[0,1],[1,1]].map(|[x,y]| cfa[p*2+xy{x,y}]);
			let rgb = rgb{r, g: g01.strict_add(g10), b};
			{let rgb{r,g,b} = rgb; assert!(r <= white_level); assert!(g <= white_level*2); assert!(b <= white_level);}
			let rgb = rgb::<f32>::from(rgb) ;
			dot(rgb::from(xyz_from[1]), rgb)
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
				let rgb = rgb{r, g: g01.strict_add(g10), b};
				let rgb = rgb::<f32>::from(rgb) ;
				//pub fn xyz_from_rgb(m: mat3, v: rgb<f32>) -> XYZ<f32> { XYZ::<f32>::from(mulv(m, v.into())) }
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
			o => { dbg!(o); image },
		};
		return Ok(image);
	}
	
	Ok(rgb8(path).map(|v| rgba8::from(v)))
}