use crate::{vector::size2, image::Image};

/*use crate::core::{floor, sq, fract, abs};
use crate::image::IntoRows;

// Rasterize polygon with analytical coverage
pub fn line(target : &mut Image<&mut [f32]>, x0: f32, y0: f32, x1: f32, y1: f32) {
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
            let target = &mut target.buffer[(x+(y0 as u32)*target.stride) as usize..];
            if floor(y0) == floor(y1) {
                let c = (y0+y1)/2. - floor(y0);
                assert!(c >= 0., c);
                target[0]+=dx* (1.-c);
                target[w]+=dx* c;
            } else {
                let a = (sq(1.-fract(y0))*dᵧx)/2.;
                let b = (sq(fract(y1))*dᵧx)/2.;
                assert!(a > 0. && 1.-a-b > 0. && b > 0.);
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
            let target = &mut target.buffer[(y*target.stride + x0 as u32) as usize..];
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
                assert!(0. < a && a < 1. && 0. < b && b < 1. && 0. < ca && ca < 1. && 0. < cb && cb < 1.);
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
    let size = size2{x:edges.size.x-2, y:edges.size.y-3};
    let mut target = Image::uninitialized(size);
    {
        let mut last = target.as_ref().rows();
        let mut edges = edges.rows();
        let end = target.as_ref().end();
        let mut target = target.as_mut().rows_mut();
        for x in 0..size.x { target[x] = edges[x]; }
        edges.next();
        target.next();
        while target.ptr as *const f32 != end {
            for x in 0..size.x { target[x] = last[x] + edges[x]; }
            last.next();
            edges.next();
            target.next();
        }
    }
    target
}*/

pub fn line(target : &mut Image<&mut [f32]>, x0: f32, y0: f32, x1: f32, y1: f32) {
    //let _ = scopeguard::guard_on_unwind(target.size,|size| eprintln!("{:?}", size));
    if y0 == y1 { return; }
    let (dir, x0, y0, x1, y1) = if y0 < y1 { (1., x0, y0, x1, y1) } else { (-1., x1, y1, x0, y0) };
    let dxdy = (x1-x0)/(y1-y0);
    let mut x = x0;
    // http://www.apache.org/licenses/LICENSE-2.0. Modified from https://github.com/raphlinus/font-rs
    for y in y0 as u32..y1.ceil() as u32 {
        let line = &mut target.buffer[(y*target.stride) as usize..];
        let dy = ((y + 1) as f32).min(y1) - (y as f32).max(y0);
        let xnext = x + dxdy * dy;
        let d = dy * dir;
        let (x0, x1) = if x < xnext { (x, xnext) } else { (xnext, x) };
        let x0floor = x0.floor();
        let x0i = x0floor as i32;
        let x1ceil = x1.ceil();
        let x1i = x1ceil as i32;
        let mut add = |x, v| { if (x as usize) < line.len() { line[x as usize] += v; } }; // FIXME
        if x1i <= x0i + 1 {
            let xmf = 0.5 * (x + xnext) - x0floor;
            //assert!((x0i as usize) < line.len(), "{:?} {} {}", target.size, line.len(), x0i);
            add(x0i, d - d * xmf);
            //assert!(((x0i+1) as usize) < line.len(), "{:?} {} {}", target.size, line.len(), x0i);
            add(x0i + 1, d * xmf);
        } else {
            //assert!(x0 >= 0. && x0i >= 0, (x0, x1, x, xnext, x0floor, x0i, x1ceil, x1i));
            let s = 1./(x1 - x0);
            let x0f = x0 - x0floor;
            let a0 = 0.5 * s * (1.0 - x0f) * (1.0 - x0f);
            let x1f = x1 - x1ceil + 1.0;
            let am = 0.5 * s * x1f * x1f;
            add(x0i, d * a0);
            if x1i == x0i + 2 {
                add(x0i + 1, d * (1.0 - a0 - am));
            } else {
                let a1 = s * (1.5 - x0f);
                add(x0i + 1, d * (a1 - a0));
                for xi in x0i + 2..x1i - 1 {
                    add(xi, d * s);
                }
                let a2 = a1 + (x1i - x0i - 3) as f32 * s;
                add(x1i - 1, d * (1.0 - a2 - am));
            }
            add(x1i, d * am);
        }
        x = xnext;
    }
}

pub fn fill(edges : &Image<&[f32]>) -> Image<Vec<f32>> {
    Image::new(size2{x: edges.size.x, y: edges.size.y-1},
        edges.buffer[0..((edges.size.y-1)*edges.size.x) as usize].iter().scan(0.,|acc, &a| {
            *acc += a;
            Some(acc.abs().min(1.0))
            //assert!(0. <= *acc && *acc <= 1., *acc);
            //Some(*acc)
        }).collect() )
}
