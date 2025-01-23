
pub fn default<T: Default>() -> T { Default::default() }
use vector::{xy, uint2, vec2, mat3, mulv, MinMax};
use image::{Image, XYZ, rgb, rgbf, rgba8, bilinear_sample, blur_3, oetf8_12, sRGB8_OETF12};
use rawler::{rawsource::RawSource, get_decoder, RawImage, RawImageData, Orientation, decoders::{Camera, WellKnownIFD}, formats::tiff::{Entry, Value}};
use rawler::{imgop::xyz::Illuminant::D65, tags::DngTag::{/*ForwardMatrix2,*/ OpcodeList2}};

fn gain(code: &[u8]) -> [Image<Box<[f32]>>; 4] {
	use bytemuck::{Zeroable, Pod, cast, cast_slice};
	let code = cast_slice::<_,u32>(code);
	let [len, mut ref code @ ..] = code[..] else {panic!()};
	let len = u32::from_be(len);
	*(0..len).map(|_| {
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
	}).collect::<Box<_>>().into_array().unwrap()
}

pub fn load(path: impl AsRef<std::path::Path>) -> Result<Image<Box<[rgba8]>>, Box<dyn std::error::Error>> {
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
	
	let tags = decoder.ifd(WellKnownIFD::VirtualDngRawTags)?.unwrap();

	let Some(Entry{value: Value::Undefined(code), ..}) = tags.get_entry(OpcodeList2) else {panic!()};
	let gain = gain(code);

	// FIXME: darktable always load D65 and has a dedicated color temperature (chromatic adaptation) much later in the pipeline
	let ref forward_matrix = forward_matrix[&D65];
	let forward_matrix = *forward_matrix.as_array::<{3*3}>().unwrap().map(|v| v.try_into().unwrap()).as_chunks().0.as_array().unwrap();
	
	/*fn CCT_from_xy(xy{x,y}: xy<f64>) -> f64 {
		let [xe, ye] = [0.3320, 0.1858];
		let n = -(x-xe)/(y-ye);
		449.*n.powi(3) + 3525.*n.powi(2) + 6823.3*n + 5520.33
	}
	fn CCT_from_XYZ(XYZ: XYZ<f64>) -> f64 { CCT_from_xy(xy(XYZ)) }
	let CCT = CCT_from_XYZ(XYZ::<f32>::from(mulv(forward, <[_;_]>::from(as_shot_neutral))).into());
	println!("CCT: {CCT}");*/

	let forward_matrix = forward_matrix.map(rgbf::from)
		.map(|row| row / as_shot_neutral) // ~ * 1/D
		.map(|row| row / white_level as f32)
		.map(<[_;_]>::from);

	let cfa = Image::new(xy{x: width as u32, y: height as u32}, data);

	assert!(gain.iter().all(|&Image{size,..}| size==gain[0].size));
	let scale = vec2::from(gain[0].size-uint2::from(1))/vec2::from(cfa.size/2-uint2::from(1))-vec2::from(f32::EPSILON);
	let image = Image::from_xy(cfa.size/2, |p| {
		let [b, g10, g01, r] = [[0,0],[1,0],[0,1],[1,1]].map(|[x,y]| cfa[p*2+xy{x,y}]);
		let [b, g10, g01, r] = [b, g10, g01, r].map(f32::from);
		let p = scale*vec2::from(p);
		let b = bilinear_sample(&gain[0], p) * b;
		let g10 = bilinear_sample(&gain[1], p) * g10;
		let g01 = bilinear_sample(&gain[2], p) * g01;
		let r = bilinear_sample(&gain[3], p) * r;
		let rgb = rgb{r, g: (g01+g10)/2., b};
		pub fn forward(forward: mat3, v: rgb<f32>) -> XYZ<f32> { XYZ::<f32>::from(mulv(forward, v.into())) }
		forward(forward_matrix, rgb)
	});
	let pixel_count = image.data.len() as u32;
	
	let haze = blur_3::<512>(&image);
	//let image = Image::from_iter(image.size, image.data.iter().zip(haze.data).map(|(image, haze)| image-haze));
	// TODO: ProfileToneCurve
	fn xy(XYZ{X,Y,Z}: XYZ<f32>) -> xy<f32> { xy{x: X/(X+Y+Z), y: Y/(X+Y+Z)} }
	let xy = image.as_ref().map(|&XYZ| xy(XYZ));
	// /!\ keeping negative values (to be normalized)
	let Y = Image::from_iter(image.size, image.data.iter().zip(haze.data).map(|(XYZ{Y:image,..},XYZ{Y:haze,..})| image-haze));
	//let Y = image.as_ref().map(|&XYZ{Y,..}| Y);
	
	let MinMax{min, max} = ui::time!(vector::minmax(Y.data.iter().copied()).unwrap());
	let bins = 0x10000;
	let mut histogram = vec![0; bins];
	for Y in &Y.data {
		let bin = f32::ceil((Y-min)/(max-min)*bins as f32-1.) as u32;
		histogram[bin as usize] += 1; 
	}
	let cdf = histogram.into_iter().scan(0, |a, h| { *a += h; Some(*a) }).collect::<Box<[u32]>>();
	//assert_eq!(cdf[0xFFFF], pixel_count);
	
	const D50 : [[f32; 3]; 3] = [[3.1338561, -1.6168667, -0.4906146], [-0.9787684, 1.9161415, 0.0334540], [0.0719450, -0.2289914, 1.4052427]];
	let oetf = &sRGB8_OETF12;
	let image = Image::from_iter(image.size, Y.data.into_iter().zip(xy.data).map(|(Y,xy{x,y})| {
		let z = 1.-x-y;
		let [a, b] = [5.*(x - y), 2.*(y - z)];
		let [a, b] = [6./5.*a, 6./5.*b];
		let [x, z] = [y + a/5., y - b/2.];
		let y = 1.-x-z; // 1-(y+a)-(y-b) = 1-(y-x-y)-(y-y-z) = 1-x-z
		let bin = f32::ceil((Y-min)/(max-min)*bins as f32-1.) as u32;
		let rank = cdf[bin as usize];
		let Y = rank as f32 / (pixel_count-1) as f32;
		let XYZ = XYZ{X: Y/y*x, Y, Z: Y/y*z};
		
		rgba8::from(rgb::from(mulv(D50, XYZ.into()).map(|c| oetf8_12(oetf, c.clamp(0., 1.)))))
	}));
	
	use Orientation::*;
	let image = match orientation {
		Normal => image,
		Rotate90 => Image::from_xy({let xy{x,y} = image.size; xy{x: y, y: x}}, |xy{x,y}| image[xy{x: y, y: image.size.y-1-x}]),
		o => { eprintln!("{o:#?}"); image },
	};
	Ok(image)
}
