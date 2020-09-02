use {std::{cmp::{min, max}, ops::Range}, ::xy::{xy, uint2, size, Rect}, ttf_parser::{Face,GlyphId}, fehler::throws, error::Error, num::{zero, Ratio}, crate::font::{self, Rasterize}};
pub mod unicode_segmentation;
use self::unicode_segmentation::{GraphemeIndex, UnicodeSegmentation};

#[derive(derive_more::Deref)] pub(crate) struct LineRange<'t> { #[deref] line: &'t str, pub(crate) range: Range<GraphemeIndex> }

pub(crate) fn line_ranges(text: &'t str) -> impl Iterator<Item=LineRange<'t>> {
	let mut iter = text.grapheme_indices(true).enumerate().peekable();
	std::iter::from_fn(move || {
		let &(start, (byte_start,_)) = iter.peek()?;
		let (end, byte_end) = iter.find(|&(_,(_,c))| c=="\n").map_or((text.len(),text.len()/*fixme*/), |(end,(byte_end,_))| (end, byte_end));
		Some(LineRange{line: &text[byte_start..byte_end], range: start..end})
	})
}

pub struct Glyph {pub index: GraphemeIndex, pub x: i32, pub id: GlyphId }
pub fn layout<'t>(font: &'t Face<'t>, iter: impl Iterator<Item=(GraphemeIndex, &'t str)>+'t) -> impl 't+Iterator<Item=Glyph> {
	iter.scan((None, 0), move |(last_id, x), (index, g)| {
		use iter::Single;
		let c = g.chars().single().unwrap();
		let id = font.glyph_index(if c == '\t' { ' ' } else { c }).unwrap_or_else(||panic!("Missing glyph for '{:?}'",c));
		if let Some(last_id) = *last_id { *x += font.kerning_subtables().next().map_or(0, |x| x.glyphs_kerning(last_id, id).unwrap_or(0) as i32); }
		*last_id = Some(id);
		let next = Glyph{index, x: *x, id};
		*x += font.glyph_hor_advance(id)? as i32;
		Some(next)
	})
}

pub fn rect(r: ttf_parser::Rect) -> Rect { Rect{min:xy{x:r.x_min as i32, y:r.y_min as i32},max:xy{x:r.x_max as i32, y:r.y_max as i32}} }
pub(crate) fn bbox<'t>(font: &'t Face<'t>, iter: impl Iterator<Item=Glyph>+'t) -> impl 't+Iterator<Item=(Rect, Glyph)> {
	iter.filter_map(move |g| Some((font.glyph_bounding_box(g.id).map(rect)?, g)))
}

#[derive(Default)] pub(crate) struct LineMetrics {pub width: u32, pub ascent: i16, pub descent: i16}
pub(crate) fn metrics(font: &Face<'_>, iter: impl Iterator<Item=Glyph>) -> LineMetrics {
	bbox(font, iter).fold(Default::default(), |metrics: LineMetrics, (bbox, Glyph{x, id, ..})| LineMetrics{
		width: (x + font.glyph_hor_side_bearing(id).unwrap() as i32 + bbox.max.x) as u32,
		ascent: max(metrics.ascent, bbox.max.y as i16),
		descent: min(metrics.descent, bbox.min.y as i16)
	})
}

pub type Color = image::bgrf;
#[derive(Clone,Copy,Debug)] pub enum FontStyle { Normal, Bold, /*Italic, BoldItalic*/ }
impl Default for FontStyle { fn default() -> Self { Self::Normal } }
#[derive(Clone,Copy,Default,Debug)] pub struct Style { pub color: Color, pub style: FontStyle }
pub type TextRange = std::ops::Range<GraphemeIndex>;
#[derive(Clone,derive_more::Deref,Debug)] pub struct Attribute<T> { #[deref] pub range: TextRange, pub attribute: T }

use std::lazy::SyncLazy;
#[allow(non_upper_case_globals)] pub static default_font : SyncLazy<font::File<'static>> = SyncLazy::new(|| font::open(
		["/usr/share/fonts/noto/NotoSans-Regular.ttf","/usr/share/fonts/liberation-fonts/LiberationSans-Regular.ttf"].iter().map(std::path::Path::new)
			.filter(|x| std::path::Path::exists(x))
			.next().unwrap()
	).unwrap());
