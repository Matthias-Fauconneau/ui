pub fn floor_div(n : u32, d : u32) -> u32 { n/d }
pub fn ceil_div(n : u32, d : u32) -> u32 { (n+d-1)/d }
use {std::cmp::{min, max}, crate::{lazy_static, core::{sign,Result}, vector::{uint2,size2,vec2,lerp,sq},image::{Image,bgra8,sRGB::sRGB}}, ttf_parser::{GlyphId,Rect}};

pub struct Font<'t>(ttf_parser::Font<'t>);
impl<'t> std::ops::Deref for Font<'t> { type Target = ttf_parser::Font<'t>; fn deref(&self) -> &Self::Target { &self.0 } }

struct FontIter<'t, I> { font: &'t Font<'t>, iter: I }
impl<'t, T> std::ops::Deref for FontIter<'t, T> { type Target = T; fn deref(&self) -> &Self::Target { &self.iter } }
impl<'t, T> std::ops::DerefMut for FontIter<'t, T> { fn deref_mut(&mut self) -> &mut Self::Target { &mut self.iter } }

type GlyphIDs<'t, T> = impl 't+Iterator<Item=GlyphId>; // Map
impl Font<'_> {
    fn glyphs<'s:'t, 't, Chars:'t+Iterator<Item=char>>(&'s self, iter: Chars) -> FontIter<'t, GlyphIDs<'t, Chars>> {
        FontIter{font: self, iter: iter.map(move |c| self.glyph_index(c).unwrap_or_else(||panic!("Missing glyph for '{:?}'",c)))}
    }
}

type LayoutGlyphs<'t, T> = impl 't+Iterator<Item=(i32,GlyphId,Rect)>; // FilterMap<Scan>
impl<'t, GlyphIDs:'t+Iterator<Item=GlyphId>> FontIter<'t, GlyphIDs> {
    fn layout(self) -> FontIter<'t, LayoutGlyphs<'t, GlyphIDs>> {
           FontIter{font: self.font, iter: self.
            iter.scan((None,0),{let font = self.font; move |(last_glyph_id, pen), glyph_id| {
                if let Some(last_glyph_id) = *last_glyph_id { *pen += font.glyphs_kerning(last_glyph_id, glyph_id).unwrap_or(0) as i32; }
                *last_glyph_id = Some(glyph_id);
                let next = (*pen, glyph_id);
                *pen += font.glyph_hor_advance(glyph_id)? as i32;
                Some(next)
            }}).filter_map({let font = self.font; move |(pen, glyph_id)| { Some((pen, glyph_id, font.glyph_bounding_box(glyph_id)?)) }})
        }
    }
}

#[derive(Default)] pub struct LineMetrics {pub width: u32, pub ascent: i16, pub descent: i16}
impl LineMetrics { pub fn height(&self) -> u16 { (self.ascent-self.descent) as u16 } }
impl<I:Iterator> FontIter<'_, I> where I::Item:std::borrow::Borrow<(i32,GlyphId,Rect)> {
    fn metrics(self) -> LineMetrics {
        let font = &self.font;
        self.iter.fold(Default::default(), |metrics:LineMetrics, item| {
            use std::borrow::Borrow;
            let &(pen, glyph_id, bbox) = item.borrow();
            LineMetrics{
                width: (pen + font.glyph_hor_side_bearing(glyph_id).unwrap() as i32 + bbox.x_max as i32) as u32,
                ascent: max(metrics.ascent, bbox.y_max),
                descent: min(metrics.descent, bbox.y_min)
            }
        })
    }
}

#[derive(Clone,Copy)] pub struct Scale(u32, u32);
impl Scale {
    fn floor(self, x: i32) -> i32 { sign(x)*floor_div(x.abs() as u32*self.0 as u32, self.1 as u32) as i32 }
    fn ceil(self, x: i32) -> i32 { sign(x)*ceil_div(x.abs() as u32*self.0 as u32, self.1 as u32) as i32 }
}
impl std::ops::Mul<u32> for Scale { type Output=u32; fn mul(self, b: u32) -> Self::Output { floor_div(b*self.0 as u32, self.1 as u32) } }
//impl std::ops::Mul<u16> for Scale { type Output=u16; fn mul(self, b: u16) -> Self::Output { (self*(b as u32)) as u16 } }
impl std::ops::Mul<f32> for Scale { type Output=f32; fn mul(self, b: f32) -> Self::Output { b*(self.0 as f32)/(self.1 as f32) } }
impl std::ops::Mul<uint2> for Scale { type Output=uint2; fn mul(self, b: uint2) -> Self::Output { uint2{x:self*b.x, y:self*b.y} } }

mod raster;
pub fn line(target : &mut Image<&mut [f32]>, p0 : vec2, p1 : vec2) { raster::line(target, p0.x, p0.y, p1.x, p1.y) }

