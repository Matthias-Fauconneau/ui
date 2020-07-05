#![allow(non_upper_case_globals)]
use {std::cmp::{min, max}, ttf_parser::{GlyphId,Rect}, crate::{error::{throws, Error}, num::Ratio, font::{self, Font}}};

pub struct FontIter<'t, I: 't> { font: &'t Font<'t>, iter: I }
impl<'t, T> std::ops::Deref for FontIter<'t, T> { type Target = T; fn deref(&self) -> &Self::Target { &self.iter } }
impl<'t, T> std::ops::DerefMut for FontIter<'t, T> { fn deref_mut(&mut self) -> &mut Self::Target { &mut self.iter } }
impl<'t, I:IntoIterator> IntoIterator for FontIter<'t, I> { type Item=I::Item; type IntoIter=I::IntoIter; fn into_iter(self) -> Self::IntoIter { self.iter.into_iter() } }

pub trait Char { fn char(&self) -> char; }
impl Char for char { fn char(&self) -> char { *self } }
impl<T> Char for (T, char) { fn char(&self) -> char { self.1 } }
type GlyphIDs<'t, I:Iterator> = impl 't+Iterator<Item=(I::Item, GlyphId)>; // Map
impl<'t> Font<'t> {
    pub fn glyphs<I:'t+Iterator<Item:Char>>(&'t self, iter: I) -> FontIter<'t, GlyphIDs<'t, I>> {
		FontIter{font: self, iter: iter.map(move |item|{let c=item.char(); (item, self.glyph_index(c).unwrap_or_else(||panic!("Missing glyph for '{:?}'",c)))})}
    }
}
pub trait DerefGlyphId { fn id(&self) -> &GlyphId; }
impl<T> DerefGlyphId for (T, GlyphId) { fn id(&self) -> &GlyphId { &self.1 } }

pub struct Layout<T>{pub x: i32, pub glyph: T, pub bbox: Rect}
type LayoutGlyphs<'t, I:Iterator> = impl 't+Iterator<Item=Layout<I::Item>>; // FilterMap<Scan>
impl<'t, I:'t+Iterator<Item:DerefGlyphId>> FontIter<'t, I> {
    pub fn layout(self) -> FontIter<'t, LayoutGlyphs<'t, I>> {
        FontIter{
            font: self.font,
            iter: self.iter.scan((None,0),{let font = self.font; move |(last_id, x), glyph| {
                        let id = *glyph.id();
                        if let Some(last_id) = *last_id { *x += font.kerning_subtables().next().map_or(0, |x| x.glyphs_kerning(last_id, id).unwrap_or(0) as i32); }
                        *last_id = Some(id);
                        let next = (*x, glyph);
                        *x += font.glyph_hor_advance(id)? as i32;
                        Some(next)
                   }})
                   .filter_map({let font = self.font; move |(x, glyph)| { let id = *glyph.id(); Some(Layout{x, glyph, bbox: font.glyph_bounding_box(id)?}) }})
        }
    }
}

#[derive(Default)] pub struct LineMetrics {pub width: u32, pub ascent: i16, pub descent: i16}
impl<T:DerefGlyphId, I:Iterator<Item=Layout<T>>> FontIter<'_, I> {
    pub fn metrics(self) -> LineMetrics {
        let font = &self.font;
        self.iter.fold(Default::default(), |metrics:LineMetrics, item| {
            let Layout{x, glyph, bbox} = item;
            LineMetrics{
                width: (x + font.glyph_hor_side_bearing(*glyph.id()).unwrap() as i32 + bbox.x_max as i32) as u32,
                ascent: max(metrics.ascent, bbox.y_max),
                descent: min(metrics.descent, bbox.y_min)
            }
        })
    }
}

pub use text_size::{TextSize, TextRange}; // ~Range<u32> with impl SliceIndex for String
use {derive_more::Deref, crate::{iter::{Single, PeekableExt}, num::{Zero, div_ceil}, vector::{uint2, size2}, image::{Image, bgra8}}};

#[derive(Deref)] struct LineRange<'t> { #[deref] text: &'t str, range: std::ops::Range<usize>}
impl LineRange<'_> {
    fn char_indices(&self) -> impl Iterator<Item=(TextSize,char)>+'_ {
        self.text[self.range.clone()].char_indices().map(move |(offset,c)| (((self.range.start+offset) as u32).into(), c))
    }
}
type ImplLineRanges<'t> = impl Iterator<Item=LineRange<'t>>;
trait LineRanges<'t> { fn line_ranges(self) -> ImplLineRanges<'t>; }
impl<'t> LineRanges<'t> for &'t str {
    fn line_ranges(self) -> ImplLineRanges<'t> {
        let mut iter = self.char_indices().peekable();
        std::iter::from_fn(move || {
            let &(start,_) = iter.peek()?;
            let end = iter.find(|&(_,c)| c=='\n').map_or(self.len(), |(end,_)| end);
            Some(LineRange{text: self, range: start..end})
        })
    }
}

pub type Color = crate::image::bgrf;
#[derive(Clone,Copy)] pub enum FontStyle { Normal, Bold, /*Italic, BoldItalic*/ }
#[derive(Clone,Copy)] pub struct Style { pub color: Color, pub style: FontStyle }
#[derive(Clone,Copy)] pub struct Attribute<T> { pub range: TextRange, pub attribute: T }
impl<T> std::ops::Deref for Attribute<T> { type Target=TextRange; fn deref(&self) -> &Self::Target { &self.range } }

