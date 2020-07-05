/// Rasterize polygon with analytical coverage
use crate::{vector::{vec2, size2}, image::Image};

/*use {crate::num::{floor, sq, fract, abs}, vector::xy};
#[track_caller] pub fn line(target : &mut Image<&mut [f32]>, xy{x:x0,y:y0}: vec2, xy{x:x1,y:y1}: vec2) {
	crate::assert!(x0 >= 0. && x1 >= 0. && y0 >= 0. && y1 >= 0., x0, x1, y0, y1);
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
        let dy = δy>=0.;
        fn major_x(target : &mut Image<&mut [f32]>, dy : bool, dᵧx : f32, dx : f32, x : u32, y0 : f32, y1 : f32) {
            let (y0, y1) = if dy { (y0, y1) } else { (y1, y0) };
            let w = target.stride as usize;
            let target = &mut target.data[(x+(y0 as u32)*target.stride) as usize..];
            if floor(y0) == floor(y1) {
                let c = (y0+y1)/2. - floor(y0);
                assert!(c >= 0., c);
                target[0]+=dx* (1.-c);
                target[w]+=dx* c;
            } else {
                let a = (sq(1.-fract(y0))*dᵧx)/2.;
                let b = (sq(fract(y1))*dᵧx)/2.;
                crate::assert!(a > 0. && 1.-a-b > 0. && b >= 0., a,b,y0,y1);
                target[      0]+=dx* a;
                target[     w]+=dx* (1.-a-b);
                target[w+w]+=dx* b;
            }
        }
        major_x(target, dy, abs_dᵧx, dx*(1.-fract(x0)), x0 as u32, y0, y);
        for x in (x0 as u32)+1..(x1 as u32) {
            let y0 = y;
            y += dₓy;
            major_x(target, dy, abs_dᵧx, dx, x, y0, y);
        }
        major_x(target, dy, abs_dᵧx, dx*fract(x1), x1 as u32, y, y1);
    } else { // Major y, |dᵧx|<1, |dₓy|>1
        let abs_dₓy = abs_δy / abs_δx;
        let sign = if δx >= 0. { 1. } else { -1. };
        let                (x0, y0, x1, y1, δx) =
        if δy >= 0. {(x0, y0, x1, y1, δx)}
        else           {(x1, y1, x0, y0, -δx)};
        let dᵧx = δx / abs_δy;
        let mut x = x0 + dᵧx*(1.-fract(y0));
        let dx = δx>=0.;
        fn major_y(target : &mut Image<&mut [f32]>, dx : bool, dᵧx : f32, dₓy : f32, dy : f32, y : u32, x0 : f32, x1 : f32) {
            let (x0, x1) = if dx { (x0, x1) } else { (x1, x0) };
            let w = target.stride as usize;
            let target = &mut target.data[(y*target.stride + x0 as u32) as usize..];
            if floor(x0) == floor(x1) {
                let c = dᵧx/2.;
                assert!(c >= 0., c);
                target[0]+=dy* c;
                target[w]+=dy* c;
            } else {
                let a = (sq(1.-fract(x0))*dₓy)/2.;
                let b = (sq(fract(x1))*dₓy)/2.;
                let ca = 1. - fract(x0) - a;
                let cb = fract(x1) - b;
                crate::assert!(0. < a && a < 1. && 0. <= b && b < 1. && 0. < ca && ca < 1. && 0. <= cb && cb < 1., a,b,ca,cb,x0,x1,dₓy);
                let (ca, a, b, cb) = if dx { (ca, a, b, cb) } else { (a, ca, cb, b) };
                target[     0]+=dy* ca;
                target[w    ]+=dy* a;
                target[     1]+=dy* b;
                target[w+1]+=dy* cb;
            }
        }
        major_y(target, dx, abs(dᵧx), abs_dₓy, sign*(1.-fract(y0)), y0 as u32, x0, x);
        for y in (y0 as u32)+1..(y1 as u32) {
            let x0 = x;
            x += dᵧx;
            major_y(target, dx, abs(dᵧx), abs_dₓy, sign, y, x0, x);
        }
        major_y(target, dx, abs(dᵧx), abs_dₓy, sign*fract(y1), y1 as u32, x, x1);
    }
}

pub fn fill(edges : &Image<&[f32]>) -> Image<Vec<f32>> {
    let mut target = Image::uninitialized(size2{x:edges.size.x, y:edges.size.y-2});
    {
        let mut edges = edges.rows(0..edges.size.y-2);
        let mut target = target.rows_mut(0..target.size.y);
        let mut last = {
			let (target, edges) = (target.next().unwrap(), edges.next().unwrap());
			for (target, &edge) in target.iter_mut().zip(edges.iter()) { *target = 0. + edge; }
			target
		};
        for (target, edges) in target.zip(edges) {
            for ((target, &last), &edge) in target.iter_mut().zip(last.iter()).zip(edges.iter()) { *target = last + edge; }
            last = target;
        }
    }
    target
}*/

