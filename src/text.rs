pub use text_size::{TextSize, TextRange}; // ~Range<u32> with impl SliceIndex for String

#[derive(derive_more::Deref)] pub(crate) struct LineRange<'t> { #[deref] line: &'t str, range: std::ops::Range<usize> }

pub(crate) fn line_ranges(text: &'t str) -> impl Iterator<Item=LineRange<'t>> {
	let mut iter = text.char_indices().peekable();
	std::iter::from_fn(move || {
		let &(start,_) = iter.peek()?;
		let range = start .. iter.find(|&(_,c)| c=='\n').map_or(text.len(), |(end,_)| end);
		Some(LineRange{line: &text[range.clone()], range})
	})
}

impl LineRange<'_> {
    pub(crate) fn char_indices(&self) -> impl Iterator<Item=(usize,char)>+'_ {
        self.line.char_indices().map(move |(offset,c)| (self.range.start+offset, c))
    }
}

use {std::cmp::{min, max}, ttf_parser::{Face,GlyphId,Rect}, core::{error::{throws, Error}, num::Ratio}, crate::font::{self, Rasterize}};

pub struct Glyph {pub index: TextSize, pub x: i32, pub id: GlyphId }
pub fn layout<'t>(font: &'t Face<'t>, iter: impl Iterator<Item=(usize,char)>+'t) -> impl 't+Iterator<Item=Glyph> {
    iter.scan((None, 0), move |(last_id, x), (index, c)| {
		let id = font.glyph_index(c).unwrap_or_else(||panic!("Missing glyph for '{:?}'",c));
		if let Some(last_id) = *last_id { *x += font.kerning_subtables().next().map_or(0, |x| x.glyphs_kerning(last_id, id).unwrap_or(0) as i32); }
		*last_id = Some(id);
		let next = Glyph{index: (index as u32).into(), x: *x, id};
		*x += font.glyph_hor_advance(id)? as i32;
		Some(next)
	})
}

pub(crate) fn bbox<'t>(font: &'t Face<'t>, iter: impl Iterator<Item=Glyph>+'t) -> impl 't+Iterator<Item=(Rect, Glyph)> {
	iter.filter_map(move |g| Some((font.glyph_bounding_box(g.id)?, g)))
}

#[derive(Default)] pub(crate) struct LineMetrics {pub width: u32, pub ascent: i16, pub descent: i16}
pub(crate) fn metrics(font: &Face<'_>, iter: impl Iterator<Item=Glyph>) -> LineMetrics {
	bbox(font, iter).fold(Default::default(), |metrics: LineMetrics, (bbox, Glyph{x, id, ..})| LineMetrics{
		width: (x + font.glyph_hor_side_bearing(id).unwrap() as i32 + bbox.x_max as i32) as u32,
		ascent: max(metrics.ascent, bbox.y_max),
		descent: min(metrics.descent, bbox.y_min)
	})
}

pub type Color = image::bgrf;
#[derive(Clone,Copy)] pub enum FontStyle { Normal, Bold, /*Italic, BoldItalic*/ }
#[derive(Clone,Copy)] pub struct Style { pub color: Color, pub style: FontStyle }
#[derive(Clone,Copy)] pub struct Attribute<T> { pub range: TextRange, pub attribute: T }
impl<T> std::ops::Deref for Attribute<T> { type Target=TextRange; fn deref(&self) -> &Self::Target { &self.range } }

use std::lazy::SyncLazy;
#[allow(non_upper_case_globals)] pub static default_font : SyncLazy<font::File<'static>> = SyncLazy::new(|| font::open(
		["/usr/share/fonts/noto/NotoSans-Regular.ttf","/usr/share/fonts/liberation-fonts/LiberationSans-Regular.ttf"].iter().map(std::path::Path::new)
			.filter(|x| std::path::Path::exists(x))
			.next().unwrap()
	).unwrap());
