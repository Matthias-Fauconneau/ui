use vector::vec2;

pub fn quad(p0: vec2, p1: vec2, p2: vec2, mut line_to: impl FnMut(vec2)) {
	use kurbo::PathEl::*;
	fn point(vec2{x,y}: vec2) -> kurbo::Point { kurbo::Point{x: x as f64, y: y as f64} }
	let (p0,p1,p2) = (point(p0),point(p1),point(p2));
	kurbo::flatten([MoveTo(p0),QuadTo(p1,p2)], 1./4., |e| if let LineTo(kurbo::Point{x,y}) = e { line_to(vec2{x: x as f32, y: y as f32}); } /*Ignore first MoveTo*/)
}