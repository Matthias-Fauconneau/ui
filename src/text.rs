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
    pub(crate) fn char_indices(&self) -> impl Iterator<Item=(TextSize,char)>+'_ {
        self.line.char_indices().map(move |(offset,c)| (((self.range.start+offset) as u32).into(), c))
    }
}

use {std::cmp::{min, max}, ttf_parser::{Face,GlyphId,Rect}, core::{error::{throws, Error}, num::Ratio}, crate::font::{self, Rasterize}};

pub(crate) struct Glyph {pub index: TextSize, pub x: i32, pub id: GlyphId, pub bbox: Rect}
pub(crate) fn layout<'t>(font: &'t Face<'t>, iter: impl Iterator<Item=(TextSize,char)>+'t) -> impl 't+Iterator<Item=Glyph> {
    iter.scan((None, 0), move |(last_id, x), (index, c)| {
		let id = font.glyph_index(c).unwrap_or_else(||panic!("Missing glyph for '{:?}'",c));
		if let Some(last_id) = *last_id { *x += font.kerning_subtables().next().map_or(0, |x| x.glyphs_kerning(last_id, id).unwrap_or(0) as i32); }
		*last_id = Some(id);
		let next = (index, *x, id);
		*x += font.glyph_hor_advance(id)? as i32;
		Some(next)
	})
	.filter_map(move |(index, x, id)| Some(Glyph{index, x, id, bbox: font.glyph_bounding_box(id)?}) )
}

#[derive(Default)] pub(crate) struct LineMetrics {pub width: u32, pub ascent: i16, pub descent: i16}
pub(crate) fn metrics(font: &Face<'_>, iter: impl Iterator<Item=Glyph>) -> LineMetrics {
	iter.fold(Default::default(), |metrics: LineMetrics, Glyph{x, id, bbox, ..}| LineMetrics{
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

pub struct Text<'font, 'text> {
    pub font : &'font Face<'font>,
    pub text : &'text str,
    style: &'text [Attribute<Style>],
    //size : Option<size2>
    cache : Vec<(GlyphId,Image<Vec<u8>>)>
}

use {::xy::{xy, size}, image::{Image, bgra8}, core::num::div_ceil};

impl<'font, 'text> Text<'font, 'text> {
    pub fn new(font: &'font Face<'font>, text : &'text str, style: &'text [Attribute<Style>]) -> Self { Self{font, text, style/*, size: None*/, cache: Vec::new()} }
    pub fn size(&/*mut*/ self) -> size {
        let Self{font, text, /*ref mut size,*/ ..} = self;
        //*size.get_or_insert_with(||{
            let (line_count, max_width) = line_ranges(text).fold((0,0),|(line_count, width), line| (line_count+1, max(width, metrics(font, layout(font, line.char_indices())).width)));
            xy{x: max_width, y: line_count * (font.height() as u32)}
        //})
    }
    pub fn paint(&mut self, target : &mut Image<&mut[bgra8]>, scale: Ratio) {
		let (mut style, mut styles) = (None, self.style.iter().peekable());
        core::time(|| for (line_index, line) in line_ranges(self.text).enumerate() {
            for Glyph{index, x, id, bbox} in layout(self.font, line.char_indices()) {
                let position = xy{
                    x: (x+self.font.glyph_hor_side_bearing(id).unwrap() as i32) as u32,
                    y: (line_index as u32)*(self.font.height() as u32) + (self.font.ascender()-bbox.y_max) as u32
                };
                let coverage = self.cache.iter().find(|(key,_)| key == &id);
                let (_,coverage) = if let Some(coverage) = coverage { coverage } else {
					self.cache.push( (id, image::from_linear(&self.font.rasterize(scale, id, bbox).as_ref())) );
					self.cache.last().unwrap()
                };
                use core::iter::{PeekableExt, Single};
                style = style.filter(|style:&&Attribute<Style>| style.contains(index)).or_else(|| styles.peeking_take_while(|style| style.contains(index)).single());
                assert!( style.map(|x|x.attribute.color).unwrap() == image::bgr{b:1., g:1., r:1.}); // todo: approximate linear tint cached sRGB glyphs
                image::set_map(&mut target.slice_mut(scale*position, coverage.size), &coverage.as_ref())
            }
        })
    }
}

fn fit_width(width: u32, from : size) -> size { xy{x: width, y: div_ceil(width * from.y, from.x)} }

use crate::widget::{Widget, Target};
impl Text<'_,'_> {
    pub fn scale(&mut self, target: &Target) -> Ratio { Ratio{num: target.size.x-1, div: Text::size(&self).x-1} } // todo: scroll
}
impl Widget for Text<'_,'_> {
    fn size(&mut self, bounds : size) -> size { fit_width(bounds.x, Text::size(&self)) }
    #[throws] fn paint(&mut self, target : &mut Target) {
        let scale = self.scale(&target);
        Text::paint(self, target, scale)
    }
}
