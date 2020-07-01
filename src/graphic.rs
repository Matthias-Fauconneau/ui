use crate::{error::{throws, Error}, num::Ratio, vector::{self, xy, int2, size2}, font::Font};

impl std::ops::Mul<size2> for Ratio { type Output=size2; #[track_caller] fn mul(self, b: size2) -> Self::Output { xy{x:self*b.x, y:self*b.y} } }
impl std::ops::Mul<int2> for Ratio { type Output=int2; #[track_caller] fn mul(self, b: int2) -> Self::Output { xy{x:self*b.x, y:self*b.y} } }

#[derive(Default)] pub struct Rect { pub top_left: int2, pub bottom_right: int2 }

pub struct Glyph { pub top_left: int2, pub id: ttf_parser::GlyphId }

pub struct Graphic<'t> {
	pub scale: Ratio,
	pub fill: Vec<Rect>,
	pub font: &'t Font<'t>,
	pub glyph: Vec<Glyph>,
}

trait Bounds : Iterator { fn bounds(self) -> Option<Self::Item>; }
impl<T, I:Iterator<Item=(T,T)>> Bounds for I where T: vector::ComponentWiseMinMax+Copy {
	fn bounds(self) -> Option<Self::Item> { self.fold_first(|(min,max), e| (vector::component_wise_min(min, e.0), vector::component_wise_max(max, e.1))) }
}

impl Graphic<'_> {
	pub fn bounds(&self) -> Rect {
					self.fill.iter().map(|r| (r.top_left, r.top_left.iter().zip(r.bottom_right.iter()).map(|(&o,&s)| if s < i32::MAX { s } else { o }).collect()))
		.chain( self.glyph.iter().map(|g| (g.top_left, g.top_left + self.font.size(g.id).into())) )
		.bounds()
		.map(|(top_left, bottom_right)| Rect{top_left, bottom_right})
		.unwrap_or_default()
	}
}

pub struct GraphicView<'t> { graphic: Graphic<'t>, view: Rect }

use crate::{widget::{Target, Widget, fg}, image::{bgra8, sRGB}};

impl Widget for GraphicView<'_> {
    fn size(&mut self, _: size2) -> size2 { (self.graphic.scale * (self.view.bottom_right-self.view.top_left)).into() }
    #[throws] fn paint(&mut self, target : &mut Target) {
		for &Rect{top_left, bottom_right} in &self.graphic.fill {
			let top_left = (self.graphic.scale * (top_left-self.view.top_left)).into();
			let bottom_right = xy::map(|i| if bottom_right[i] == i32::MAX { target.size[i] } else { self.graphic.scale * (bottom_right[i]-self.view.top_left[i]) as u32 });
			target.slice_mut(top_left, vector::component_wise_min(bottom_right, target.size)-top_left).set(|_| fg);
		}
		for &Glyph{top_left, id} in &self.graphic.glyph {
			let bbox = self.graphic.font.glyph_bounding_box(id).unwrap();
			let coverage = self.graphic.font.rasterize(self.graphic.scale, id, bbox);
			let offset = (self.graphic.scale * (top_left-self.view.top_left)).into();
			if offset < target.size {
				let size = vector::component_wise_min(coverage.size, target.size-offset);
				target.slice_mut(offset, size).set_map(coverage.slice(xy::zero(), size), |_,coverage| bgra8{a : 0xFF, ..sRGB(coverage).into()});
			}
		}
	}
}

impl<'t> GraphicView<'t> { pub fn new(graphic: Graphic<'t>) -> Self { Self{view: graphic.bounds(), graphic} } }