struct Outline { scale : Scale, x_min: i32, y_max: i32, target : Image<Vec<f32>>, first : Option<vec2>, p0 : Option<vec2>}
impl Outline {
    fn new(scale : Scale, rect : Rect) -> Self {
        let x_min = scale.floor(rect.x_min as i32);
        let y_max = scale.ceil(rect.y_max as i32);
        let size = size2{x: (scale.ceil(rect.x_max as i32)-x_min) as u32,
                                   y: (y_max-scale.floor(rect.y_min as i32)) as u32+1};
        Self{scale, x_min, y_max, target: Image::new(size, vec![0.; (size.x*size.y) as usize]), first:None, p0:None}
    }
    fn map(&self, x : f32, y : f32) -> vec2 { vec2{x: self.scale*x-(self.x_min as f32), y: (self.y_max as f32)-self.scale*y} }
}
impl ttf_parser::OutlineBuilder for Outline {
    fn move_to(&mut self, x: f32, y: f32) { assert!(self.first.is_none() && self.p0.is_none()); self.first = Some(self.map(x,y)); self.p0 = self.first; }
    fn line_to(&mut self, x: f32, y: f32) { let p1 = self.map(x,y); line(&mut self.target.as_mut(), self.p0.unwrap(), p1); self.p0 = Some(p1); }
    fn quad_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32) {
        let p0 = self.p0.unwrap();
        let p1 = self.map(x1, y1);
        let p2 = self.map(x2, y2);
        let dev = sq(p0 - 2.*p1 + p2);
        if dev < 1./3. { line(&mut self.target.as_mut(), p0, p2); }
        else {
            let tol = 3.;
            let n = 1 + (tol * dev).sqrt().sqrt().floor() as usize;
            let rcp_n = 1./(n as f32);
            let mut t = 0.;
            let mut p = p0;
            for _ in 0..n-1 {
                t += rcp_n;
                let pn = lerp(t, lerp(t, p0, p1), lerp(t, p1, p2));
                line(&mut self.target.as_mut(), p, pn);
                p = pn;
            }
            line(&mut self.target.as_mut(), p, p2);
        }
        self.p0 = Some(p2);
    }
    fn curve_to(&mut self, _: f32, _: f32, _: f32, _: f32, _: f32, _: f32) { unimplemented!(); }
    fn close(&mut self) { line(&mut self.target.as_mut(), self.p0.unwrap(), self.first.unwrap()); self.first = None; self.p0 = None; }
}

pub struct Text<'font, 'text> {
    font : &'font Font<'font>,
    text : &'text str,
    size : Option<size2>
}
impl<'font, 'text> Text<'font, 'text> {
    pub fn new(text : &'text str) -> Self {
        rental! { mod rent {
            #[rental(covariant)]
            pub struct MapFont {
                map: Box<memmap::Mmap>,
                font: super::Font<'map>
            }
        } } use rent::MapFont;
        pub fn from_file(path: &str) -> Result<MapFont> {
            Ok(MapFont::new(box unsafe{memmap::Mmap::map(&std::fs::File::open(path)?)}?, |map| Font(ttf_parser::Font::from_data(map, 0).unwrap())))
        }
        lazy_static! { default_font : MapFont = from_file("/usr/share/fonts/noto/NotoSans-Regular.ttf").unwrap(); }
        Self{font: default_font.suffix(), text, size: None}
    }
    pub fn size(&mut self) -> size2 {
        let Self{font, text, ref mut size} = self;
        *size.get_or_insert_with(||{
            let (count, max_width) = text.lines().map(|line| font.glyphs(line.chars()).layout().metrics()).fold((0,0),|(count, width), line| (count+1, max(width, line.width)));
            size2{x: max_width, y: count * (font.height() as u32)}
        })
    }
    pub fn render(&self, target : &mut Image<&mut[bgra8]>, scale: Scale) {
        self.text.lines().enumerate().for_each(|(line_index,line)| self.font.glyphs(line.chars()).layout().by_ref().for_each(|(pen, glyph_id, bbox)| {
            let mut outline = Outline::new(scale, bbox);
            self.font.outline_glyph(glyph_id, &mut outline).unwrap();
            let coverage = raster::fill(&outline.target.as_ref());
            target.slice_mut(scale*uint2{x: (pen+self.font.glyph_hor_side_bearing(glyph_id).unwrap() as i32) as u32,
                                                          y: (line_index as u32)*(self.font.height() as u32) + (self.font.ascender()-bbox.y_max) as u32}, coverage.size)
                .map(coverage, |_,c| bgra8{a : 0xFF, ..sRGB(c).into()})
        }))
    }
}

fn fit_width(width: u32, from : size2) -> size2 { size2{x: width, y: ceil_div(width * from.y, from.x)} }

use crate::{/*text::Text,*/window::{Widget, Target}};
impl Widget for Text<'_,'_> {
    fn size(&mut self, size : size2) -> size2 { fit_width(size.x, self.size()) }
    fn render(&mut self, target : &mut Target) {
        let scale = Scale(target.size.x-1, self.size().x-1); // todo: scroll
        Text::render(&self, target, scale)
    }
}
