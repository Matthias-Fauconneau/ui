pub use ::xy::int2;

pub struct Parallelogram { pub top_left: int2, pub bottom_right: int2, pub vertical_thickness: u32 }

impl Parallelogram {
	pub fn translate(&mut self, offset: int2) { self.top_left += offset; self.bottom_right += offset; }
}

pub struct Glyph { pub top_left: int2, pub face: &'static Face<'static>, pub id: ttf_parser::GlyphId }

impl Glyph {
	pub fn translate(&mut self, offset: int2) { self.top_left += offset; }
}

pub use {::num::Ratio, ::xy::Rect, ttf_parser::Face};

pub fn vertical(x: i32, dx: u32, y0: i32, y1: i32) -> Rect { Rect{ min: xy{ x: x-(dx/2) as i32, y: y0 }, max: xy{ x: x+(dx/2) as i32, y: y1 } } }

pub struct Graphic {
	pub scale: Ratio,
	pub rects: Vec<Rect>,
	pub parallelograms: Vec<Parallelogram>,
	pub glyphs: Vec<Glyph>,
}

use {fehler::throws, crate::{Error, Result, widget::{self, RenderContext, size}, font::{rect, PathEncoder}}, ::xy::{xy, Component, ifloor, ceil}, num::zero};

impl Graphic {
	pub fn new(scale: Ratio) -> Self { Self{scale, rects: vec![], parallelograms: vec![], glyphs: vec![]} }
	pub fn bounds(&self) -> Rect {
		use vector::MinMax;
		self.rects.iter().map(|r| MinMax{min: r.min, max: std::iter::zip(r.min, r.max).map(|(min,max)| if max < i32::MAX as _ { max } else { min }).collect()})
		.chain( self.glyphs.iter().map(|g| MinMax{min: g.top_left, max: g.top_left + rect(g.face.glyph_bounding_box(g.id).unwrap()).size().signed()}) )
		.reduce(MinMax::minmax)
		.map(|MinMax{min, max}| Rect{min: min, max: max})
		.unwrap_or_default()
	}
}

pub struct View { graphic: Graphic, view: Rect }

impl View { pub fn new(graphic: Graphic) -> Self { Self{view: graphic.bounds(), graphic} } }

impl widget::Widget for View {
    fn size(&mut self, _: size) -> size { ceil(self.graphic.scale, self.view.size()) }
    #[throws] fn paint(&mut self, context: &mut RenderContext, size: size) {
		let Self{graphic: Graphic{scale, rects, parallelograms, glyphs}, view: Rect{min, ..}} = &self;
		use piet::RenderContext;
		for &Rect{min: top_left, max: bottom_right} in rects {
			let top_left = top_left - min;
			if top_left < (size/scale).signed() {
				let top_left = ifloor(*scale, top_left);
				let bottom_right : int2 = Component::enumerate().map(|i| if bottom_right[i] == i32::MAX { size[i] as _ } else { scale.ifloor(bottom_right[i]-min[i]) }).into();
				context.fill(piet::kurbo::Rect::new(top_left.x as _, top_left.y as _, bottom_right.x as f64, bottom_right.y as f64), &piet::Color::BLACK);
			}
		}
		for &Parallelogram{top_left, bottom_right, vertical_thickness: _} in parallelograms {
			let top_left = top_left - min;
			if top_left < (size/scale).signed() {
				let top_left = ifloor(*scale, top_left);
				let bottom_right : int2 = Component::enumerate().map(|i| if bottom_right[i] == i32::MAX as _ { size[i] as _ } else { scale.ifloor(bottom_right[i] as i32 - min[i]) }).into();
				context.fill(piet::kurbo::Rect::new(top_left.x as _, top_left.y as _, bottom_right.x as f64, bottom_right.y as f64), &piet::Color::BLACK);
			}
		}
		for Glyph{top_left, face, id} in glyphs {
			let top_left = top_left - min;
			if top_left < (size/scale).signed() {
				let offset = ifloor(*scale, top_left + xy{x: -face.glyph_hor_side_bearing(*id).unwrap() as _, y: face.glyph_bounding_box(*id).unwrap().y_max as _}).into();
				if face.outline_glyph(*id, &mut PathEncoder{scale: (*scale).into(), offset, context, first: zero(), p0: zero()}).is_some() {
					use piet::IntoBrush;
					let brush = piet::Color::BLACK.make_brush(context, || unreachable!());
					context.encode_brush(&brush);
				}
			}
		}
	}
}

pub struct Widget<T>(pub T);
impl<T:Fn(size)->Result<Graphic>> widget::Widget for Widget<T> {
    fn size(&mut self, size: size) -> size { View::new(self.0(size).unwrap()).size(size) }
    #[throws] fn paint(&mut self, context: &mut RenderContext, size: size) { View::new(self.0(size)?).paint(context, size)? }
}
