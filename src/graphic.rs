pub use vector::int2;

pub struct Parallelogram { pub top_left: int2, pub bottom_right: int2, pub vertical_thickness: u32 }

impl Parallelogram {
	pub fn translate(&mut self, offset: int2) { self.top_left += offset; self.bottom_right += offset; }
}

pub use {num::Ratio, vector::Rect, crate::font::{Face, GlyphId}};

pub struct Glyph<'t> { pub top_left: int2, pub face: &'t Face<'t>, pub id: GlyphId, pub scale: Ratio }

impl Glyph<'_> {
	pub fn translate(&mut self, offset: int2) { self.top_left += offset; }
}

pub fn horizontal(y: i32, dy: u32, x0: i32, x1: i32) -> Rect { Rect{ min: xy{ y: y-(dy/2) as i32, x: x0 }, max: xy{ y: y+(dy/2) as i32, x: x1 } } }
pub fn vertical   (x: i32, dx: u32, y0: i32, y1: i32) -> Rect { Rect{ min: xy{ x: x-(dx/2) as i32, y: y0 }, max: xy{ x: x+(dx/2) as i32, y: y1 } } }

pub struct Graphic<'t> {
	pub scale: Ratio,
	pub rects: Vec<Rect>,
	parallelograms: Vec<Parallelogram>,
	pub glyphs: Vec<Glyph<'t>>,
}

use {crate::{throws, Error, Result}, vector::{xy, ifloor, ceil}, crate::{font::rasterize, widget::{self, Target, size}}};

impl Graphic<'_> {
	pub fn new(scale: Ratio) -> Self { Self{scale, rects: vec![], parallelograms: vec![], glyphs: vec![]} }
	pub fn extend(&mut self, mut graphic: Self, position: int2) {
		self.rects.extend(graphic.rects.drain(..).map(|mut x| { x.translate(position); x }));
		self.parallelograms.extend(graphic.parallelograms.drain(..).map(|mut x| { x.translate(position); x }));
		self.glyphs.extend(graphic.glyphs.drain(..).map(|mut x| { x.translate(position); x }));
	}
	pub fn bounds(&self) -> Rect {
		use {vector::MinMax, num::Option};
		self.rects.iter().map(|r| MinMax{min: r.min, max: r.min.zip(r.max).map(|(min,max)| if max < i32::MAX as _ { max } else { min })})
		.chain( self.parallelograms.iter().map(|p| MinMax{min: p.top_left, max: p.bottom_right}) )
		.chain( self.glyphs.iter().map(|g| MinMax{min: g.top_left, max: g.top_left + g.face.bbox(g.id).unwrap().size().signed()}) )
		.reduce(MinMax::minmax)
		.map(|MinMax{min, max}| Rect{min: min, max: max})
		.unwrap_or_zero()
	}

	pub fn rect(&mut self, rect: Rect) { self.rects.push(rect) }
	pub fn horizontal(&mut self, y: i32, dy: u32, x0: i32, x1: i32) { self.rects.push(horizontal(y,dy,x0,x1)) }

	#[track_caller] pub fn parallelogram(&mut self, p: Parallelogram) {
		assert!(p.top_left <= p.bottom_right);
		self.parallelograms.push(p)
	}
}

pub struct View<'t> { graphic: Graphic<'t>, view: Rect }

impl<'t> View<'t> { pub fn new(graphic: Graphic<'t>) -> Self { Self{view: graphic.bounds(), graphic} } }

impl widget::Widget for View<'_> {
    fn size(&mut self, _: size) -> size { ceil(self.graphic.scale, self.view.size()) }
    #[throws] fn paint(&mut self, target: &mut Target, size: size, _offset: int2) {
		let Self{graphic: Graphic{scale, rects, parallelograms, glyphs}, view: Rect{min, ..}} = &self;

		use {num::zero, image::{Image, bgr, PQ10}, crate::{background,foreground}};
		let buffer = {
			assert!(target.size == size);
			let mut target = Image::fill(size, background.g);

			for &Rect{min: top_left, max: bottom_right} in rects {
				let top_left = top_left - min;
				if top_left < (size/scale).signed() {
					let top_left = ifloor(*scale, top_left);
					let bottom_right : int2 = int2::enumerate().map(|i| if bottom_right[i] == i32::MAX { size[i] as _ } else { scale.ifloor(bottom_right[i]-min[i]) }).into();
					target.slice_mut(top_left.unsigned(), (vector::component_wise_min(bottom_right, size.signed())-top_left).unsigned()).set(|_| foreground.g);
					//context.fill(piet::kurbo::Rect::new(top_left.x as _, top_left.y as _, bottom_right.x as f64, bottom_right.y as f64), &piet::Color::BLACK);
				}
			}
			for &Parallelogram{top_left, bottom_right, vertical_thickness: _} in parallelograms {
				let top_left = top_left - min;
				if top_left < (size/scale).signed() {
					let top_left = ifloor(*scale, top_left);
					let bottom_right : int2 = int2::enumerate().map(|i| if bottom_right[i] == i32::MAX as _ { size[i] as _ } else { scale.ifloor(bottom_right[i] as i32 - min[i]) }).into();
					target.slice_mut(top_left.unsigned(), (vector::component_wise_min(bottom_right, size.signed())-top_left).unsigned()).set(|_| foreground.g);
					//context.fill(piet::kurbo::Rect::new(top_left.x as _, top_left.y as _, bottom_right.x as f64, bottom_right.y as f64), &piet::Color::BLACK);
				}
			}
			for &Glyph{top_left, face, id, scale: glyph_scale} in glyphs {
				let top_left = top_left - min;
				if top_left < (size/scale).signed() {
					let coverage = rasterize(face, *scale*glyph_scale, id, face.bbox(id).unwrap());
					let offset = *scale*top_left;
					let target_size = size.signed() - offset;
					let target_offset = vector::component_wise_max(zero(), offset).unsigned();
					let source_offset = vector::component_wise_max(zero(), -offset);
					let source_size = coverage.size.signed() - source_offset;
					let size = vector::component_wise_min(source_size, target_size);
					if size.x > 0 && size.y > 0 {
						let size = size.unsigned();
						target.slice_mut(target_offset, size).zip_map(&coverage.slice(source_offset.unsigned(), size),
							|&target, &coverage| target +/*-*/ coverage
						);
					}
					/*let offset = *scale*(top_left + glyph_scale*int2{x: -face.glyph_hor_side_bearing(id).unwrap() as _, y: face.glyph_bounding_box(id).unwrap().y_max as _}).unsigned();
					let mut glyph = piet_gpu::encoder::GlyphEncoder::default();
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
		target.zip_map(&buffer, |_, &buffer| bgr::from(PQ10(f32::min(1.,buffer))).into());
	}
}

pub struct Widget<T>(pub T);
impl<'t, T: Fn(size)->Result<Graphic<'t>>> widget::Widget for Widget<T> {
    fn size(&mut self, size: size) -> size { View::new(self.0(size).unwrap()).size(size) }
    fn paint(&mut self, context: &mut Target, size: size, offset: int2) -> Result { View::new(self.0(size)?).paint(context, size, offset) }
}
