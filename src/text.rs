use {fehler::throws, super::Error, std::{cmp::{min, max}, ops::Range}, vector::{xy, uint2, int2, size, Rect, vec2}, ttf_parser::{Face,GlyphId}, num::{zero, Ratio}, crate::font::{self, rect, PathEncoder}};
pub mod unicode_segmentation;
use self::unicode_segmentation::{GraphemeIndex, UnicodeSegmentation};

#[derive(derive_more::Deref)] pub(crate) struct LineRange<'t> { #[deref] line: &'t str, pub(crate) range: Range<GraphemeIndex> }

pub(crate) fn line_ranges<'t>(text: &'t str) -> impl Iterator<Item=LineRange<'t>> {
	let mut iter = text.grapheme_indices(true).enumerate().peekable();
	std::iter::from_fn(move || {
		let &(start, (byte_start,_)) = iter.peek()?;
		let (end, byte_end) = iter.find(|&(_,(_,c))| c=="\n").map_or((text.len(),text.len()/*fixme*/), |(end,(byte_end,_))| (end, byte_end));
		Some(LineRange{line: &text[byte_start..byte_end], range: start..end})
	})
}

pub type Font<'t> = [&'t Face<'t>; 2];
pub struct Glyph<'t> {pub index: GraphemeIndex, pub x: u32, pub id: GlyphId, face: &'t Face<'t> }
pub fn layout<'t>(font: &'t Font<'t>, iter: impl Iterator<Item=(GraphemeIndex, &'t str)>+'t) -> impl 't+Iterator<Item=Glyph<'t>> {
	iter.scan((None, 0), move |(last_id, x), (index, g)| {
		let c = iter::Single::single(g.chars()).unwrap();
		//let [c] = arrayvec::ArrayVec::from_iter(g.chars()).into_inner().unwrap();
		let (face, id) = font.iter().find_map(|face| face.glyph_index(if c == '\t' { ' ' } else { c }).map(|id| (face, id))).unwrap_or_else(||panic!("Missing glyph for '{c}' {:x?}", c as u32));
		//if let Some(last_id) = *last_id { *x += face.tables().kern.unwrap().subtables.into_iter().next().map_or(0, |x| x.glyphs_kerning(last_id, id).unwrap_or(0) as i32); }
		*last_id = Some(id);
		let next = Glyph{index, x: *x, id, face};
		*x += face.glyph_hor_advance(id)? as u32;
		Some(next)
	})
}

pub(crate) fn bbox<'t>(iter: impl Iterator<Item=Glyph<'t>>) -> impl Iterator<Item=(Rect, Glyph<'t>)> {
	iter.filter_map(move |g| Some((g.face.glyph_bounding_box(g.id).map(rect)?, g)))
}

struct LineMetrics {pub width: u32, pub ascent: i16, pub descent: i16}
fn metrics<'t>(iter: impl Iterator<Item=Glyph<'t>>) -> LineMetrics {
	bbox(iter).fold(LineMetrics{width: 0, ascent: 0, descent: 0}, |metrics: LineMetrics, (bbox, Glyph{x, id, face, ..})| LineMetrics{
		width: (x as i32 + face.glyph_hor_side_bearing(id).unwrap() as i32 + bbox.max.x) as u32,
		ascent: max(metrics.ascent, bbox.max.y as i16),
		descent: min(metrics.descent, bbox.min.y as i16)
	})
}

pub type Color = crate::color::bgrf;
pub use crate::color::bgr;
#[derive(Clone,Copy,Default,Debug)] pub enum FontStyle { #[default] Normal, Bold, /*Italic, BoldItalic*/ }
#[derive(Clone,Copy,Default,Debug)] pub struct Style { pub color: Color, pub style: FontStyle }
pub type TextRange = std::ops::Range<GraphemeIndex>;
#[derive(Clone,derive_more::Deref,Debug)] pub struct Attribute<T> { #[deref] pub range: TextRange, pub attribute: T }
const fn from(color: Color) -> Attribute<Style> { Attribute{range: 0..GraphemeIndex::MAX, attribute: Style{color, style: FontStyle::Normal}} }
impl From<Color> for Attribute<Style> { fn from(color: Color) -> Self { from(color) } }

#[allow(non_upper_case_globals)] pub static default_font_files : std::sync::LazyLock<[font::File<'static>; 2]> = std::sync::LazyLock::new(||
	["/usr/share/fonts/truetype/noto/NotoSans-Regular.ttf","/usr/share/fonts/truetype/noto/NotoSansSymbols-Regular.ttf"].map(|p| font::open(std::path::Path::new(p)).unwrap()));
pub fn default_font() -> Font<'static> { default_font_files.each_ref().map(|x| std::ops::Deref::deref(x)) }
#[allow(non_upper_case_globals)]
pub const default_style: [Attribute::<Style>; 1] = [from(Color{b:1.,r:1.,g:1.})];

