use {num::{abs, fract}, vector::{xy,uint2,size,vec2}, image::{Image, bgrf}};
pub fn generate_line(size: size, p: [vec2; 2]) -> impl Iterator<Item=(uint2, uint2, f32, f32)> {
	let d = p[1] - p[0];
	let (transpose, p0, p1, d) = if abs(d.x) < abs(d.y) { (true, p[0].yx(), p[1].yx(), d.yx()) } else { (false, p[0], p[1], d) };
	assert!(d.x != 0.); // p0==p1
	let (p0, p1) = if p0.x > p1.x { (p1, p0) } else { (p0, p1) };
	let gradient = d.y / d.x;
	let f = move |x: u32, y: u32, cx: f32, cy: f32| if transpose { (xy{x: y, y: x}, xy{x: y+1, y: x}, cx, cy) } else { (xy{x,y},xy{x,y:y+1}, cx, cy) };
	std::iter::from_generator(move || {
		let (i0, intery) = {
			let xend = f32::round(p0.x);
			let yend = p0.y + gradient * (xend - p0.x);
			let xgap = 1. - (p0.x + 1./2. - xend);
			let fract_yend = yend - f32::floor(yend);
			yield f(xend as u32, yend as u32, xgap, fract_yend);
			(xend as i32, yend + gradient)
		};
		let xend = f32::round(p1.x);
		let i1 = xend as u32;
		{
			let x = i0+1;
			let (mut intery, mut x) = if x < 0 { (intery+(0-x as i32) as f32 * gradient, 0) } else { (intery, x as u32) };
			while x < i1.min(if transpose { size.y } else { size.x }) {
				yield f(x, intery as u32, 1., fract(intery));
				intery += gradient;
				x += 1;
			}
		}
		let yend = p1.y + gradient * (xend - p1.x);
		let xgap = p1.x + 1./2. - xend;
		let fract_yend = yend - f32::floor(yend);
		yield f(xend as u32, yend as u32, xgap, fract_yend);
	})
}
fn blend(eotf: &[f32; 256], oetf: &[u8; 0x1000], target: &mut Image<&mut[u32]>, color: bgrf, p: uint2, coverage: f32) { if p < target.size { target[p] = image::lerp/*PQ10(PQ10⁻¹)*/(eotf, oetf, coverage, target[p], color); } }
pub fn line(eotf: &[f32; 256], oetf: &[u8; 0x1000], target: &mut Image<&mut[u32]>, p0: vec2, p1: vec2, color: bgrf) {
	assert!(p0 != p1);
	let size = target.size;
	let mut f = |p,c| {println!("{p}"); blend(eotf, oetf, target, color, p, c)};
	for (p0, p1, cx, cy) in generate_line(size, [p0, p1]) { f(p0, cx*(1.-cy)); f(p1, cx*cy) }
}
pub fn parallelogram(target: &mut Image<&mut[f32]>, top_left: vec2, bottom_right: vec2, descending: bool, vertical_thickness: f32, opacity: f32) {
	assert!(top_left != bottom_right);
	let size = target.size;
	let mut f = |p,c| if p < target.size { target[p] += opacity*c; };
	let [top, bottom] = if descending  {[[top_left, bottom_right-xy{x: 0., y: vertical_thickness}], [top_left+xy{x: 0., y: vertical_thickness}, bottom_right]]} else
																//{[[xy{x: top_left.x, y: bottom_right.y}, xy{x: 0., y: vertical_thickness}, bottom_right], [top_left+xy{x: 0., y: vertical_thickness}, bottom_right]]};
																{[[xy{x: top_left.x, y: bottom_right.y-vertical_thickness}, xy{x: bottom_right.x, y: top_left.y}], [xy{x: top_left.x, y: bottom_right.y}, xy{x: bottom_right.x, y: top_left.y+vertical_thickness}]]};
	let d = top[1] - top[0] /*==d(bottom)*/; if abs(d.x) > abs(d.y) {
		for ((top0, top1, cx, top_cy), (bottom0, bottom1, bottom_cx, bottom_cy)) in generate_line(size, top).zip(generate_line(size, bottom)) {
			assert!(top0.x == top1.x && top1.x == bottom0.x && bottom0.x == bottom1.x && cx == bottom_cx, "{top0} {top1} {bottom0} {bottom1} {cx} {bottom_cx}"); // TODO: opti for cx==1 (i.e except ends)
			f(top0, cx*(1.-top_cy));
			let x = top0.x;
			for y in top1.y..=bottom0.y { f(xy{x,y}, 1.) }
			f(bottom1, cx*bottom_cy);
		}
	} else {
		//unimplemented!()
	}
}