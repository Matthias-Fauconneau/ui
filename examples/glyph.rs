use {ui::Result, num::Ratio, vector::{xy,size,int2,Rect}, ui::{font, foreground, Target,Widget, run}};
struct Glyph<'t> {
	face: &'t ttf_parser::Face<'t>,
	id: ttf_parser::GlyphId,
	bbox: Rect,
}
impl Widget for Glyph<'_> {
	fn size(&mut self, fit: size) -> size {
		let glyph = self.bbox.size();
		let scale = if fit.x*glyph.y < fit.y*glyph.x { Ratio{num: fit.x, div: glyph.x} } else { Ratio{num: fit.y, div: glyph.y} };
		ui::font::Rasterize::glyph_scaled_size(self.face, scale, self.id)
	}
	fn paint(&mut self, target: &mut Target, size: size, _: int2) -> Result {
		let ref coverage = ui::font::Rasterize::rasterize(self.face, size.zip(self.bbox.size()).map(|(size,bbox)| Ratio{num:size-1, div: bbox}).into_iter().min().unwrap(), self.id, self.bbox);
		image::blend(&coverage.as_ref(), &mut target.slice_mut(xy{x:0,y:0}, coverage.size), foreground);
		Ok(())
	}
}
fn main() -> Result {
	let ref face = font::open(std::path::Path::new("/usr/share/fonts/OTF/Bravura.otf")).unwrap();
	//pub const G : char = '\u{E050}';
	pub const _2 : char = '\u{E082}';
	let id = face.glyph_index(_2).unwrap();
	let bbox = font::rect(face.glyph_bounding_box(id).unwrap());
	run("glyph", &mut Glyph{face, id, bbox})
}