pub struct View<'t, D> {
    pub font : Font<'t>,
	pub data: D,
    pub size : Option<size>
}

impl<'t, D> View<'t, D> {
	pub fn new(data: D) -> Self { Self{font: default_font(), data, size: None} }
	pub fn new_with_face(face : &'t Face<'t>, data: D) -> Self { Self{font: [&face, &face], data, size: None} }
}

use num::{IsZero, div_ceil};
pub fn fit_width(width: u32, from : size) -> size { if from.is_zero() { return zero(); } xy{x: width, y: div_ceil(width * from.y, from.x)} }
pub fn fit_height(height: u32, from : size) -> size { if from.is_zero() { return zero(); } xy{x: div_ceil(height * from.x, from.y), y: height} }
pub fn fit(size: size, from: size) -> size { if size.x*from.y < size.y*from.x { fit_width(size.x, from) } else { fit_height(size.y, from) } }

impl<D:AsRef<str>> View<'_, D> {
	pub fn size(&mut self) -> size {
		let Self{font, data, ref mut size, ..} = self;
		*size.get_or_insert_with(||{
			let text = data.as_ref();
			let (line_count, max_width) = line_ranges(&text).fold((0,0),|(line_count, width), line| (line_count+1, max(width, metrics(layout(font, line.graphemes(true).enumerate())).width)));
			xy{x: max_width, y: line_count * (font[0].height() as u32)}
		})
	}
	pub fn size_scale(&mut self, fit: size) -> (size, Ratio) {
		let size = Self::size(self);
		//if fit.x*size.y < fit.y*fit.x { Ratio{num: fit.x-1, div: size.x-1} } else { Ratio{num: fit.y-1, div: size.y-1} } // Fit
		(size, if size.is_zero() { Ratio{num: 1, div: 1} } else { Ratio{num: fit.x-1, div: size.x-1} }) // Fit width
	}
	pub fn scale(&mut self, fit: size) -> Ratio { self.size_scale(fit).1 }
}

#[derive(PartialEq,Eq,PartialOrd,Ord,Clone,Copy,Debug)] pub struct LineColumn {
	pub line: usize,
	pub column: GraphemeIndex // May be on the right of the corresponding line (preserves horizontal affinity during line up/down movement)
}

pub fn index(text: &str, LineColumn{line, column}: LineColumn) -> GraphemeIndex {
	let Range{start, end} = line_ranges(text).nth(line).unwrap().range;
	assert!(start+column <= end);
	start+column
}

impl LineColumn {
	#[throws(as Option)] pub fn from_text_index(text: &str, index: GraphemeIndex) -> Self {
		let (line, LineRange{range: Range{start,..}, ..}) = line_ranges(text).enumerate().find(|&(_,LineRange{range: Range{start,end},..})| start <= index && index <=/*\n*/ end)?;
		Self{line, column: index-start}
	}
}

#[derive(PartialEq,Clone,Copy,Debug)] pub struct Span {
	pub start: LineColumn,
	pub end: LineColumn,
}
impl Span {
	pub fn new(end: LineColumn) -> Self { Self{start: end, end} }
	pub fn min(&self) -> LineColumn { min(self.start, self.end) }
	pub fn max(&self) -> LineColumn { max(self.start, self.end) }
}

pub(crate) mod iter;
use iter::NthOrLast;
fn position(font: &Font<'_>, text: &str, LineColumn{line, column}: LineColumn) -> uint2 {
	if text.is_empty() { assert!(line==0&&column==0); zero() } else {
	xy{
		x: layout(font, line_ranges(text).nth(line).unwrap().graphemes(true).enumerate()).nth_or_last(column as usize).map_or_else(
			|last| last.map_or(0, |Glyph{x,id,face,..}| x+face.glyph_hor_advance(id).unwrap() as u32),
			|layout| layout.x
		) as u32,
		y: (line as u32)*(font[0].height() as u32)
	}
}}

