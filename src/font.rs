use crate::{num::Ratio, vector::{size, xy}, Image, vector::vec2, quad::quad, cubic::cubic, raster::{line, self}};
struct Outline<'t> { scale : Ratio /*f32 loses precision*/, x_min: f32, y_max: f32, target : &'t mut Image<&'t mut[f32]>, first : Option<vec2>, p0 : Option<vec2>}
impl std::ops::Mul<f32> for Ratio { type Output=f32; #[track_caller] fn mul(self, b: f32) -> Self::Output { b * self.num as f32 / self.div as f32 } } // b*(n/d) loses precision
impl Outline<'_> { fn map(&self, x : f32, y : f32) -> vec2 { vec2{x: self.scale*x-self.x_min, y: -(self.scale*y)+self.y_max} } }
impl ttf_parser::OutlineBuilder for Outline<'_> {
    fn move_to(&mut self, x: f32, y: f32) {
		crate::assert!(self.scale*y < self.y_max, y, self.scale*y, self.y_max);
		assert!(self.first.is_none() && self.p0.is_none());
		self.first = Some(self.map(x,y));
		self.p0 = self.first;
	}
    fn line_to(&mut self, x: f32, y: f32) { let p1 = self.map(x,y); line(self.target, self.p0.unwrap(), p1); self.p0 = Some(p1); }
    fn quad_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32) {
		let p0 = self.p0.unwrap();
		let p1 = self.map(x1, y1);
        let p2 = self.map(x2, y2);
        let mut pp = p0;
        quad(p0, p1, p2, |p| { line(&mut self.target, pp, p); pp = p });
        self.p0 = Some(p2);
    }
    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x3: f32, y3: f32) {
		crate::assert!(self.scale*x1 >= self.x_min, self.x_min, x1, self.scale*x1);
		crate::assert!(self.scale*y1 <= self.y_max, self.y_max, y1, self.scale*y1);
		crate::assert!(self.scale*x2 >= self.x_min, self.x_min, x2, self.scale*x2);
		crate::assert!(self.scale*y2 <= self.y_max, self.y_max, y2, self.scale*y2, y2 == y2 as i32 as f32);
		crate::assert!(self.scale*x3 >= self.x_min, self.x_min, x3, self.scale*x3);
		crate::assert!(self.scale*y3 <= self.y_max, self.scale, self.y_max, y3, self.scale*y3);
		let p0 = self.p0.unwrap();
		let p1 = self.map(x1, y1);
        let p2 = self.map(x2, y2);
		let p3 = self.map(x3, y3);
        let mut pp = p0;
        cubic(p0, p1, p2, p3, |p| {
			crate::assert!(p.x >= 0. && p.y >= 0., pp, p, p0, p1, p2, p3);
			line(&mut self.target, pp, p);
			pp = p;
		});
        self.p0 = Some(p3);
    }
    fn close(&mut self) { line(&mut self.target, self.p0.unwrap(), self.first.unwrap()); self.first = None; self.p0 = None; }
}

pub trait Rasterize {
	fn glyph_size(&self, id: ttf_parser::GlyphId) -> size;
	fn glyph_scaled_size(&self, scale: Ratio, id: ttf_parser::GlyphId) -> size;
	fn rasterize(&self, scale: Ratio, id: ttf_parser::GlyphId, bbox: ttf_parser::Rect) -> Image<Vec<f32>>;
}
impl<'t> Rasterize for ttf_parser::Face<'t> {
	fn glyph_size(&self, id: ttf_parser::GlyphId) -> size {
		let b = self.glyph_bounding_box(id).unwrap();
		xy{x: (b.x_max as i32 - b.x_min as i32) as u32, y: (b.y_max as i32 - b.y_min as i32) as u32}
	}
	fn glyph_scaled_size(&self, scale: Ratio, id: ttf_parser::GlyphId) -> size {
		let b = self.glyph_bounding_box(id).unwrap();
		xy{x: (scale.iceil(b.x_max as i32) - scale.ifloor(b.x_min as i32)) as u32, y: (scale.iceil(b.y_max as i32) - scale.ifloor(b.y_min as i32)) as u32}
	}
    fn rasterize(&self, scale: Ratio, id: ttf_parser::GlyphId, bbox: ttf_parser::Rect) -> Image<Vec<f32>> {
		let x_min = scale.ifloor(bbox.x_min as i32)-1; // Correct rasterization with f32 roundoff without bound checking
        let y_max = scale.iceil(bbox.y_max as i32);
        let mut target = Image::zero(self.glyph_scaled_size(scale, id)+xy{x:1, y:1/*2*/});
        self.outline_glyph(id, &mut Outline{scale: scale.into(), x_min: x_min as f32, y_max: y_max as f32, target: &mut target.as_mut(), first:None, p0:None}).unwrap();
        raster::fill(&target.as_ref())
    }
}

cfg_if::cfg_if! { if #[cfg(all(feature="owning-ref",feature="memmap"))] {
use crate::error::{Error, throws};
#[derive(derive_more::Deref)] pub struct Handle<'t>(ttf_parser::Face<'t>); // impl Deref for File
pub type File<'t> = owning_ref::OwningHandle<Box<memmap::Mmap>, Handle<'t>>;
#[throws] pub fn open(path: &std::path::Path) -> File {
	owning_ref::OwningHandle::new_with_fn(
		Box::new(unsafe{memmap::Mmap::map(&std::fs::File::open(path)?)}?),
		//unsafe { |map| ttf_parser::Font::from_data(&*map, 0).unwrap() }
		unsafe { |map| Handle(ttf_parser::Face::from_slice(&*map, 0).unwrap()) }
		//unsafe { |map| Handle(Font(ttf_parser::Font::from_data(&*map, 0).unwrap())) }
	)
}
}}
