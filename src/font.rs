#[derive(derive_more::Deref)] pub struct Face<'t>(rustybuzz::Face<'t>);
pub use rustybuzz::ttf_parser::{self, GlyphId};
use vector::{xy, vec2, Rect};
impl<'t> Face<'t> {
	pub fn advance(&self, codepoint: char) -> u32 { self.glyph_hor_advance(self.glyph_index(codepoint).unwrap()).unwrap() as u32 }
	pub fn bbox(&self, id: GlyphId) -> Option<Rect> {
		let r = self.glyph_bounding_box(id)?;
		Some(Rect{min:xy{x: r.x_min.into(), y: r.y_min.into()}, max: xy{x: r.x_max.into(), y: r.y_max.into()}})
	}
}
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

pub fn rasterize(face: &Face, scale: Ratio, id: GlyphId, bbox: Rect) -> Image<Box<[f32]>> {
	let x_min = scale.ifloor(bbox.min.x)-1; // Correct rasterization with f32 roundoff without bound checking
	let y_max = scale.iceil(bbox.max.y as i32);
	let size = scale*face.bbox(id).unwrap().size()+xy{x:2, y:1};
	let mut target = Image::new(size, vec![0.; (size.y*size.x+1) as usize]);
	face.outline_glyph(id, &mut Outline{scale: scale.into(), x_min: x_min as f32, y_max: y_max as f32, target: &mut target.as_mut(), first:None, p0:None}).unwrap();
	raster::fill(&target.as_ref())
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

#[cfg(target_os="linux")] mod memory_map {
	pub struct MemoryMap{ ptr: *mut core::ffi::c_void, len: usize }
	impl MemoryMap {
		pub fn map<Fd: std::os::fd::AsFd>(fd: Fd) -> rustix::io::Result<Self> {unsafe {
				use rustix::{fs, mm};
				let len = fs::fstat(&fd)?.st_size as usize;
				Ok(Self{ptr: mm::mmap(std::ptr::null_mut(), len, mm::ProtFlags::READ, mm::MapFlags::SHARED, fd, 0)?, len})
		}}
	}
	impl std::ops::Deref for MemoryMap { type Target = [u8]; fn deref(&self) -> &Self::Target { unsafe { std::slice::from_raw_parts(self.ptr as *const u8, self.len) } } }
	impl Drop for MemoryMap { fn drop(&mut self) { unsafe { rustix::mm::munmap(self.ptr, self.len).unwrap() } } }
	unsafe impl Sync for MemoryMap {}
	unsafe impl Send for MemoryMap {}
}

#[derive(derive_more::Deref)] pub struct Handle<'t>(Face<'t>);
use {fehler::throws, super::Error};
cfg_if::cfg_if!{if #[cfg(target_os="linux")] {
	use memory_map::MemoryMap;
	pub type File<'t> = owning_ref::OwningHandle<Box<MemoryMap>, Handle<'t>>;
	#[throws] pub fn open<'t>(path: &std::path::Path) -> File<'t> {
		owning_ref::OwningHandle::new_with_fn(
			Box::new(MemoryMap::map(&std::fs::File::open(path)?)?),
			unsafe { |map| Handle(Face(rustybuzz::Face::from_slice(&*map, 0).unwrap())) }
		)
	}
} else {
	pub type File<'t> = owning_ref::OwningHandle<std::sync::Arc<Vec<u8>>, Handle<'t>>;
	#[throws] pub fn open<'t>(file: std::sync::Arc<Vec<u8>>) -> File<'t> {
		owning_ref::OwningHandle::new_with_fn(
			file,
			unsafe { |file| Handle(Face(rustybuzz::Face::from_slice(&*file, 0).unwrap())) }
		)
	}
}}