lazy_static::lazy_static! {
	static ref default_font : font::File<'static> = font::open(
		["/usr/share/fonts/noto/NotoSans-Regular.ttf","/usr/share/fonts/liberation-fonts/LiberationSans-Regular.ttf"].iter().map(std::path::Path::new)
			.filter(|x| std::path::Path::exists(x))
			.next().unwrap()
	).unwrap();
	pub static ref default_style: [Attribute::<Style>; 1] =
		[Attribute::<Style>{range: TextRange::up_to(u32::MAX.into()), attribute: Style{color: Color{b:1.,r:1.,g:1.}, style: FontStyle::Normal}}];
}

pub struct Text<'font, 'text> {
    pub font : &'font Font<'font>,
    pub text : &'text str,
    style: &'text [Attribute<Style>],
    //size : Option<size2>
}
impl<'font, 'text> Text<'font, 'text> {
    pub fn new(font: &'font Font<'font>, text : &'text str, style: &'text [Attribute<Style>]) -> Self { Self{font, text, style/*, size: None*/} }
    pub fn size(&/*mut*/ self) -> size2 {
        let Self{font, text, /*ref mut size,*/ ..} = self;
        //*size.get_or_insert_with(||{
            let (line_count, max_width) = text.lines()
				.map(|line| font.glyphs(line.chars()).layout().metrics())
				.fold((0,0),|(line_count, width), line| (line_count+1, max(width, line.width)));
            size2{x: max_width, y: line_count * (font.height() as u32)}
        //})
    }
    pub fn paint(&self, target : &mut Image<&mut[bgra8]>, scale: Ratio) {
        let (mut style, mut styles) = (None, self.style.iter().peekable());
        for (line_index, line) in self.text.line_ranges().enumerate() {
            for Layout{x, glyph: ((index,_), id), bbox} in self.font.glyphs(line.char_indices()).layout() {
                let position = uint2{
                    x: (x+self.font.glyph_hor_side_bearing(id).unwrap() as i32) as u32,
                    y: (line_index as u32)*(self.font.height() as u32) + (self.font.ascender()-bbox.y_max) as u32
                };
                let coverage = self.font.rasterize(scale, id, bbox);
                style = style.filter(|style:&&Attribute<Style>| style.contains(index)).or_else(|| styles.peeking_take_while(|style| style.contains(index)).single());
                target.slice_mut(scale*position, coverage.size).set_map(coverage, |_,&coverage| bgra8{a : 0xFF, ..(coverage*style.map(|x|x.attribute.color).unwrap()).into()})
            }
        }
    }
}

fn fit_width(width: u32, from : size2) -> size2 { size2{x: width, y: div_ceil(width * from.y, from.x)} }

use crate::widget::{Widget, Target};
impl Text<'_,'_> {
    pub fn scale(&mut self, target: &Target) -> Ratio { Ratio{num: target.size.x-1, div: Text::size(&self).x-1} } // todo: scroll
}
impl Widget for Text<'_,'_> {
    fn size(&mut self, bounds : size2) -> size2 { fit_width(bounds.x, Text::size(&self)) }
    #[throws] fn paint(&mut self, target : &mut Target) {
        let scale = self.scale(&target);
        Text::paint(&self, target, scale)
    }
}


struct LineColumn{line: usize, column: usize}
impl Zero for LineColumn { fn zero() -> Self { Self{line: 0, column: 0} } }

pub struct TextEdit<'font, 'text> {
    text: Text<'font, 'text>,
    cursor: LineColumn
}

impl<'font, 'text> TextEdit<'font, 'text> {
    pub fn new(text : &'text str, style: &'text [Attribute<Style>]) -> Self {
        Self{text: Text::new(&default_font, text, style), cursor: Zero::zero()}
    }
}

impl Widget for TextEdit<'_,'_> {
    fn size(&mut self, size : size2) -> size2 { Widget::size(&mut self.text, size) }
    #[throws] fn paint(&mut self, target : &mut Target) {
        Widget::paint(&mut self.text, target)?;
        /*if self.hasFocus()*/ {
            let scale = self.text.scale(&target);
            let Self{text: Text{text, font, ..}, cursor} = self;
            let &mut LineColumn{line: line_index, column} = cursor;
            let line = text.line_ranges().nth(line_index).unwrap();
            trait NthOrLast : Iterator {
                fn nth_or_last(&mut self, mut n: usize) -> Result<Self::Item, Option<Self::Item>> {
                    let mut last = None;
                    for x in self {
                        if n == 0 { return Ok(x); }
                        n -= 1;
                        last = Some(x);
                    }
                    Err(last)
                }
            }
            impl<I:Iterator> NthOrLast for I {}
            let position = uint2{
                x:
					font.glyphs(line.chars()).layout().nth_or_last(column).map_or_else(
                        |last| last.map_or(0, |last| last.x+(font.glyph_hor_advance(*last.glyph.id()).unwrap() as i32)),
                        |layout| layout.x
					) as u32,
                y: (line_index as u32)*(font.height() as u32)
            };
            let height = font.height() as u32;
            target.slice_mut(scale*position, scale*uint2{x:height/8,y:height}).set(|_| 0xFF.into());
        }
    }
}
