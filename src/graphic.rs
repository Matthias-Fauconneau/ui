use crate::{error::{throws, Error}, num::Ratio, vector::{self, xy, int2, size2}, font::{self, Font}};

impl std::ops::Mul<int2> for Ratio { type Output=int2; #[track_caller] fn mul(self, b: int2) -> Self::Output { int2{x:self*b.x, y:self*b.y} } }

#[derive(Default)] pub struct Rect { pub top_left: int2, pub size: size2 }

pub struct Glyph { pub top_left: int2, pub id: font::GlyphId }

pub struct Graphic<'t> {
	pub fill: Vec<Rect>,
	pub font: &'t Font<'t>,
	pub scale: Ratio,
	pub glyph: Vec<Glyph>,
}

trait Bounds : Iterator { fn bounds(self) -> Option<Self::Item>; }
impl<T, I:Iterator<Item=(T,T)>> Bounds for I where T: vector::ComponentWiseMinMax+Copy {
	fn bounds(self) -> Option<Self::Item> { self.fold_first(|(min,max), e| (vector::component_wise_min(min, e.0), vector::component_wise_max(max, e.1))) }
}

impl Graphic<'_> {
	pub fn bounds(&self) -> Rect {
		self.fill.iter().map(|r| (r.top_left, r.top_left.iter().zip(r.size.iter()).map(|(o,&s)| o + if s < i32::MAX as u32 { s as i32 } else { 0 }).collect()))
		.chain( self.glyph.iter().map(|g| (g.top_left, g.top_left + {
				let b = self.font.glyph_bounding_box(g.id).unwrap();
				xy{x: self.scale.ceil(b.x_max as i32) - self.scale.floor(b.x_min as i32), y: self.scale.ceil(b.y_max as i32) - self.scale.floor(b.y_min as i32)}
		})))
		.bounds()
		.map(|(top_left, bottom_right)| Rect{top_left, size: (bottom_right-top_left).into()})
		.unwrap_or_default()
	}
}

pub struct GraphicView<'t> { graphic: Graphic<'t>, view: Rect }

use crate::{widget::{Target, Widget, fg}, image::{bgra8, sRGB}};

impl Widget for GraphicView<'_> {
    fn size(&mut self, size: size2) -> size2 { dbg!(self.view.size.iter().zip(size.iter()).map(|(&v,&s)| if v > 0 { v } else { s }).collect()) }
    #[throws] fn paint(&mut self, target : &mut Target) {
		for &Rect{top_left, size} in &self.graphic.fill {
			let offset = (top_left-self.view.top_left).into();
			target.slice_mut(offset, vector::component_wise_min(size, target.size-offset)).set(|_| fg);
		}
		for &Glyph{top_left, id} in &self.graphic.glyph {
			let bbox = self.graphic.font.glyph_bounding_box(id).unwrap();
			let coverage = self.graphic.font.rasterize(self.graphic.scale, id, bbox);
			target.slice_mut((top_left-self.view.top_left).into(), coverage.size).set_map(coverage, |_,coverage| bgra8{a : 0xFF, ..sRGB(coverage).into()});
		}
	}
}

impl<'t> GraphicView<'t> { pub fn new(graphic: Graphic<'t>) -> Self { Self{view: graphic.bounds(), graphic} } }
