use crate::assert;
use crate::core::{floor, sq, fract, abs};
use crate::image::{size2, Image, IntoRows};

impl<T:Copy> Image<&[T]> {
    pub fn get(&self, x : u32, y: u32) -> T { self.buffer[(y*self.stride+x) as usize] }
}
impl<T:Copy> Image<&mut [T]> {
    pub fn set(&mut self, x : u32, y: u32, v: T) { self.buffer[(y*self.stride+x) as usize] = v; }
}

// Rasterize polygon with analytical coverage
impl Image<Vec<f32>> {
    fn major_x_inc_y(&mut self, dᵧx : f32, dx : f32, x : u32, y0 : f32, y1 : f32) {
        let target = &mut self.buffer[(x+(y0 as u32)*self.stride) as usize..];
        let w = self.stride as usize;
        if floor(y0) == floor(y1) {
            let c = (y0+y1)/2. - floor(y0); // Trapeze
            assert!(c >= 0., c);
            target[0]+=dx* (1.-c);
            target[w]+=dx* c;
        } else {
            let a0 = (sq(1.-fract(y0))*dᵧx)/2.; // Triangle
            let a1 = (sq(fract(y1))*dᵧx)/2.; // Triangle
            assert!(a0 > 0. && 1.-a0-a1 > 0. && a1 > 0.);
            target[      0]+=dx* a0;
            target[     w]+=dx* (1.-a0-a1);
            target[w+w]+=dx* a1;
        }
    }

    fn major_x_dec_y(&mut self, dᵧx : f32, dx : f32, x : u32, y0 : f32, y1 : f32) {
        let target = &mut self.buffer[(x+(y1 as u32)*self.stride) as usize..];
        let w = self.stride as usize;
        if floor(y0) == floor(y1) {
            let c = (y0+y1)/2. - floor(y0); // Trapeze
            assert!(c >= 0., c);
            target[0]+=dx* (1.-c);
            target[w]+=dx* c;
        } else {
            let a0 = (sq(fract(y0))*dᵧx)/2.; // Triangle
            let a1 = (sq(1.-fract(y1))*dᵧx)/2.; // Triangle
            assert!(a0 > 0. && 1.-a0-a1 > 0. && a1 > 0., (a0, a1, y0, y1, dᵧx));
            target[0]+=dx* a0;
            target[     w]+=dx* (1.-a0-a1);
            target[w+w]+=dx* a1;
        }
    }

    fn major_y_inc_x(&mut self, dᵧx : f32, dₓy : f32, dy : f32, y : u32, x0 : f32, x1 : f32) {
        let target = &mut self.buffer[(y*self.stride + x0 as u32) as usize..];
        let w = self.stride as usize;
        if floor(x0) == floor(x1) {
            let c = dᵧx/2.; // Triangle
            assert!(c >= 0., c);
            target[0]+=dy* c;
            target[w]+=dy* c;
        } else {
            let a0 = (sq(1.-fract(x0))*dₓy)/2.; // Triangle
            let a1 = (sq(fract(x1))*dₓy)/2.; // Triangle
            assert!(a0 > 0. && 1.-a0-a1 > 0. && a1 > 0.);
            target[     0]+=dy* ((1.-fract(x0))*1. - a0); // Rectangle - Triangle
            target[     1]+=dy* a1;
            target[w    ]+=dy* a0;
            target[w+1]+=dy* (fract(x1)*1. - a1); // Rectangle - Triangle
        }
    }

    fn major_y_dec_x(&mut self, dᵧx : f32, dₓy : f32, dy : f32, y : u32, x0 : f32, x1 : f32) {
        let target = &mut self.buffer[(y*self.stride + x1 as u32) as usize..];
        let w = self.stride as usize;
        if floor(x0) == floor(x1) {
            let c = dᵧx/2.; // Triangle
            target[0]+=dy* c;
            target[w]+=dy* c;
        } else {
            let a0 = (sq(fract(x0))*dₓy)/2.; // Triangle
            let a1 = (sq(1.-fract(x1))*dₓy)/2.; // Triangle
            assert!(a0 > 0. && 1.-a0-a1 > 0. && a1 > 0., (a0, a1, x1, dₓy));
            target[     0]+=dy* a1;
            target[     1]+=dy* (fract(x0)*1. - a0); // Rectangle - Triangle
            target[w     ]+=dy* ((1.-fract(x1))*1. - a1);
            target[w+1]+=dy* a0; // Rectangle - Triangle
        }
    }

