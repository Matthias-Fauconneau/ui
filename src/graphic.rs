pub use vector::int2;

pub struct Parallelogram { pub top_left: int2, pub bottom_right: int2, pub vertical_thickness: u32 }

impl Parallelogram {
	pub fn translate(&mut self, offset: int2) { self.top_left += offset; self.bottom_right += offset; }
}

pub struct Glyph<'t> { pub top_left: int2, pub face: &'t Face<'t>, pub id: ttf_parser::GlyphId, pub scale: Ratio }

impl Glyph<'_> {
	pub fn translate(&mut self, offset: int2) { self.top_left += offset; }
}

pub use {num::Ratio, vector::Rect, ttf_parser::Face};

pub fn horizontal(y: i32, dy: u32, x0: i32, x1: i32) -> Rect { Rect{ min: xy{ y: y-(dy/2) as i32, x: x0 }, max: xy{ y: y+(dy/2) as i32, x: x1 } } }
pub fn vertical   (x: i32, dx: u32, y0: i32, y1: i32) -> Rect { Rect{ min: xy{ x: x-(dx/2) as i32, y: y0 }, max: xy{ x: x+(dx/2) as i32, y: y1 } } }

pub struct Graphic<'t> {
	pub scale: Ratio,
	pub rects: Vec<Rect>,
	pub parallelograms: Vec<Parallelogram>,
	pub glyphs: Vec<Glyph<'t>>,
}

use {fehler::throws, crate::{Error, Result, widget::{self, Target, size}, font::rect}, vector::{xy, ifloor, ceil}};

impl Graphic<'_> {
	pub fn new(scale: Ratio) -> Self { Self{scale, rects: vec![], parallelograms: vec![], glyphs: vec![]} }
	pub fn bounds(&self) -> Rect {
		use {vector::MinMax, num::Option};
		self.rects.iter().map(|r| MinMax{min: r.min, max: std::iter::zip(r.min, r.max).map(|(min,max)| if max < i32::MAX as _ { max } else { min }).collect()})
		.chain( self.glyphs.iter().map(|g| MinMax{min: g.top_left, max: g.top_left + rect(g.face.glyph_bounding_box(g.id).unwrap()).size().signed()}) )
		.reduce(MinMax::minmax)
		.map(|MinMax{min, max}| Rect{min: min, max: max})
		.unwrap_or_zero()
	}
}

pub struct View<'t> { graphic: Graphic<'t>, view: Rect }

impl<'t> View<'t> { pub fn new(graphic: Graphic<'t>) -> Self { Self{view: graphic.bounds(), graphic} } }

impl widget::Widget for View<'_> {
    fn size(&mut self, _: size) -> size { ceil(self.graphic.scale, self.view.size()) }
    #[throws] fn paint(&mut self, target: &mut Target, size: size, _offset: int2) {
		let Self{graphic: Graphic{scale, rects, parallelograms, glyphs}, view: Rect{min, ..}} = &self;

		use {num::zero, image::{Image, bgra, sRGB::sRGB8}};
		let buffer = {
			let mut target = Image::fill(target.size, 1.);

			for &Rect{min: top_left, max: bottom_right} in rects {
				let top_left = top_left - min;
				if top_left < (size/scale).signed() {
					let top_left = ifloor(*scale, top_left);
					let bottom_right : int2 = int2::enumerate().map(|i| if bottom_right[i] == i32::MAX { size[i] as _ } else { scale.ifloor(bottom_right[i]-min[i]) }).into();
					target.slice_mut(top_left.unsigned(), (vector::component_wise_min(bottom_right, target.size.signed())-top_left).unsigned()).set(|_| 0.);
					//context.fill(piet::kurbo::Rect::new(top_left.x as _, top_left.y as _, bottom_right.x as f64, bottom_right.y as f64), &piet::Color::BLACK);
				}
			}
			for &Parallelogram{top_left, bottom_right, vertical_thickness: _} in parallelograms {
				let top_left = top_left - min;
				if top_left < (size/scale).signed() {
					let top_left = ifloor(*scale, top_left);
					let bottom_right : int2 = int2::enumerate().map(|i| if bottom_right[i] == i32::MAX as _ { size[i] as _ } else { scale.ifloor(bottom_right[i] as i32 - min[i]) }).into();
					target.slice_mut(top_left.unsigned(), (vector::component_wise_min(bottom_right, target.size.signed())-top_left).unsigned()).set(|_| 0.);
					//context.fill(piet::kurbo::Rect::new(top_left.x as _, top_left.y as _, bottom_right.x as f64, bottom_right.y as f64), &piet::Color::BLACK);
				}
			}
			for &Glyph{top_left, face, id, scale: glyph_scale} in glyphs {
				let scale = *scale*glyph_scale;
				let top_left = top_left - min;
				if top_left < (size/scale).signed() {
					let offset = ifloor(scale, top_left + xy{x: -face.glyph_hor_side_bearing(id).unwrap() as _, y: face.glyph_bounding_box(id).unwrap().y_max as _}).into();
					if offset < target.size {
						let bbox = face.glyph_bounding_box(id).map(rect).unwrap();
						use crate::font::Rasterize;
						let coverage = face.rasterize(self.graphic.scale, id, bbox);
						let size = vector::component_wise_min(coverage.size, target.size-offset);
						target.slice_mut(offset, size).zip_map(coverage.slice(zero(), size), |_, &target, &coverage| target - coverage);
					}
					/*let mut glyph = piet_gpu::encoder::GlyphEncoder::default();
					let mut path_encoder = PathEncoder{scale: f32::from(*scale)*glyph_scale, offset, path_encoder: glyph.path_encoder()};
					if face.outline_glyph(*id, &mut path_encoder).is_some() {
						let mut path_encoder = path_encoder.path_encoder;
						path_encoder.path();
						let n_pathseg = path_encoder.n_pathseg();
						glyph.finish_path(n_pathseg);
						context.encode_glyph(&glyph);
						context.fill_glyph(piet::Color::BLACK.as_rgba_u32());
					}*/
				}
			}
			target
		};
		target.set_map(&buffer, |_, &buffer| bgra{a: 0xFF, ..sRGB8(f32::min(buffer,1.)).into()});
	}
}

pub struct Widget<T>(pub T);
impl<'t, T: Fn(size)->Result<Graphic<'t>>> widget::Widget for Widget<T> {
    fn size(&mut self, size: size) -> size { View::new(self.0(size).unwrap()).size(size) }
    #[throws] fn paint(&mut self, context: &mut Target, size: size, offset: int2) { View::new(self.0(size)?).paint(context, size, offset)? }
}
