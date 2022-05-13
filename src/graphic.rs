pub use ::vector::int2;

pub struct Parallelogram { pub top_left: int2, pub bottom_right: int2, pub vertical_thickness: u32 }

impl Parallelogram {
	pub fn translate(&mut self, offset: int2) { self.top_left += offset; self.bottom_right += offset; }
}

pub struct Glyph { pub top_left: int2, pub face: &'static Face<'static>, pub id: ttf_parser::GlyphId, pub scale: f32 }

impl Glyph {
	pub fn translate(&mut self, offset: int2) { self.top_left += offset; }
}

pub use {::num::Ratio, ::vector::Rect, ttf_parser::Face};

pub fn horizontal(y: i32, dy: u32, x0: i32, x1: i32) -> Rect { Rect{ min: xy{ y: y-(dy/2) as i32, x: x0 }, max: xy{ y: y+(dy/2) as i32, x: x1 } } }
pub fn vertical   (x: i32, dx: u32, y0: i32, y1: i32) -> Rect { Rect{ min: xy{ x: x-(dx/2) as i32, y: y0 }, max: xy{ x: x+(dx/2) as i32, y: y1 } } }

pub struct Graphic {
	pub scale: Ratio,
	pub rects: Vec<Rect>,
	pub parallelograms: Vec<Parallelogram>,
	pub glyphs: Vec<Glyph>,
}

use {fehler::throws, crate::{Error, Result, widget::{self, RenderContext, size}, font::{rect, PathEncoder}}, ::vector::{xy, ifloor, ceil}};

impl Graphic {
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

pub struct View { graphic: Graphic, view: Rect }

impl View { pub fn new(graphic: Graphic) -> Self { Self{view: graphic.bounds(), graphic} } }

impl widget::Widget for View {
    fn size(&mut self, _: size) -> size { ceil(self.graphic.scale, self.view.size()) }
    #[throws] fn paint(&mut self, context: &mut RenderContext, size: size, _offset: int2) {
		let Self{graphic: Graphic{scale, rects, parallelograms, glyphs}, view: Rect{min, ..}} = &self;
		use piet::RenderContext;
		for &Rect{min: top_left, max: bottom_right} in rects {
			let top_left = top_left - min;
			if top_left < (size/scale).signed() {
				let top_left = ifloor(*scale, top_left);
				let bottom_right : int2 = int2::enumerate().map(|i| if bottom_right[i] == i32::MAX { size[i] as _ } else { scale.ifloor(bottom_right[i]-min[i]) }).into();
				context.fill(piet::kurbo::Rect::new(top_left.x as _, top_left.y as _, bottom_right.x as f64, bottom_right.y as f64), &piet::Color::BLACK);
			}
		}
		for &Parallelogram{top_left, bottom_right, vertical_thickness: _} in parallelograms {
			let top_left = top_left - min;
			if top_left < (size/scale).signed() {
				let top_left = ifloor(*scale, top_left);
				let bottom_right : int2 = int2::enumerate().map(|i| if bottom_right[i] == i32::MAX as _ { size[i] as _ } else { scale.ifloor(bottom_right[i] as i32 - min[i]) }).into();
				context.fill(piet::kurbo::Rect::new(top_left.x as _, top_left.y as _, bottom_right.x as f64, bottom_right.y as f64), &piet::Color::BLACK);
			}
		}
		for Glyph{top_left, face, id, scale: glyph_scale} in glyphs {
			let top_left = top_left - min;
			if top_left < (size/scale).signed() {
				let offset = ifloor(*scale, top_left + xy{x: -face.glyph_hor_side_bearing(*id).unwrap() as _, y: face.glyph_bounding_box(*id).unwrap().y_max as _}).into();
				let mut encoder = piet_gpu::encoder::GlyphEncoder::default();
				let mut path_encoder = PathEncoder{scale: f32::from(*scale)*glyph_scale, offset, path_encoder: encoder.path_encoder()};
				if face.outline_glyph(*id, &mut path_encoder).is_some() {
					let mut path_encoder = path_encoder.path_encoder;
					path_encoder.path();
					let n_pathseg = path_encoder.n_pathseg();
    				encoder.finish_path(n_pathseg);
					use piet::IntoBrush;
					let brush = piet::Color::BLACK.make_brush(context, || unreachable!());
					context.encode_brush(&brush);
				}
			}
		}
	}
}

pub struct Widget(pub Box<dyn Fn(size)->Result<Graphic>>);
impl widget::Widget for Widget {
    fn size(&mut self, size: size) -> size { View::new(self.0(size).unwrap()).size(size) }
    #[throws] fn paint(&mut self, context: &mut RenderContext, size: size, offset: int2) { View::new(self.0(size)?).paint(context, size, offset)? }
}
