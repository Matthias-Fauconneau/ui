use {::xy::{xy, vec2, Rect}, piet_gpu_types::scene::{LineSeg, QuadSeg, CubicSeg}};

pub fn rect(r: ttf_parser::Rect) -> Rect { Rect{min: xy{ x: r.x_min as i32, y: r.y_min as i32}, max: xy{ x: r.x_max as i32, y: r.y_max as i32} } }

pub struct PathEncoder<'t> { pub scale : f32, pub offset: vec2, pub context: &'t mut piet_gpu::PietGpuRenderContext, pub first : [f32; 2], pub p0 : [f32; 2]}
impl PathEncoder<'_> { fn map(&self, x : f32, y : f32) -> [f32; 2] { (self.offset+self.scale*vec2{x,y: -y}).into() } }
impl ttf_parser::OutlineBuilder for PathEncoder<'_> {
	fn move_to(&mut self, x: f32, y: f32) { 
		self.first = self.map(x,y);
		self.p0 = self.first;
	}
	fn line_to(&mut self, x: f32, y: f32) { 
		let p1 = self.map(x,y); 
		self.context.encode_line_seg(LineSeg{p0: self.p0, p1}); 
		self.p0 = p1;
	}
	fn quad_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32) {
		let p1 = self.map(x1, y1);
		let p2 = self.map(x2, y2);
		self.context.encode_quad_seg(QuadSeg{p0: self.p0, p1, p2}); 
		self.p0 = p2;
	}
	fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x3: f32, y3: f32) {
		let p1 = self.map(x1, y1);
		let p2 = self.map(x2, y2);
		let p3 = self.map(x3, y3);
		self.context.encode_cubic_seg(CubicSeg{p0: self.p0, p1, p2, p3}); 
		self.p0 = p3;
	}
	fn close(&mut self) { self.context.encode_line_seg(LineSeg{p0: self.p0, p1: self.first}); }
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