#[allow(non_upper_case_globals)]
pub const default_style: [Attribute::<Style>; 1] = [Attribute{range: 0..GraphemeIndex::MAX, attribute: Style{color: Color{b:1.,r:1.,g:1.}, style: FontStyle::Normal}}];
use std::sync::Mutex;
pub static CACHE: SyncLazy<Mutex<Vec<((Ratio, GlyphId),Image<Vec<u8>>)>>> = SyncLazy::new(|| Mutex::new(Vec::new()));

pub struct View<'f, D> {
    pub font : &'f Face<'f>,
    pub data: D,
    //size : Option<size2>
}

use {image::{Image, bgra8}, num::{IsZero, Zero, div_ceil, clamp}};

fn fit_width(width: u32, from : size) -> size { if from.is_zero() { return Zero::zero(); } xy{x: width, y: div_ceil(width * from.y, from.x)} }
fn fit_height(height: u32, from : size) -> size { if from.is_zero() { return Zero::zero(); } xy{x: div_ceil(height * from.x, from.y), y: height} }
pub fn fit(size: size, from: size) -> size { if size.x*from.y < size.y*from.x { fit_width(size.x, from) } else { fit_height(size.y, from) } }

impl<D:AsRef<str>> View<'_, D> {
	pub fn size(&/*mut*/ self) -> size {
		let Self{font, data, /*ref mut size,*/ ..} = self;
		//*size.get_or_insert_with(||{
			let text = data.as_ref();
			let (line_count, max_width) = line_ranges(&text).fold((0,0),|(line_count, width), line| (line_count+1, max(width, metrics(font, layout(font, line.graphemes(true).enumerate())).width)));
			xy{x: max_width, y: line_count * (font.height() as u32)}
		//})
	}
	pub fn scale(&/*mut*/ self, fit: size) -> Ratio {
		let size = Self::size(&self);
		if fit.x*size.y < fit.y*fit.x { Ratio{num: fit.x-1, div: size.x-1} } else { Ratio{num: fit.y-1, div: size.y-1} } // todo: scroll
	}
}

#[derive(PartialEq,Eq,PartialOrd,Ord,Clone,Copy,Debug)] pub struct LineColumn {
	pub line: usize,
	pub column: GraphemeIndex // May be on the right of the corresponding line (preserves horizontal affinity during line up/down movement)
}
impl Zero for LineColumn { fn zero() -> Self { Self{line: 0, column: 0} } }

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
impl Zero for Span { fn zero() -> Self { Self{start: Zero::zero(), end: Zero::zero()} } }
impl Span {
	pub fn new(end: LineColumn) -> Self { Self{start: end, end} }
	pub fn min(&self) -> LineColumn { min(self.start, self.end) }
	pub fn max(&self) -> LineColumn { max(self.start, self.end) }
}

use iter::NthOrLast;
fn position(font: &ttf_parser::Face<'_>, text: &str, LineColumn{line, column}: LineColumn) -> uint2 { xy{
	x: layout(font, line_ranges(text).nth(line).unwrap().graphemes(true).enumerate()).nth_or_last(column as usize).map_or_else(
		|last| last.map_or(0, |Glyph{x,id,..}| x+(font.glyph_hor_advance(id).unwrap() as i32)),
		|layout| layout.x
	) as u32,
	y: (line as u32)*(font.height() as u32)
}}
fn span(font: &ttf_parser::Face<'_>, text: &str, min: LineColumn, max: LineColumn) -> Rect {
	Rect{min: position(font, text, min).signed(), max: (position(font, text, max)+xy{x:0, y: font.height() as u32}).signed()}
}

