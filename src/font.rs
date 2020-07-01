use {derive_more::Deref, crate::{num::Ratio, vector::size2}};
#[derive(Deref)] pub struct Font<'t>(ttf_parser::Font<'t>);
impl<'t> Font<'t> {
	pub fn size(&self, id: ttf_parser::GlyphId) -> size2 {
		let b = self.glyph_bounding_box(id).unwrap();
		size2{x: (b.x_max as i32 - b.x_min as i32) as u32, y: (b.y_max as i32 - b.y_min as i32) as u32}
	}
	pub fn scaled_size(&self, scale: Ratio, id: ttf_parser::GlyphId) -> size2 {
		let b = self.glyph_bounding_box(id).unwrap();
		size2{x: (scale.ceil(b.x_max as i32) - scale.floor(b.x_min as i32)) as u32, y: (scale.ceil(b.y_max as i32) - scale.floor(b.y_min as i32)) as u32}
	}
}

use crate::{Image, vector::vec2, quad::quad, cubic::cubic, raster::{line, self}};
struct Outline<'t> { scale : f32, x_min: f32, y_max: f32, target : &'t mut Image<&'t mut[f32]>, first : Option<vec2>, p0 : Option<vec2>}
impl Outline<'_> { fn map(&self, x : f32, y : f32) -> vec2 { vec2{x: self.scale*x-self.x_min, y: -self.scale*y+self.y_max} } }
impl ttf_parser::OutlineBuilder for Outline<'_> {
    fn move_to(&mut self, x: f32, y: f32) { assert!(self.first.is_none() && self.p0.is_none()); self.first = Some(self.map(x,y)); self.p0 = self.first; }
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
		let p0 = self.p0.unwrap();
		let p1 = self.map(x1, y1);
        let p2 = self.map(x2, y2);
		let p3 = self.map(x3, y3);
        let mut pp = p0;
        cubic(p0, p1, p2, p3, |p| { line(&mut self.target, pp, p); pp = p; });
        self.p0 = Some(p3);
    }
    fn close(&mut self) { line(&mut self.target, self.p0.unwrap(), self.first.unwrap()); self.first = None; self.p0 = None; }
}

impl<'t> Font<'t> {
    pub fn rasterize(&self, scale: Ratio, id: ttf_parser::GlyphId, bbox: ttf_parser::Rect) -> Image<Vec<f32>> {
        let x_min = scale.floor(bbox.x_min as i32);
        let y_max = scale.ceil(bbox.y_max as i32);
        let mut target = Image::zero(self.scaled_size(scale, id)+size2{x:0,y:1});
        self.outline_glyph(id, &mut Outline{scale: scale.into(), x_min: x_min as f32, y_max: y_max as f32, target: &mut target.as_mut(), first:None, p0:None}).unwrap();
        raster::fill(&target.as_ref())
    }
}

cfg_if::cfg_if! { if #[cfg(all(feature="owning-ref",feature="memmap"))] {
use crate::error::{Error, throws};
#[derive(Deref)]
pub struct Handle<'t>(Font<'t>); // OwningHandle forwards deref, but Font derefs to ttf_parser::Font, while we want OwningHandle to deref to Font not ttf_parser::Font
pub type File<'t> = owning_ref::OwningHandle<Box<memmap::Mmap>, Handle<'t>>;
#[throws] pub fn open(path: &std::path::Path) -> File {
	owning_ref::OwningHandle::new_with_fn(
		Box::new(unsafe{memmap::Mmap::map(&std::fs::File::open(path)?)}?),
		unsafe { |map| Handle(Font(ttf_parser::Font::from_data(&*map, 0).unwrap())) }
	)
}
}}
