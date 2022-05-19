use vector::{xy, vec2, Rect};

pub fn rect(r: ttf_parser::Rect) -> Rect { Rect{min: xy{ x: r.x_min as i32, y: r.y_min as i32}, max: xy{ x: r.x_max as i32, y: r.y_max as i32} } }

pub struct PathEncoder<'t> { pub scale : f32, pub offset: vec2, pub path_encoder: piet_gpu::stages::PathEncoder<'t> }
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
}

use {fehler::throws, super::Error};
#[derive(derive_more::Deref)] pub struct Handle<'t>(ttf_parser::Face<'t>);
pub type File<'t> = owning_ref::OwningHandle<Box<memmap::Mmap>, Handle<'t>>;
#[throws] pub fn open<'t>(path: &std::path::Path) -> File<'t> {
	owning_ref::OwningHandle::new_with_fn(
		Box::new(unsafe{memmap::Mmap::map(&std::fs::File::open(path)?)}?),
		unsafe { |map| Handle(ttf_parser::Face::from_slice(&*map, 0).unwrap()) }
	)
}
