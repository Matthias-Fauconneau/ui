pub use ::xy::int2;

pub struct Parallelogram { pub top_left: int2, pub bottom_right: int2, pub vertical_thickness: u8 }

impl Parallelogram {
	pub fn translate(&mut self, offset: int2) { self.top_left += offset; self.bottom_right += offset; }
}

pub struct Glyph { pub top_left: int2, pub id: ttf_parser::GlyphId }

impl Glyph {
	pub fn translate(&mut self, offset: int2) { self.top_left += offset; }
}

pub use {core::num::Ratio, xy::Rect, ttf_parser::Face};

pub struct Graphic<'t> {
	pub scale: Ratio,
	pub rects: Vec<Rect>,
	pub parallelograms: Vec<Parallelogram>,
	pub font: &'t Face<'t>,
	pub glyphs: Vec<Glyph>,
}

impl<'t> Graphic<'t> {
	pub fn new(scale: Ratio, font: &'t Face) -> Self {
		Self{scale, rects: Default::default(), parallelograms: Default::default(), font, glyphs: Default::default() }
	}
	pub fn bounds(&self) -> Rect {
	    use core::vector::{Bounds, MinMax};
					self.rects.iter().map(|r| MinMax{min: r.top_left, max: r.top_left.iter().zip(r.bottom_right.iter()).map(|(&o,&s)| if s < i32::MAX { s } else { o }).collect()})
		.chain( self.glyphs.iter().map(|g| MinMax{min: g.top_left, max: g.top_left + self.font.glyph_size(g.id).into()}) )
		.bounds()
		.map(|MinMax{min, max}| Rect{top_left: min, bottom_right: max})
		.unwrap_or_default()
	}
}

pub struct View<'t> { graphic: Graphic<'t>, view: Rect }

use {core::{error::{throws, Error, Result}, num::Zero}, ::xy::{size, uint2}, image::{Image, sRGB, bgra8} crate::{widget::{self, Target}, font::Rasterize}};

impl widget::Widget for View<'_> {
    fn size(&mut self, _: size) -> size { self.graphic.scale * (self.view.bottom_right-self.view.top_left).unsigned() }
    #[throws] fn paint(&mut self, target : &mut Target) {
		let buffer = {
			let mut target = Image::zero(target.size);
			for &Rect{top_left, bottom_right} in &self.graphic.rects {
				let top_left = self.graphic.scale * (top_left-self.view.top_left).unsigned();
				if top_left < target.size {
					let bottom_right = xy(|i| if bottom_right[i] == i32::MAX { target.size[i] } else { self.graphic.scale.ifloor(bottom_right[i]-self.view.top_left[i]) as u32 });
					target.slice_mut(top_left, vector::component_wise_min(bottom_right, target.size)-top_left).set(|_| 1.);
				}
			}
			for &Glyph{top_left, id} in &self.graphic.glyphs {
				let offset = self.graphic.scale * (top_left-self.view.top_left).unsigned();
				if offset < target.size {
					let bbox = self.graphic.font.glyph_bounding_box(id).unwrap();
					let coverage = self.graphic.font.rasterize(self.graphic.scale, id, bbox);
					let size = vector::component_wise_min(coverage.size, target.size-offset);
					target.slice_mut(offset, size).zip_map(coverage.slice(Zero::zero(), size), |_, &target, &coverage| target + coverage);
				}
			}
			target
		};
		target.set_map(buffer, |_, &buffer| bgra8{a: 0xFF, ..sRGB(f32::min(buffer,1.)).into()});
	}
}

impl<'t> View<'t> { pub fn new(graphic: Graphic<'t>) -> Self { Self{view: graphic.bounds(), graphic} } }

fn ceil(scale: &Ratio, v: uint2) -> uint2 { xy{x:scale.ceil(v.x), y:scale.ceil(v.y)} } }

pub struct Widget<T>(pub T);
impl<'t, T:Fn(size)->Result<Graphic<'t>>> widget::Widget for Widget<T> {
    fn size(&mut self, size: size) -> size { let graphic = self.0(size).unwrap(); let view = graphic.bounds(); ceil(graphic.scale, (view.bottom_right-view.top_left).unsigned()) }
    #[throws] fn paint(&mut self, target : &mut Target) { View::new(self.0(target.size)?).paint(target)? }
}
