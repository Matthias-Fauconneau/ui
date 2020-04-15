pub(self) mod raster;
mod font; use font::Font;
#[allow(unused_imports)]
use {std::cmp::max, crate::{core::{ceil_div,Single,PeekableExt,split_for_each},lazy_static, vector::{uint2,size2}, image::{Image, bgra8}}};
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
        split_for_each(self.text.chars().enumerate().map(|(o,c)|((o as u32).into(),c)),|&(_,character)| character=='\n', |line_index, line| {
            for (pen, ((offset,_), glyph_id), bbox) in self.font.glyphs(line).layout() {
                let position = uint2{
                    x: (pen+self.font.glyph_hor_side_bearing(glyph_id).unwrap() as i32) as u32,
                    y: (line_index as u32)*(self.font.height() as u32) + (self.font.ascender()-bbox.y_max) as u32
                };
                let coverage = self.font.rasterize(scale, glyph_id, bbox);
                style = style.filter(|style:&&Attribute<Style>| style.contains(offset)).or(styles.peeking_take_while(|style| style.contains(offset)).single());
                target.slice_mut(scale*position, coverage.size).map(coverage, |_,coverage| bgra8{a : 0xFF, ..(coverage*style.map(|x|x.attribute.color).unwrap()).into()})
            }
        });
    }
}

fn fit_width(width: u32, from : size2) -> size2 { size2{x: width, y: ceil_div(width * from.y, from.x)} }

use crate::window::{Widget, Target};
impl Widget for Text<'_,'_> {
    fn size(&mut self, size : size2) -> size2 { fit_width(size.x, self.size()) }
    fn render(&mut self, target : &mut Target) {
        let scale = font::Scale{num: target.size.x-1, div: self.size().x-1}; // todo: scroll
        Text::render(&self, target, scale)
    }
}
