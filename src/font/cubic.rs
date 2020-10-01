use xy::vec2;

cfg_if::cfg_if! { if #[cfg(feature="kurbo")] {

pub fn cubic(p0: vec2, p1: vec2, p2: vec2, p3: vec2, mut line_to: impl FnMut(vec2)) {
	use kurbo::PathEl::*;
	fn point(vec2{x,y}: vec2) -> kurbo::Point { kurbo::Point{x: x as f64, y: y as f64} }
	let (p0,p1,p2,p3) = (point(p0),point(p1),point(p2),point(p3));
	kurbo::flatten(std::array::IntoIter::new([MoveTo(p0),CurveTo(p1,p2,p3)]), 1./4., |e| if let LineTo(kurbo::Point{x,y}) = e { line_to(vec2{x: x as f32, y: y as f32}); } /*Ignore first MoveTo*/)
}

} else {

use crate::{vector::{vec2, sq}, quad::Quad};

// Modified from https://github.com/linebender/kurbo MIT

pub fn cubic(p0: vec2, p1: vec2, p2: vec2, p3: vec2, mut line_to: impl FnMut(vec2)) {
	// Subdivide into quadratics, and estimate the number of
	// subdivisions required for each, summing to arrive at an
	// estimate for the number of subdivisions for the cubic.
	// Also retain these parameters for later.
	let tolerance = 1./4.;
	let tolerance_sqrt = 1./2.;
	let tolerance_cubic_to_quadratic = 0.1;
	let accuracy = tolerance * tolerance_cubic_to_quadratic;
	let max_hypot2 = 432. * accuracy * accuracy;
	let err = sq((3. * p2 - p3) - (3. * p1 - p0));
	let n = ((err / max_hypot2).powf(1./6.).ceil() as usize).max(1);

	let mut quads = Vec::new(); quads.reserve(n); // FIXME: ArrayVec
	let sqrt_remain_tolerance = tolerance_sqrt * (1. - tolerance_cubic_to_quadratic).sqrt();
	let mut sum = 0.;
	for i in 0..n {
		let t0 = i as f32 / n as f32;
        let t1 = (i + 1) as f32 / n as f32;
        fn cubic(p0: vec2, p1: vec2, p2: vec2, p3: vec2, t: f32) -> vec2 {
			let mt = 1. - t;
			(mt * mt * mt) * p0 + t * ((mt * mt * 3.) * p1 + t * ((mt * 3.) * p2 + t * p3))
		}
        let s0 = cubic(p0, p1, p2, p3, t0);
        let s3 = cubic(p0, p1, p2, p3, t1);
        let scale = (t1 - t0) * (1./3.);
        let d = Quad{p0: 3. * (p1 - p0), p1: 3. * (p2 - p1), p2: 3. * (p3 - p2)};
        let s1 = s0 + scale * d.eval(t0);
        let s2 = s3 - scale * d.eval(t1);
        let q = Quad{p0: s0, p1: (((3. * s1 - s0) + (3. * s2 - s3)) / 4.), p2: s3};
		let subdivision = q.estimate_subdiv(sqrt_remain_tolerance);
		sum += subdivision.val;
		quads.push((q, subdivision));
	}
	// Iterate through the quadratics, outputting the points of
	// subdivisions that fall within that quadratic.
	let n = ((1./2. * sum / sqrt_remain_tolerance).ceil() as usize).max(1);
	let step = sum / (n as f32);
	let mut i = 1;
	let mut val_sum = 0.;
	for (q, subdivision) in &quads {
		let mut target = (i as f32) * step;
		let recip_val = subdivision.val.recip();
		while target < val_sum + subdivision.val {
			let u = (target - val_sum) * recip_val;
			let t = subdivision.determine_subdiv_t(u);
			let p = q.eval(t);
			line_to(p);
			i += 1;
			if i == n + 1 {
				break;
			}
			target = (i as f32) * step;
		}
		val_sum += subdivision.val;
	}
	line_to(p3);
}

}}