pub fn line(target : &mut Image<&mut [f32]>, p0: vec2, p1: vec2) {
    #[allow(clippy::float_cmp)] if p0.y == p1.y { return; }
    let (dir, x0, y0, x1, y1) = if p0.y < p1.y { (1., p0.x, p0.y, p1.x, p1.y) } else { (-1., p1.x, p1.y, p0.x, p0.y) };
    let dxdy = (x1-x0)/(y1-y0);
    let mut x = x0;
    // Modified from https://github.com/raphlinus/font-rs Apache 2
    //for (y, row) in target.rows_mut(y0 as u32..y1.ceil() as u32).enumerate() {
    for y in y0 as u32..(y1.ceil() as u32).min(target.size.y) { let row = &mut target.data[(y*target.stride) as usize..]; // May access first column of next line
        let dy = ((y + 1) as f32).min(y1) - (y as f32).max(y0);
        crate::assert!(x + dxdy * dy >= 0., p0, p1, dxdy, dy, dxdy * dy, x);
        let xnext = x + dxdy * dy;
        let d = dy * dir;
        let (x0, x1) = if x < xnext { (x, xnext) } else { (xnext, x) };
        let x0floor = x0.floor();
        let x0i = x0floor as u32;
        let x1ceil = x1.ceil();
        let x1i = x1ceil as u32;
        if x1i <= x0i + 1 {
            let xmf = (x + xnext) / 2. - x0floor;
            //assert!((x0i as usize) < line.len(), "{:?} {} {}", target.size, line.len(), x0i);
            row[x0i as usize] += d - d * xmf;
            //assert!(((x0i+1) as usize) < line.len(), "{:?} {} {}", target.size, line.len(), x0i);
            row[(x0i + 1) as usize] += d * xmf;
        } else {
            //assert!(x0 >= 0. && x0i >= 0, (x0, x1, x, xnext, x0floor, x0i, x1ceil, x1i));
            let s = 1./(x1 - x0);
            let x0f = x0 - x0floor;
            let a0 = s / 2. * (1. - x0f) * (1. - x0f);
            let x1f = x1 - x1ceil + 1.;
            let am = s / 2. * x1f * x1f;
            row[x0i as usize] += d * a0;
            if x1i == x0i + 2 {
                row[(x0i + 1) as usize] += d * (1. - a0 - am);
            } else {
                let a1 = s * (3./2. - x0f);
                row[(x0i + 1) as usize] += d * (a1 - a0);
                for xi in x0i + 2..x1i - 1 {
                    row[xi as usize] += d * s;
                }
                let a2 = a1 + (x1i - x0i - 3) as f32 * s;
                row[(x1i - 1) as usize] += d * (1.0 - a2 - am);
            }
            row[x1i as usize] += d * am;
        }
        x = xnext;
    }
}

pub fn fill(edges : &Image<&[f32]>) -> Image<Vec<f32>> {
    let mut target = Image::uninitialized(size2{x:edges.size.x-1, y:edges.size.y-1});
	for (target, edges) in target.rows_mut(0..target.size.y).zip(edges.rows(0..edges.size.y-1)) {
		let mut coverage = 0.;
		for (target, &edge) in target.iter_mut().zip(edges.iter().skip(1)) {
			coverage += edge;
			crate::assert!(-0.0000004 <= coverage && coverage <= 1.0000004, coverage);
			*target = crate::num::clamp(coverage);
		}
	}
    target
}
