use crate::{error::{throws, Error, Result}, num::Ratio, vector::{self, uint2, size, int2, Zero}, image::Image, font::Rasterize};
pub use {crate::vector::xy, ttf_parser::Face};

impl std::ops::Mul<uint2> for Ratio { type Output=uint2; #[track_caller] fn mul(self, b: uint2) -> Self::Output { xy{x:self*b.x, y:self*b.y} } }
impl std::ops::Div<Ratio> for uint2{ type Output=uint2; #[track_caller] fn div(self, r: Ratio) -> Self::Output { xy{x:self.x/r, y:self.y/r} } }

#[derive(Default)] pub struct Rect { pub top_left: int2, pub bottom_right: int2 }

impl Rect {
	pub fn horizontal(y: i32, dy: u8, left: i32, right: i32) -> Rect { Self{ top_left: xy{ y: y-(dy/2) as i32, x: left }, bottom_right: xy{ y: y+(dy/2) as i32, x: right     } } }
	pub fn vertical(x: i32, dx: u8, top: i32, bottom: i32) -> Rect { Self{ top_left: xy{ x: x-(dx/2) as i32, y: top }, bottom_right: xy{ x: x+(dx/2) as i32, y: bottom } } }
}

impl Rect {
	pub fn translate(&mut self, offset: int2) { self.top_left += offset; self.bottom_right += offset; }
}


pub struct Parallelogram { pub top_left: int2, pub bottom_right: int2, pub vertical_thickness: u8 }

impl Parallelogram {
	pub fn translate(&mut self, offset: int2) { self.top_left += offset; self.bottom_right += offset; }
}


pub struct Glyph { pub top_left: int2, pub id: ttf_parser::GlyphId }

impl Glyph {
	pub fn translate(&mut self, offset: int2) { self.top_left += offset; }
}

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
	    use vector::Bounds;
					self.rects.iter().map(|r| vector::MinMax{min: r.top_left, max: r.top_left.iter().zip(r.bottom_right.iter()).map(|(&o,&s)| if s < i32::MAX { s } else { o }).collect()})
		.chain( self.glyphs.iter().map(|g| vector::MinMax{min: g.top_left, max: g.top_left + self.font.glyph_size(g.id).into()}) )
		.bounds()
		.map(|vector::MinMax{min, max}| Rect{top_left: min, bottom_right: max})
		.unwrap_or_default()
	}
}

pub struct View<'t> { graphic: Graphic<'t>, view: Rect }

use crate::{widget::{self, Target}, image::{bgra8, sRGB}};

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

impl Ratio { #[track_caller] fn ceil2(&self, v: uint2) -> uint2 { xy{x:self.ceil(v.x), y:self.ceil(v.y)} } }

pub struct Widget<T>(pub T);
impl<'t, T:Fn(size)->Result<Graphic<'t>>> widget::Widget for Widget<T> {
    fn size(&mut self, size: size) -> size { let graphic = self.0(size).unwrap(); let view = graphic.bounds(); graphic.scale.ceil2((view.bottom_right-view.top_left).unsigned()) }
    #[throws] fn paint(&mut self, target : &mut Target) { View::new(self.0(target.size)?).paint(target)? }
}
