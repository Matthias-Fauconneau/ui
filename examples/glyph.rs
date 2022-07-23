use {num::Ratio, vector::{size,Rect}};
struct Glyph<'t> {
	face: &'t ttf_parser::Face<'t>,
	id: ttf_parser::GlyphId,
	bbox: Rect,
}
impl ui::Widget for Glyph<'_> {
	fn size(&mut self, fit: size) -> size {
		let glyph = self.bbox.size();
		let scale = if fit.x*glyph.y < fit.y*glyph.x { Ratio{num: fit.x, div: glyph.x} } else { Ratio{num: fit.y, div: glyph.y} };
		//ceil(scale, glyph)
		ui::font::Rasterize::glyph_scaled_size(self.face, scale, self.id)
	}
	#[fehler::throws(ui::Error)] fn paint(&mut self, target: &mut ui::Target, size: ui::size, _: ui::int2) {
		let glyph = ui::font::Rasterize::rasterize(self.face, num::Ratio{num:size.y-1, div: self.bbox.size().y}, self.id, self.bbox);
		target.set_map(&glyph, |_,&v| image::rgb::from(image::sRGB::sRGB8(1.-v)).into());
	}
}
fn main() -> ui::Result {
	let ref face = ui::font::open(std::path::Path::new(&(std::env::var("HOME").unwrap()+"/.local/share/fonts/Bravura.otf"))).unwrap();
	pub const G : char = '\u{E050}';
	let id = face.glyph_index(G).unwrap();
	let bbox = ui::font::rect(face.glyph_bounding_box(id).unwrap());
	ui::run(&mut Glyph{face, id, bbox})
}

