use vector::{xy, vec2, Rect};

pub fn rect(r: ttf_parser::Rect) -> Rect { Rect{min: xy{ x: r.x_min as i32, y: r.y_min as i32}, max: xy{ x: r.x_max as i32, y: r.y_max as i32} } }

mod quad; mod cubic; mod raster;
use {num::Ratio, image::Image, quad::quad, cubic::cubic, raster::line};

struct Outline<'t> { scale : Ratio /*f32 loses precision*/, x_min: f32, y_max: f32, target : &'t mut Image<&'t mut[f32]>, first : Option<vec2>, p0 : Option<vec2>}
impl Outline<'_> { fn map(&self, x : f32, y : f32) -> vec2 { vec2{x: self.scale*x-self.x_min, y: -(self.scale*y)+self.y_max} } }
impl ttf_parser::OutlineBuilder for Outline<'_> {
	fn move_to(&mut self, x: f32, y: f32) {
		let p0 = self.map(x,y);
		self.p0 = Some(p0);
		self.first = Some(p0);
	}
	fn line_to(&mut self, x: f32, y: f32) {
		let p0 = self.p0.unwrap();
		let p1 = self.map(x,y);
		line(self.target, p0, p1); self.p0 = Some(p1);
	}
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

use {ttf_parser::GlyphId, vector::size};
pub trait Rasterize {
	fn glyph_size(&self, id: GlyphId) -> size;
	fn glyph_scaled_size(&self, scale: Ratio, id: GlyphId) -> size;
	fn rasterize(&self, scale: Ratio, id: GlyphId, bbox: Rect) -> Image<Box<[f32]>>;
}
use /*ttf_parser*/rustybuzz::Face;
impl<'t> Rasterize for Face<'t> {
	fn glyph_size(&self, id: ttf_parser::GlyphId) -> size {
		let b = self.glyph_bounding_box(id).unwrap();
		xy{x: (b.x_max as i32 - b.x_min as i32) as u32, y: (b.y_max as i32 - b.y_min as i32) as u32}
	}
	fn glyph_scaled_size(&self, scale: Ratio, id: ttf_parser::GlyphId) -> size {
		let b = self.glyph_bounding_box(id).unwrap();
		xy{x: (scale.iceil(b.x_max as i32) - scale.ifloor(b.x_min as i32)) as u32, y: (scale.iceil(b.y_max as i32) - scale.ifloor(b.y_min as i32)) as u32}
	}
	fn rasterize(&self, scale: Ratio, id: ttf_parser::GlyphId, bbox: Rect) -> Image<Box<[f32]>> {
		let x_min = scale.ifloor(bbox.min.x)-1; // Correct rasterization with f32 roundoff without bound checking
		let y_max = scale.iceil(bbox.max.y as i32);
		let mut target = Image::zero(self.glyph_scaled_size(scale, id)+xy{x:1, y:1});
		self.outline_glyph(id, &mut Outline{scale: scale.into(), x_min: x_min as f32, y_max: y_max as f32, target: &mut target.as_mut(), first:None, p0:None}).unwrap();
		raster::fill(&target.as_ref())
	}
}

/*pub struct PathEncoder<'t> { pub scale : f32, pub offset: vec2, pub path_encoder: piet_gpu::stages::PathEncoder<'t> }
impl PathEncoder<'_> { fn map(&self, x : f32, y : f32) -> [f32; 2] { (self.offset+self.scale*vec2{x,y: -y}).into() } }
impl ttf_parser::OutlineBuilder for PathEncoder<'_> {
	fn move_to(&mut self, x: f32, y: f32) {
		let [x,y] = self.map(x,y);
		self.path_encoder.move_to(x, y)
	}
	fn line_to(&mut self, x: f32, y: f32) {
		let [x,y] = self.map(x,y);
		self.path_encoder.line_to(x, y)
	}
	fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
		let [[x1,y1], [x,y]] = [self.map(x1,y1), self.map(x,y)];
		self.path_encoder.quad_to(x1,y1, x,y)
	}
	fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x3: f32, y3: f32) {
		 let [[x1,y1], [x2,y2], [x3,y3]] = [self.map(x1,y1), self.map(x2,y2), self.map(x3,y3)];
		self.path_encoder.cubic_to(x1,y1, x2,y2, x3,y3)
	}
	fn close(&mut self) { self.path_encoder.close_path(); }
}*/

use {fehler::throws, super::Error};
#[derive(derive_more::Deref)] pub struct Handle<'t>(Face<'t>);
pub type File<'t> = owning_ref::OwningHandle<Box<memmap::Mmap>, Handle<'t>>;
#[throws] pub fn open<'t>(path: &std::path::Path) -> File<'t> {
	owning_ref::OwningHandle::new_with_fn(
		Box::new(unsafe{memmap::Mmap::map(&std::fs::File::open(path)?)}?),
		unsafe { |map| Handle(Face::from_slice(&*map, 0).unwrap()) }
	)
}