    pub fn line_xy(&mut self, x0: f32, y0: f32, x1: f32, y1: f32) {
        let δx = x1 - x0;
        if δx==0. { return; }
        let δy = y1 - y0;
        let abs_δx = abs(δx);
        let abs_δy = abs(δy);
        if abs_δx > abs_δy { // Major x
            let abs_dᵧx : f32 = abs_δx / abs_δy; // |dᵧx|>1
            let               (dx, x0, y0, x1, y1, δy) =
            if δx >= 0. {( 1., x0, y0, x1, y1, δy)}
            else           {(-1., x1, y1, x0, y0, -δy)};
            let dₓy : f32 = δy / abs_δx; // |dₓy|<1
            let mut y = y0 + dₓy*(1.-fract(x0));
            if δy >= 0. {
                self.major_x_inc_y(abs_dᵧx, dx*(1.-fract(x0)), x0 as u32, y0, y);
                for x in (x0 as u32)+1..(x1 as u32) {
                    let y0 = y;
                    y += dₓy;
                    self.major_x_inc_y(abs_dᵧx, dx, x, y0, y);
                }
                self.major_x_inc_y(abs_dᵧx, dx*fract(x1), x1 as u32, y, y1);
            } else {
                self.major_x_dec_y(abs_dᵧx, dx*(1.-fract(x0)), x0 as u32, y0, y);
                for x in (x0 as u32)+1..(x1 as u32) {
                    let y0 = y;
                    y += dₓy;
                    self.major_x_dec_y(abs_dᵧx, dx, x, y0, y);
                }
                self.major_x_dec_y(abs_dᵧx, dx*fract(x1), x1 as u32, y, y1);
            }
        } else { // Major y, |dᵧx|<1, |dₓy|>1
            let abs_dₓy = abs_δy / abs_δx;
            let sign = if δx >= 0. { 1. } else { -1. };
            let                (x0, y0, x1, y1, δx) =
            if δy >= 0. {(x0, y0, x1, y1, δx)}
            else           {(x1, y1, x0, y0, -δx)};
            let dᵧx = δx / abs_δy;
            let mut x = x0 + dᵧx*(1.-fract(y0));
            if δx >= 0. {
                self.major_y_inc_x(abs(dᵧx), abs_dₓy, sign*(1.-fract(y0)), y0 as u32, x0, x);
                for y in (y0 as u32)+1..(y1 as u32) {
                    let x0 = x;
                    x += dᵧx;
                    self.major_y_inc_x(abs(dᵧx), abs_dₓy, sign, y, x0, x);
                }
                self.major_y_inc_x(abs(dᵧx), abs_dₓy, sign*fract(y1), y1 as u32, x, x1);
            } else {
                self.major_y_dec_x(abs(dᵧx), abs_dₓy, sign*(1.-fract(y0)), y0 as u32, x0, x);
                for y in (y0 as u32)+1..(y1 as u32) {
                    let x0 = x;
                    x += dᵧx;
                    self.major_y_dec_x(abs(dᵧx), abs_dₓy, sign, y, x0, x);
                }
                self.major_y_dec_x(abs(dᵧx), abs_dₓy, sign*fract(y1), y1 as u32, x, x1);
            }
        }
    }

    pub fn fill(&self) -> Image<Vec<f32>> {
        let size = size2{x:self.size.x-2, y:self.size.y-3};
        let mut target = Image::uninitialized(size);
        for x in 0..size.x { target.as_mut().set(x,0, self.as_ref().get(1+x,0)+self.as_ref().get(1+x,1)); }
        for y in 1..size.y { for x in 0..size.x { let v = target.as_ref().get(x,y-1) + self.as_ref().get(1+x,1+y); target.as_mut().set(x,y, v); } }
        //for y in 1..size.y { for x in 0..size.x { let v = self.as_ref().get(1+x,1+y); target.as_mut().set(x,y, v); } }
        /*{
            let mut last = target.as_ref().rows();
            let mut source = self.as_ref().rows();
            let end = target.as_ref().end();
            let mut target = target.as_mut().rows();
            for x in 0..size.x { target[x] = source[x]; }
            source.next();
            target.next();
            while target.ptr as *const f32 != end {
                //for x in 0..size.x { target[x] = last[x] + source[x]; }
                for x in 0..size.x { target[x] = source[x]; }
                last.next();
                source.next();
                target.next();
            }
        }*/
        target
    }
}
