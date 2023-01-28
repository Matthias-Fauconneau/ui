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
		let coverage = ui::font::Rasterize::rasterize(self.face, num::Ratio{num:size.y-1, div: self.bbox.size().y}, self.id, self.bbox);
		//target.slice_mut(xy{x:0,y:0}, glyph.size).zip_map(&glyph, |_,&v| image::bgr::from(image::PQ10(1.-v)).into());
		image::blend(&coverage, &mut target.slice_mut(xy{x:0,y:0}, coverage.size), foreground);
		Ok(())
	}
}
fn main() -> Result {
	let ref face = font::open(std::path::Path::new("/usr/share/fonts/OTF/Bravura.otf")).unwrap();
	pub const G : char = '\u{E050}';
	let id = face.glyph_index(G).unwrap();
	let bbox = font::rect(face.glyph_bounding_box(id).unwrap());
	run("glyph", &mut Glyph{face, id, bbox})
}

