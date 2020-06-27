use {std::cmp::{min, max}, crate::{num::{sign,floor_div,ceil_div}, error::{Error,throws}, vector::{uint2,size2,vec2,lerp,sq}}, ttf_parser::{GlyphId,Rect}};

use derive_more::Deref; //From
#[derive(Deref)]
pub struct Font<'t>(ttf_parser::Font<'t>);

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

#[derive(Clone,Copy)] pub struct Scale{pub num: u32, pub div:u32}
impl Scale {
    fn floor(self, x: i32) -> i32 { sign(x)*floor_div(x.abs() as u32*self.num, self.div) as i32 }
    fn ceil(self, x: i32) -> i32 { sign(x)*ceil_div(x.abs() as u32*self.num, self.div) as i32 }
}
impl std::ops::Mul<f32> for Scale { type Output=f32; fn mul(self, b: f32) -> Self::Output { b*(self.num as f32)/(self.div as f32) } }
impl std::ops::Mul<u32> for Scale { type Output=u32; fn mul(self, b: u32) -> Self::Output { floor_div(b*self.num, self.div) } }
impl std::ops::Mul<uint2> for Scale { type Output=uint2; fn mul(self, b: uint2) -> Self::Output { uint2{x:self*b.x, y:self*b.y} } }

use super::raster::{self, Image};
pub fn line(target : &mut Image<&mut [f32]>, p0 : vec2, p1 : vec2) { raster::line(target, p0.x, p0.y, p1.x, p1.y) }

pub struct Outline { scale : Scale, x_min: i32, y_max: i32, target : Image<Vec<f32>>, first : Option<vec2>, p0 : Option<vec2>}
impl Outline {
    pub fn new(scale: Scale, bbox: Rect) -> Self {
        let x_min = scale.floor(bbox.x_min as i32);
        let y_max = scale.ceil(bbox.y_max as i32);
        let size = size2{
            x: (scale.ceil(bbox.x_max as i32)-x_min) as u32,
            y: (y_max-scale.floor(bbox.y_min as i32)) as u32+1
        };
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
impl<'t> Font<'t> {
    pub fn rasterize(&self, scale: Scale, glyph_id: GlyphId, bbox: Rect) -> Image<Vec<f32>> {
        let mut outline = Outline::new(scale, bbox);
        self.outline_glyph(glyph_id, &mut outline).unwrap();
        raster::fill(&outline.target.as_ref())
    }
}

pub struct Handle<'t>(Font<'t>); // OwningHandle forwards deref, but Font derefs to ttf_parser::Font, while we want OwningHandle to deref to Font not ttf_parser::Font
impl<'t> std::ops::Deref for Handle<'t> { type Target = Font<'t>; fn deref(&self) -> &Self::Target{ &self.0 } }
#[throws] pub fn from_file(path: &std::path::Path) -> owning_ref::OwningHandle<Box<memmap::Mmap>, Handle> {
	owning_ref::OwningHandle::new_with_fn(
		Box::new(unsafe{memmap::Mmap::map(&std::fs::File::open(path)?)}?),
		unsafe { |map| Handle(Font(ttf_parser::Font::from_data(&*map, 0).unwrap())) }
	)
}
