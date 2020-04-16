pub(self) mod raster;
mod font;

use {std::cmp::max, derive_more::Deref, crate::{core::{ceil_div,Single,PeekableExt,Zero},lazy_static, vector::{uint2,size2}, image::{Image, bgra8}}};
use font::{Font, Layout,GlyphID};

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
pub use text_size::{TextSize, TextRange}; // ~Range<u32> with impl SliceIndex for String
#[derive(Clone,Copy)] pub struct Attribute<T> { pub range: TextRange, pub attribute: T }
impl<T> std::ops::Deref for Attribute<T> { type Target=TextRange; fn deref(&self) -> &Self::Target { &self.range } }

pub struct Text<'font, 'text> {
    font : &'font Font<'font>,
    text : &'text str,
    style: &'text [Attribute<Style>],
    size : Option<size2>
}
impl<'font, 'text> Text<'font, 'text> {
    pub fn new(text : &'text str, style: &'text [Attribute<Style>]) -> Self {
        lazy_static! { default_font : font::MapFont = font::from_file("/usr/share/fonts/noto/NotoSans-Regular.ttf").unwrap(); }
        Self{font: default_font.suffix(), text, style, size: None}
    }
    pub fn size(&mut self) -> size2 {
        let Self{font, text, ref mut size, ..} = self;
        *size.get_or_insert_with(||{
            let (count, max_width) = text.lines().map(|line| font.glyphs(line.chars()).layout().metrics()).fold((0,0),|(count, width), line| (count+1, max(width, line.width)));
            size2{x: max_width, y: count * (font.height() as u32)}
        })
    }
    pub fn render(&self, target : &mut Image<&mut[bgra8]>, scale: font::Scale) {
        let (mut style, mut styles) = (None, self.style.iter().peekable());
        for (line_index, line) in self.text.line_ranges().enumerate() {
            for Layout{x, glyph: ((offset,_), id), bbox} in self.font.glyphs(line.char_indices()).layout() {
                let position = uint2{
                    x: (x+self.font.glyph_hor_side_bearing(id).unwrap() as i32) as u32,
                    y: (line_index as u32)*(self.font.height() as u32) + (self.font.ascender()-bbox.y_max) as u32
                };
                let coverage = self.font.rasterize(scale, id, bbox);
                style = style.filter(|style:&&Attribute<Style>| style.contains(offset)).or(styles.peeking_take_while(|style| style.contains(offset)).single());
                target.slice_mut(scale*position, coverage.size).map(coverage, |_,coverage| bgra8{a : 0xFF, ..(coverage*style.map(|x|x.attribute.color).unwrap()).into()})
            }
        }
    }
}

fn fit_width(width: u32, from : size2) -> size2 { size2{x: width, y: ceil_div(width * from.y, from.x)} }

use crate::window::{Widget, Target};
impl Text<'_,'_> {
    pub fn scale(&mut self, target: &Target) -> font::Scale { font::Scale{num: target.size.x-1, div: self.size().x-1} } // todo: scroll
}
impl Widget for Text<'_,'_> {
    fn size(&mut self, size : size2) -> size2 { fit_width(size.x, self.size()) }
    fn render(&mut self, target : &mut Target) {
        let scale = self.scale(&target);
        Text::render(&self, target, scale)
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
        Self{text: Text::new(text, style), cursor: Zero::zero()}
    }
}

impl Widget for TextEdit<'_,'_> {
    fn size(&mut self, size : size2) -> size2 { Widget::size(&mut self.text, size) }
    fn render(&mut self, target : &mut Target) {
        Widget::render(&mut self.text, target);
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
                x: font.glyphs(line.chars()).layout().nth_or_last(column).map_or_else(
                        |last| last.map_or(0, |last| last.x+(font.glyph_hor_advance(last.glyph.id()).unwrap() as i32)),
                        |layout| layout.x
                    ) as u32,
                y: (line_index as u32)*(font.height() as u32)
            };
            let height = font.height() as u32;
            target.slice_mut(scale*position, scale*uint2{x:height/8,y:height}).set(|_| 0xFF.into());
        }
    }
}