#[allow(non_upper_case_globals)] pub static default_style: SyncLazy<[Attribute::<Style>; 1]> = SyncLazy::new(||
		[Attribute::<Style>{range: TextRange::up_to(u32::MAX.into()), attribute: Style{color: Color{b:1.,r:1.,g:1.}, style: FontStyle::Normal}}] );
use std::sync::RwLock;
#[allow(non_upper_case_globals)] pub static cache: SyncLazy<RwLock<Vec<(GlyphId,Image<Vec<u8>>)>>> = SyncLazy::new(|| RwLock::new(Vec::new()));

use std::cell::RefCell;
pub struct Buffer<'t, T> {
	pub text : &'t RefCell<T>,
    pub style: &'t [Attribute<Style>],
}
impl<'t, T> Buffer<'t, T> { pub fn new(text: &'t RefCell<T>) -> Self { Self{text, style: &*default_style} } }

pub struct TextView<'f, 't, T> {
    pub font : &'f Face<'f>,
    pub buffer: Buffer<'t, T>,
    //size : Option<size2>
}

use {::xy::{xy, size}, image::{Image, bgra8}, core::num::div_ceil};

use core::num::{IsZero, Zero};
fn fit_width(width: u32, from : size) -> size { if from.is_zero() { return Zero::zero(); } xy{x: width, y: div_ceil(width * from.y, from.x)} }

impl<'f, 't, T:std::ops::Deref<Target=str>> TextView<'f, 't, T> {
    pub fn new(font: &'f Face<'f>, buffer: Buffer<'t, T>) -> Self { Self{font, buffer/*, size: None*/} }
    pub fn size(&/*mut*/ self) -> size {
        let Self{font, buffer: Buffer{text, ..}, /*ref mut size,*/ ..} = self;
        //*size.get_or_insert_with(||{
            let (line_count, max_width) = line_ranges(&text.borrow()).fold((0,0),|(line_count, width), line| (line_count+1, max(width, metrics(font, layout(font, line.char_indices())).width)));
            xy{x: max_width, y: line_count * (font.height() as u32)}
        //})
    }
    pub fn scale(&mut self, size: size) -> Ratio { Ratio{num: size.x-1, div: Self::size(&self).x-1} } // todo: scroll
    pub fn paint(&mut self, target : &mut Image<&mut[bgra8]>, scale: Ratio) {
		let (mut style, mut styles) = (None, self.buffer.style.iter().peekable());
        for (line_index, line) in line_ranges(&self.buffer.text.borrow()).enumerate() {
            for (bbox, Glyph{index, x, id}) in bbox(self.font, layout(self.font, line.char_indices())) {
				use core::iter::{PeekableExt, Single};
				style = style.filter(|style:&&Attribute<Style>| style.contains(index)).or_else(|| styles.peeking_take_while(|style| style.contains(index)).single());
                assert!( style.map(|x|x.attribute.color).unwrap() == image::bgr{b:1., g:1., r:1.}); // todo: approximate linear tint cached sRGB glyphs

                if cache.read().unwrap().iter().find(|(key,_)| key == &id).is_none() {
					cache.write().unwrap().push( (id, image::from_linear(&self.font.rasterize(scale, id, bbox).as_ref())) );
                };
                let cache_read = cache.read().unwrap();
                let coverage = &cache_read.iter().find(|(key,_)| key == &id).unwrap().1;

				let position = xy{
                    x: (x+self.font.glyph_hor_side_bearing(id).unwrap() as i32) as u32,
                    y: (line_index as u32)*(self.font.height() as u32) + (self.font.ascender()-bbox.y_max) as u32
                };
                image::set_map(&mut target.slice_mut(scale*position, coverage.size), &coverage.as_ref())
            }
        }
    }
}

use crate::widget::{Widget, Target};
impl<T:std::ops::Deref<Target=str>> Widget for TextView<'_,'_,T> {
    fn size(&mut self, bounds : size) -> size { fit_width(bounds.x, TextView::size(&self)) }
    #[throws] fn paint(&mut self, target : &mut Target) {
        let scale = self.scale(target.size);
        TextView::paint(self, target, scale)
    }
}
