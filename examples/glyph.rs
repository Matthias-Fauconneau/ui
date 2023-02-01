use {ui::Result, num::Ratio, vector::{xy,size,int2,Rect}, ui::{font::{self, Face, GlyphId, rasterize}, foreground, Target,Widget, run}};
struct Glyph<'t> {
	face: &'t Face<'t>,
	id: GlyphId,
	bbox: Rect,
}
impl Widget for Glyph<'_> {
	fn size(&mut self, fit: size) -> size {
		let glyph = self.bbox.size();
		let scale = if fit.x*glyph.y < fit.y*glyph.x { Ratio{num: fit.x, div: glyph.x} } else { Ratio{num: fit.y, div: glyph.y} };
		scale*self.face.bbox(self.id).unwrap().size()
	}
	fn paint(&mut self, target: &mut Target, size: size, _: int2) -> Result {
		let ref coverage = rasterize(self.face, size.zip(self.bbox.size()).map(|(size,bbox)| Ratio{num:size-1, div: bbox}).into_iter().min().unwrap(), self.id, self.bbox);
		image::blend(&coverage.as_ref(), &mut target.slice_mut(xy{x:0,y:0}, coverage.size), foreground);
		Ok(())
	}
}
fn main() -> Result {
	let ref face = font::open(std::path::Path::new("/usr/share/fonts/OTF/Bravura.otf")).unwrap();
	//pub const G : char = '\u{E050}';
	pub const _2 : char = '\u{E082}';
	let id = face.glyph_index(_2).unwrap();
	let bbox = face.bbox(id).unwrap();
	run("glyph", &mut Glyph{face, id, bbox})
}