impl<D:AsRef<str>> View<'_, D> {
	pub fn cursor(&self, size: size, position: uint2) -> LineColumn {
		let position = position / self.scale(size);
		let View{font, data, ..} = self;
		let text = data.as_ref();
		let line = clamp(0, (position.y/font.height() as u32) as usize, line_ranges(text).count()-1);
		LineColumn{line, column:
			layout(font, line_ranges(text).nth(line).unwrap().graphemes(true).enumerate())
			.map(|Glyph{index, x, id}| (index, x+font.glyph_hor_advance(id).unwrap() as i32/2))
			.take_while(|&(_, x)| x <= position.x as i32).last().map(|(index,_)| index+1).unwrap_or(0)
		}
	}
	pub fn paint_span(&self, target : &mut Target, scale: Ratio, span: Span, bgr: image::bgr<bool>) {
		let Self{font, data} = self;
		let [min, max] = [span.min(), span.max()];
		let text = AsRef::<str>::as_ref(&data);
		if min.line < max.line { image::invert(&mut target.slice_mut_clip(scale*self::span(font,text,min,LineColumn{line: min.line, column: usize::MAX})), bgr); }
		if min.line == max.line {
			if min == max { // cursor
				fn widen(l: Rect, dx: u32) -> Rect { Rect{min: l.min-xy{x:dx/2,y:0}.signed(), max:l.max+xy{x:dx/2,y:0}.signed()} }
				image::invert(&mut target.slice_mut_clip(scale*widen(self::span(font,text,span.end,span.end), font.height() as u32/16)), bgr);
			}
			if min != max { // selection
				image::invert(&mut target.slice_mut_clip(scale*self::span(font,text,min,max)), bgr);
			}
		}
		else { for line in min.line+1..max.line {
			image::invert(&mut target.slice_mut_clip(scale*self::span(font,text,LineColumn{line, column: 0},LineColumn{line, column: usize::MAX})), bgr);
		}}
		if max.line > min.line { image::invert(&mut target.slice_mut_clip(scale*self::span(font,text,LineColumn{line: max.line, column: 0}, max)), bgr); }
	}
}

impl<D:AsRef<str>+AsRef<[Attribute<Style>]>> View<'_, D> {
	pub fn paint(&mut self, target : &mut Image<&mut[bgra8]>, scale: Ratio) {
		if target.size < self.size(target.size) { return; }
		//assert!(target.size >= self.size(target.size), "{:?} {:?} ", target.size, self.size(target.size));
		let Self{font, data} = &*self;
		let (mut style, mut styles) = (None, AsRef::<[Attribute<Style>]>::as_ref(&data).iter().peekable());
		for (line_index, line) in line_ranges(&data.as_ref()).enumerate() {
			for (bbox, Glyph{index, x, id}) in bbox(font, layout(font, line.graphemes(true).enumerate().map(|(i,e)| (line.range.start+i, e)))) {
				style = style.filter(|style:&&Attribute<Style>| style.contains(&index));
				while let Some(next) = styles.peek() {
					if next.end <= index { styles.next(); } // skips whitespace style
					else if next.contains(&index) { style = styles.next(); }
					else { break; }
				}
				let mut cache = CACHE.lock().unwrap();
				if cache.iter().find(|(key,_)| key == &(scale, id)).is_none() {
					cache.push( ((scale, id), image::from_linear(&self.font.rasterize(scale, id, bbox).as_ref())) );
				};
				let coverage = &cache.iter().find(|(key,_)| key == &(scale, id)).unwrap().1;
				let position = xy{
					x: (x+font.glyph_hor_side_bearing(id).unwrap() as i32) as u32,
					y: (line_index as u32)*(font.height() as u32) + (font.ascender()-bbox.max.y as i16) as u32
				};
				let offset = scale*position;
				let size = vector::component_wise_min(coverage.size, target.size-offset);
				image::fill_mask(&mut target.slice_mut(offset, size), style.map(|x|x.attribute.color).unwrap_or((1.).into()), &coverage.slice(zero(), size));
			}
		}
	}
}
use crate::widget::{Widget, Target};
impl<'f, D:AsRef<str>+AsRef<[Attribute<Style>]>> Widget for View<'f, D> {
    fn size(&mut self, size: size) -> size { fit(size, Self::size(&self)) }
    #[throws] fn paint(&mut self, target : &mut Target) {
        let scale = self.scale(target.size);
        Self::paint(self, target, scale)
    }
}
