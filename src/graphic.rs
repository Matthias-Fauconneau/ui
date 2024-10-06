pub use vector::int2;

pub struct Parallelogram { pub top_left: int2, pub bottom_right: int2, pub descending : bool, pub vertical_thickness: u32 }

impl Parallelogram {
	pub fn translate(&mut self, offset: int2) { self.top_left += offset; self.bottom_right += offset; }
}

pub use {num::Ratio, vector::Rect, crate::font::{Face, GlyphId}};

pub struct Glyph<'t> { pub top_left: int2, pub face: &'t Face<'t>, pub id: GlyphId, pub scale: Ratio, pub style: f32 }

impl Glyph<'_> {
	pub fn translate(&mut self, offset: int2) { self.top_left += offset; }
}

pub fn horizontal(y: i32, dy: u32, x0: i32, x1: i32) -> Rect { Rect{ min: xy{ y: y-(dy/2) as i32, x: x0 }, max: xy{ y: y+(dy/2) as i32, x: x1 } } }
pub fn vertical   (x: i32, dx: u32, y0: i32, y1: i32) -> Rect { Rect{ min: xy{ x: x-(dx/2) as i32, y: y0 }, max: xy{ x: x+(dx/2) as i32, y: y1 } } }

pub struct Graphic<'t> {
	pub scale: Ratio,
	pub rects: Vec<(Rect, f32)>,
	parallelograms: Vec<Parallelogram>,
	pub glyphs: Vec<Glyph<'t>>,
}

use {num::zero, crate::{throws, Error, Result}, vector::{xy, size, vec2, ifloor, ceil}, image::{Image, bgr, /*PQ10*/sRGB8}, crate::{font::rasterize, widget, Target, dark}};

impl Graphic<'_> {
	pub fn new(scale: Ratio) -> Self { Self{scale, rects: Vec::new(), parallelograms: Vec::new(), glyphs: Vec::new()} }
	pub fn extend(&mut self, mut graphic: Self, position: int2) {
		self.rects.extend(graphic.rects.drain(..).map(|(mut x, style)| { x.translate(position); (x, style) }));
		self.parallelograms.extend(graphic.parallelograms.drain(..).map(|mut x| { x.translate(position); x }));
		self.glyphs.extend(graphic.glyphs.drain(..).map(|mut x| { x.translate(position); x }));
	}
	pub fn bounds(&self) -> Rect {
		use {vector::MinMax, num::Option};
		self.rects.iter().map(|(r,_)| MinMax{min: r.min, max: r.min.zip(r.max).map(|(min,max)| if max < i32::MAX as _ { max } else { min }).collect()})
		.chain( self.parallelograms.iter().map(|p| MinMax{min: p.top_left, max: p.bottom_right}) )
		.chain( self.glyphs.iter().map(|g| MinMax{min: g.top_left, max: g.top_left + g.face.bbox(g.id).unwrap().size()}) )
		.reduce(MinMax::minmax)
		.map(|MinMax{min, max}| Rect{min: min, max: max})
		.unwrap_or_zero()
	}

	#[track_caller] pub fn rect(&mut self, r: Rect, style: f32) { assert!(r.min <= r.max); self.rects.push((r, style)) }
	pub fn horizontal(&mut self, y: i32, dy: u32, x0: i32, x1: i32, style: f32) { self.rect(horizontal(y,dy,x0,x1), style) }
	pub fn vertical(&mut self, x: i32, dx: u32, y0: i32, y1: i32, style: f32) { self.rect(vertical(x,dx,y0,y1), style) }

	#[track_caller] pub fn parallelogram(&mut self, p: Parallelogram) { assert!(p.top_left <= p.bottom_right); self.parallelograms.push(p) }
}

pub struct View<'t> { graphic: Graphic<'t>, view: Rect }

impl<'t> View<'t> { pub fn new(graphic: Graphic<'t>) -> Self { Self{view: graphic.bounds(), graphic} } }

impl widget::Widget for View<'_> {
    fn size(&mut self, _: size) -> size { ceil(self.graphic.scale, self.view.size().unsigned()) }
    #[throws] fn paint(&mut self, target: &mut Target, size: size, _offset: int2) {
		let Self{graphic: Graphic{scale, rects, parallelograms, glyphs}, view: Rect{min, ..}} = &self;

		let buffer = {
			assert!(target.size == size);
			let mut target = Image::fill(size, 0.);

			for &(Rect{min: top_left, max: bottom_right}, style) in rects {
				let top_left = top_left - min;
				if top_left < (size/scale).signed() {
					let top_left = ifloor(*scale, top_left);
					let bottom_right : int2 = int2::enumerate().map(|i| if bottom_right[i] == i32::MAX { size[i] as _ } else { scale.ifloor(bottom_right[i]-min[i]) }).into();
					target.slice_mut(top_left.unsigned(), (vector::component_wise_min(bottom_right, size.signed())-top_left).unsigned()).set(|_| style);
				}
			}
			for &Parallelogram{top_left, bottom_right, descending, vertical_thickness} in parallelograms {
				let top_left = top_left - min;
				if top_left < (size/scale).signed() {
					let scale = f32::from(*scale);
					crate::parallelogram(&mut target.as_mut(), scale*vec2::from(top_left), scale*vec2::from(bottom_right-min), descending, scale*vertical_thickness as f32, 1.);
					//parallelogram(target, scale*vec2::from(top_left), scale*vec2::from(bottom_right.signed()-min) )
					//target.slice_mut(top_left.unsigned(), (vector::component_wise_min(bottom_right, size.signed())-top_left).unsigned()).set(|_| foreground.g);
				}
			}
			for &Glyph{top_left, face, id, scale: glyph_scale, style} in glyphs {
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
							|&target, &coverage| target + coverage*style
						);
					}
				}
			}
			target
		};
		target.zip_map(&buffer, |_, &buffer| { let c = f32::min(1.,buffer); bgr::from(/*PQ10*/sRGB8(if dark {c} else {1.-c})).into()});
	}
}

pub struct Widget<T>(pub T);
impl<'t, T: Fn(size)->Result<Graphic<'t>>> widget::Widget for Widget<T> {
    fn size(&mut self, size: size) -> size { View::new(self.0(size).unwrap()).size(size) }
    fn paint(&mut self, context: &mut Target, size: size, offset: int2) -> Result { View::new(self.0(size)?).paint(context, size, offset) }
}
