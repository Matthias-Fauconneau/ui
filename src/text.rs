use std::cmp::{min, max};
use crate::assert;
use crate::core::{Result, sign, abs, sq, size2, offset2};
pub fn floor_div(n : u32, d : u32) -> u32 { n/d }
pub fn ceil_div(n : u32, d : u32) -> u32 { (n+d-1)/d }
use crate::image::{Image, bgra8, IntoPixelIterator, sRGB};

pub struct Font(memmap::Mmap);
impl Font {
    pub fn map() -> Result<Self> { Ok(Font(unsafe{memmap::Mmap::map(&std::fs::File::open("/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf")?)}?)) }
    pub fn parse(&self) -> Result<ttf_parser::Font> { Ok(ttf_parser::Font::from_data(&self.0, 0)?) }
}

// Dummy to get rect
struct Builder();
impl ttf_parser::OutlineBuilder for Builder {
    fn move_to(&mut self, _: f32, _: f32) {}
    fn line_to(&mut self, _: f32, _: f32) {}
    fn quad_to(&mut self, _: f32, _: f32, _: f32, _: f32) {}
    fn curve_to(&mut self, _: f32, _: f32, _: f32, _: f32, _: f32, _: f32) { unimplemented!(); }
    fn close(&mut self) {}
}

pub struct Metrics {pub width: u32, pub ascent: i16, pub descent: i16}
pub fn metrics(font : &Font, text: &str) -> Result<Metrics> {
    let (mut line_metrics, mut pen, mut last_glyph_index) = (Metrics{width: 0, ascent:0, descent:0}, 0, None);
    let font = font.parse()?;
    for c in text.chars() {
        let glyph_index = font.glyph_index(c)?;
        if let Some(last_glyph_index) = last_glyph_index { pen += font.glyphs_kerning(last_glyph_index, glyph_index).unwrap_or(0) as i32; }
        last_glyph_index = Some(glyph_index);
        let metrics = font.glyph_hor_metrics(glyph_index)?;
        if let Ok(rect) = font.outline_glyph(glyph_index, &mut Builder()) {
            line_metrics.width = (pen + metrics.left_side_bearing as i32 + rect.x_max as i32) as u32;
            line_metrics.ascent = max(line_metrics.ascent, rect.y_max);
            line_metrics.descent = min(line_metrics.descent, rect.y_min);
        };
        pen += metrics.advance as i32;
    }
    Ok(line_metrics)
}
impl Metrics {
    pub fn height(&self) -> u16 { (self.ascent-self.descent) as u16 }
}

#[derive(Clone,Copy)] struct Scale(u16, u16);
impl Scale {
    fn floor(self, x: i16) -> i16 { sign(x)*floor_div(x.abs() as u32*self.0 as u32, self.1 as u32) as i16 }
    fn ceil(self, x: i16) -> i16 { sign(x)*ceil_div(x.abs() as u32*self.0 as u32, self.1 as u32) as i16 }
}
impl std::ops::Mul<u32> for Scale { type Output=u32; fn mul(self, b: u32) -> Self::Output { floor_div(b*self.0 as u32, self.1 as u32) } }
impl std::ops::Mul<u16> for Scale { type Output=u16; fn mul(self, b: u16) -> Self::Output { (self*(b as u32)) as u16 } }
impl std::ops::Mul<f32> for Scale { type Output=f32; fn mul(self, b: f32) -> Self::Output { b*(self.0 as f32)/(self.1 as f32) } }

pub fn line(target : &mut Image<&mut [f32]>, p0 : vec2, p1 : vec2) { crate::raster::line(target, p0.x, p0.y, p1.x, p1.y) }