impl<D:AsRef<str>> View<'_, D> {
	pub fn text(&self) -> &str { AsRef::<str>::as_ref(&self.data) }
	fn position(&self, cursor: LineColumn) -> uint2 { self::position(&self.font, self.text(), cursor) }
	pub fn span(&self, min: LineColumn, max: LineColumn) -> Rect {
		Rect{min: self.position(min).signed(), max: (self.position(max)+xy{x:0, y: self.font[0].height() as u32}).signed()}
	}
	/*#[throws(as Option)]*/ pub fn cursor(&mut self, size: size, position: uint2) -> LineColumn {
		//fn ensure(s: bool) -> Option<()> { s.then(||()) }
		//ensure(!self.text().is_empty())?;
		let position = position / self.scale(size);
		let View{font, ..} = &self;
		let line = ((position.y/font[0].height() as u32) as usize).min(line_ranges(self.text()).count()-1);
		LineColumn{line, column:
			layout(font, line_ranges(self.text()).nth(line).unwrap().graphemes(true).enumerate())
			.map(|Glyph{index, x, id, face}| (index, x+face.glyph_hor_advance(id).unwrap() as u32/2))
			.take_while(|&(_, x)| x <= position.x).last().map(|(index,_)| index+1).unwrap_or(0)
		}
	}
	pub fn paint_span(&self, _context: &mut RenderContext, _scale: Ratio, _offset: int2, span: Span, _bgr: crate::color::bgr<bool>) {
		let [min, max] = [span.min(), span.max()];
		let /*mut*/ invert = |_r:Rect| {};//image::invert(&mut target.slice_mut_clip(scale*(offset+r))?, bgr);
		if min.line < max.line { invert(self.span(min,LineColumn{line: min.line, column: usize::MAX})); }
		if min.line == max.line {
			if min != max { invert(self.span(min,max)); } // selection
			else { // cursor
				fn widen(l: Rect, dx: u32) -> Rect { Rect{min: l.min-xy{x:dx/2,y:0}.signed(), max:l.max+xy{x:dx/2,y:0}.signed()} }
				invert(widen(self.span(span.end,span.end), self.font[0].height() as u32/16));
			}
		}
		else { for line in min.line+1..max.line {
			invert(self.span(LineColumn{line, column: 0},LineColumn{line, column: usize::MAX}));
		}}
		if max.line > min.line { invert(self.span(LineColumn{line: max.line, column: 0}, max)); }
	}
}

impl<D:AsRef<str>+AsRef<[Attribute<Style>]>> View<'_, D> {
	pub fn paint(&mut self, context: &mut RenderContext, size: size, scale: Ratio, offset: int2) {
		let Self{font, data, ..} = &*self;
		let (mut style, mut styles) = (None, AsRef::<[Attribute<Style>]>::as_ref(&data).iter().peekable());
		for (line_index, line) in line_ranges(&data.as_ref()).enumerate()
																						.take_while({let clip = (size.y/scale) as i32 - offset.y; move |&(line_index,_)| (((line_index as u32)*(font[0].height() as u32)) as i32) < clip}) {
			for Glyph{index, x, id, face} in layout(font, line.graphemes(true).enumerate().map(|(i,e)| (line.range.start+i, e))) {
				style = style.filter(|style:&&Attribute<Style>| style.contains(&index));
				while let Some(next) = styles.peek() {
					if next.end <= index { styles.next(); } // skips whitespace style
					else if next.contains(&index) { style = styles.next(); }
					else { break; }
				}
				let position = xy{
					x: (x as i32+face.glyph_hor_side_bearing(id).unwrap() as i32) as u32,
					y: (line_index as u32) * (font[0].height() as u32) + font[0].ascender() as u32
				};
				let mut glyph = piet_gpu::encoder::GlyphEncoder::default();
				let mut path_encoder = PathEncoder{scale: scale.into(), offset: f32::from(scale)*vec2::from(offset + position.signed()), path_encoder: glyph.path_encoder()};
				if face.outline_glyph(id, &mut path_encoder).is_some() {
					let mut path_encoder = path_encoder.path_encoder;
					path_encoder.path();
					let n_pathseg = path_encoder.n_pathseg();
    				glyph.finish_path(n_pathseg);
					context.encode_glyph(&glyph);
					context.fill_glyph(piet::Color::BLACK.as_rgba_u32());
				}
			}
		}
	}
	pub fn paint_fit(&mut self, cx: &mut RenderContext, size: size, offset: int2) -> Ratio {
		let scale = self.scale(size);
		self.paint(cx, size, scale, offset);
		scale
	}
}
use crate::widget::{Widget, RenderContext};
impl<'f, D:AsRef<str>+AsRef<[Attribute<Style>]>> Widget for View<'f, D> {
	fn size(&mut self, size: size) -> size { fit_width(size.x, Self::size(self)) }
	#[throws] fn paint(&mut self, cx: &mut RenderContext, size: size, offset: int2) { self.paint_fit(cx, size, offset); }
}

pub struct Plain<T>(pub T);
impl<T:AsRef<str>> AsRef<str> for Plain<T> { fn as_ref(&self) -> &str { self.0.as_ref() } }
impl<T> AsRef<[Attribute<Style>]> for Plain<T> {  fn as_ref(&self) -> &[Attribute<Style>] { &[] } }

pub struct Buffer<T, S> {
	pub text : T,
	pub style: S,
}
impl<T:AsRef<str>,S> AsRef<str> for Buffer<T,S> { fn as_ref(&self) -> &str { self.text.as_ref() } }
impl<T,S:AsRef<[Attribute<Style>]>> AsRef<[Attribute<Style>]> for Buffer<T,S> {  fn as_ref(&self) -> &[Attribute<Style>] { self.style.as_ref() } }
pub type Borrowed<'t> = Buffer<&'t str, &'t [Attribute<Style>]>;