struct Outline { scale : Scale, x_min: i16, y_max: i16, target : Image<Vec<f32>>, first : Option<vec2>, p0 : Option<vec2>}
impl Outline {
    /*fn new(scale : Scale, rect : ttf_parser::Rect) -> Self {
        Self{scale, x_min: rect.x_min, y_max: rect.y_max, target: Image::zero(size2{x: scale.ceil((rect.x_max-rect.x_min) as u16) as u32 + 2,
                                                                                                                                      y: scale.ceil((rect.y_max-rect.y_min) as u16) as u32 + 3}), first:None, p0:None}
    }
    fn map(&self, x : f32, y : f32) -> vec2 { vec2{x: self.scale*(x-(self.x_min as f32))+1., y: self.scale*((self.y_max as f32)-y)+1.} }*/
    fn new(scale : Scale, rect : ttf_parser::Rect) -> Self {
        let x_min = scale.floor(rect.x_min);
        let y_max = scale.ceil(rect.y_max);
        let size = size2{x: (scale.ceil(rect.x_max)-x_min) as u32,
                                   y: (y_max-scale.floor(rect.y_min)) as u32+1};
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
        let dev = sq(p0 - float(2.) * p1 + p2);
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

pub fn text(target : &mut Image<&mut[bgra8]>, font : &Font, text: &str) -> Result<()> {
    let line_metrics = metrics(font, text)?;
    let font = font.parse()?;
    const N: u32 = 1; //16; // undersample rasterization for debug visualization
    let scale = Scale((target.size.y/N-1) as u16, line_metrics.height()-1);
    let (mut pen, mut last_glyph_index) = (0, None);
    for c in text.chars() {
        let glyph_index = font.glyph_index(c)?;
        if let Some(last_glyph_index) = last_glyph_index { pen += font.glyphs_kerning(last_glyph_index, glyph_index).unwrap_or(0) as i32; }
        last_glyph_index = Some(glyph_index);
        let metrics = font.glyph_hor_metrics(glyph_index)?;
        if let Ok(rect) = font.outline_glyph(glyph_index, &mut Builder()) {
            let mut outline = Outline::new(scale, rect);
            font.outline_glyph(glyph_index, &mut outline)?;
            let coverage = crate::raster::fill(&outline.target.as_ref());
            if let Some(last_glyph_index) = last_glyph_index { pen += font.glyphs_kerning(last_glyph_index, glyph_index).unwrap_or(0) as i32; }
            //let target = target.slice_mut(offset2{x: scale*((pen+metrics.left_side_bearing as i32) as u32), y: (scale*((line_metrics.ascent-rect.y_max) as u16)) as u32}, coverage.size)?;
            let mut target = target.slice_mut(offset2{x: scale*((pen+metrics.left_side_bearing as i32) as u32)*N, y: ((scale*((line_metrics.ascent-rect.y_max) as u16)) as u32)*N},
                                                                       size2{x: coverage.size.x*N, y: coverage.size.y*N})?;
            if N==1 {
                 for (&c, target) in (coverage.as_ref(), target).pixels() {
                    assert!(0. <= c && c <= 1., c);
                    let a = sRGB(f32::min(abs(c),1.));
                    *target = bgra8{b : a, g : a, r : a, a : 0xFF};
                }
            } else {
                for y in 0..target.size.y { for x in 0..target.size.x {
                    let c = coverage.as_ref().get(x/N,y/N);
                    //assert!(c >= 0. && c<= 1., c);
                    let a = sRGB(f32::min(abs(c),1.));
                    target.set(x,y, if c>0. { bgra8{b : 0, g : a, r : a, a : 0xFF} } else { bgra8{b : a, g : a, r : 0, a : 0xFF} } );
                }}
            }
        }
        pen += metrics.advance as i32;
    }
    Ok(())
}

use std::rc::Rc;
//use text::{Font, metrics, ceil_div, text};
use crate::window::{Widget, Target};

pub struct Text {
    pub font : Rc<Font>,
    pub text : String
}
impl Widget for Text {
    fn size(&mut self, size : size2) -> size2 {
        let metrics = metrics(&self.font, &self.text).unwrap();
        size2{x: size.x, y: ceil_div(size.x*(metrics.height()-1) as u32, metrics.width)+1}
    }
    fn render(&mut self, target : &mut Target) -> Result { text(target, &self.font, &self.text) }
}
