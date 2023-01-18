pub fn list<T>(iter: impl std::iter::IntoIterator<Item=T>) -> Box<[T]> { iter.into_iter().collect() }
fn map<T,U>(iter: impl std::iter::IntoIterator<Item=T>, f: impl Fn(T)->U) -> Box<[U]> { list(iter.into_iter().map(f)) }

use {::vector::{xy,vec2}, image::{Image, bgrf}};
fn line(target: &mut Image<&mut[u32]>, p0: vec2, p1: vec2, color: bgrf) {
	use num::{abs, fract};
	let d = p1 - p0;
	let (transpose, p0, p1, d) = if abs(d.x) < abs(d.y) { (true, p0.yx(), p1.yx(), d.yx()) } else { (false, p0, p1, d) };
	if d.x == 0. { return; } // p0==p1
	let (p0, p1) = if p0.x > p1.x { (p1, p0) } else { (p0, p1) };
	let gradient = d.y / d.x;
	fn blend(target: &mut Image<&mut[u32]>, x: u32, y: u32, color: bgrf, coverage: f32, transpose: bool) {
		let xy{x,y} = if transpose { xy{x: y, y: x} } else { xy{x,y} };
		if x < target.size.x && y < target.size.y { target[xy{x,y}]/*.saturating_add_assign(*/= (coverage*color).into(); }
	}
	let (i0, intery) = {
		let xend = f32::round(p0.x);
		let yend = p0.y + gradient * (xend - p0.x);
		let xgap = 1. - (p0.x + 1./2. - xend);
		let fract_yend = yend - f32::floor(yend);
		blend(target, xend as u32, yend as u32, color, (1.-fract_yend) * xgap, transpose);
		blend(target, xend as u32, yend as u32+1, color, fract_yend * xgap, transpose);
		(xend as i32, yend + gradient)
	};
	let i1 = {
		let xend = f32::round(p1.x);
		let yend = p1.y + gradient * (xend - p1.x);
		let xgap = p1.x + 1./2. - xend;
		let fract_yend = yend - f32::floor(yend);
		blend(target, xend as u32, yend as u32, color, (1.-fract_yend) * xgap, transpose);
		blend(target, xend as u32, yend as u32+1, color, fract_yend * xgap, transpose);
		xend as u32
	};
	let x = i0+1;
	let (mut intery, mut x) = if x < 0 { (intery+(0-x as i32) as f32 * gradient, 0) } else { (intery, x as u32) };
	while x < i1.min(if transpose { target.size.y } else { target.size.x }) {
		blend(target, x, intery as u32, color, 1.-fract(intery), transpose);
		blend(target, x, intery as u32+1, color, fract(intery), transpose);
		intery += gradient;
		x += 1;
	}
}

use {std::ops::Range, num::zero};
pub struct Plot<'t> {
	title: &'t str,
	axis_label: xy<&'t str>,
	keys: &'t [&'t str],
	pub x_values: &'t [f64],
	pub sets: &'t [&'t [f64]],
	//pub values: &'t [Frame],
	range: xy<Range<f64>>,
	top: u32, bottom: u32, left: u32, right: u32,
	last: usize,
}

impl<'t> Plot<'t> {
	pub fn new(title: &'t str, axis_label: xy<&'t str>, keys: &'t [&'t str], x_values: &'t [f64], sets: &'t [&'t [f64]]) -> Self {
		for set in sets { assert_eq!(x_values.len(), set.len(), "{x_values:?} {set:?}"); }
		Self{title, axis_label, keys, x_values, sets, range: zero(), top: 0, bottom: 0, left: 0, right: 0, last: 0}
	}
}

impl crate::Widget for Plot<'_> {
#[fehler::throws(crate::Error)] fn paint(&mut self, mut target: &mut crate::Target, _: crate::size, _: crate::int2) {
	let [black, white] : [bgrf; 2]  = [0., 1.].map(Into::into);
	#[allow(non_upper_case_globals)] const dark : bool = false;
	let [bg, fg] = if dark { [black, white] } else { [white, black] };

	/*let (keys, values) = {
		let filter = self.values.iter()
			.map(|(_,sets)| map(&**sets, |set| map(&**set, |&_y| /*_y>1e-6*/true)))
			.reduce(|a,b| map(a.iter().zip(b.iter()), |(a,b)| map(a.iter().zip(b.iter()), |(a,b)| a|b))).unwrap();
		let keys = map(self.keys.iter().zip(filter.iter()), |(sets, filter)| map(sets.iter().zip(filter.iter()).filter(|(_,&filter)| filter), |(set,_)| set));
		let values = map(self.values, |(x,sets)| (x, map(sets.iter().zip(filter.iter()), |(set,filter)| map(set.iter().zip(filter.iter()).filter(|(_,&filter)| filter), |(&set,_)| set))));
		(keys, values)
	};*/
	let (keys, sets) = (self.keys, self.sets);

	let colors =
		if sets.len() == 1 { [fg].into() }
		else { map(0..sets.len(), |i| bgrf::from(crate::color::LCh{L: if fg>bg { 100. } else { 100. /*53.*/ }, C:179., h: 2.*std::f32::consts::PI*(i as f32)/(sets.len() as f32)})) };

	let ticks = |Range{end,..}| {
		if end == 0. { return (zero(), [(0.,"0".to_string())].into(), ""); }
		assert!(end > 0.);
		let end = f64::abs(end);
		let log10 = f64::log10(end);
		let floor10 = f64::floor(log10); // order of magnitude
		let part10 = num::exp10(log10 - floor10); // remaining magnitude part within the decade: x / 10^⌊log(x)⌋
		//assert!(part10 >= 1.. "{part10}");
		let (end, tick_count) = *[(1.,5),(1.2,6),(1.4,7),(2.,10),(2.5,5),(3.,3),(3.2,8),(4.,8),(5.,5),(6.,6),(8.,8),(10.,5)].iter().find(|(end,_)| part10-f64::exp2(-52.) <= *end).unwrap();
		assert!(end <= 10.);
		let end = end*num::exp10(floor10); // Scale back with order of magnitude
		let log10 = f64::log10(end);
		let floor1000 = f64::floor(log10/3.); // submultiple
		let part1000 = num::exp10(log10 - floor1000*3.); // remaining magnitude part within the submultiple: x / 1000^⌊log1000(x)⌋
		let labels = map(0..=tick_count, |i| {
			let fract = (i as f64)/(tick_count as f64);
			let label = fract*part1000;
			let label = if part1000/(tick_count as f64) < 1. { format!("{:.1}",label) } else { (f64::round(label) as u32).to_string() };
			(fract*end, label)
		});
		for [a,b] in labels.array_windows() { assert!(a.1 != b.1, "{a:?} {b:?} {:?}", (end, part1000/(tick_count as f64))); }
		let submultiple = ["n","µ","m","","k","M","G"][(3+(floor1000 as i8)) as usize];
		for (_, label) in &*labels { assert!(label.len() <= 4, "{label}"); }
		((0.)..end, labels, submultiple)
	};

	struct Axis { range: Range<f64>, labels: Box<[(f64, String)]>, submultiple: &'static str }
	let xy{x: Some(x), y: Some(y)} = xy{x: vector::minmax(self.x_values.iter().copied()).unwrap(), y: vector::minmax(sets.iter().map(|&set| set.iter()).flatten().copied()).unwrap()}.map(|&minmax| {
		let range = Range::from(minmax);
		if range.is_empty() { return None; }
		let (range, labels, submultiple) = ticks(range);
		assert!(!range.is_empty());
		Some(Axis{range, labels, submultiple})
	}) else { return Ok(()); };

	let size = target.size;
	#[track_caller] fn linear_step(Range{start,end} : &Range<f64>, v: f64) -> f32 { assert!(start < end); ((v-start) / (end-start)) as f32 }
	use crate::size;
	#[track_caller] fn map_x(size: size, left: u32, right: u32, range: &Range<f64>, v: f64) -> f32 { left as f32+linear_step(range, v)*(size.x-left-right-1) as f32 }
	#[track_caller] fn map_y(size: size, top: u32, bottom: u32, range: &Range<f64>, v: f64) -> f32 { (size.y-bottom-1) as f32-linear_step(range, v)*(size.y-top-bottom-1) as f32 }

	let axis = xy{x,y};
	let range = axis.map(|a| a.range.clone());
	if range != self.range {
		(self.range, self.last) = (range, 0);
		let ref range = self.range;

		image::fill(&mut target, bg.into());

		let ref bold = [crate::text::Style{color: fg, style: crate::text::FontStyle::Bold}.into()];
		let text = |text, style| crate::text::with_color(fg, text, style);

		let labels = xy{x: map(&*axis.x.labels, |(_,label)| label.as_ref()), y: map(&*axis.y.labels, |(_,label)| label.as_ref())};
		let mut ticks = labels.map(|labels| map(&**labels, |label| text(label, &[])));
		let label_size = ticks.map_mut(|ticks| vector::max(ticks.iter_mut().map(|tick| tick.size())).unwrap());

		let styles = map(&*colors, |&color| Box::from([color.into()]));
		let mut key_labels = map(keys.iter().zip(&*styles), |(&key,style)| text(key, style));
		let key_label_size = vector::max(key_labels.iter_mut().map(|label| label.size())).unwrap();

		let x_label_scale = num::Ratio{num: size.x/(ticks.x.len() as u32*2).max(5)-1, div: label_size.x.x-1};
		let y_label_scale = num::Ratio{num: size.y/4/(ticks.y.len() as u32)-1, div: label_size.y.y-1};
		let scale = std::cmp::min(x_label_scale, y_label_scale);
		let [x_label_scale, y_label_scale, key_label_scale] = [scale; 3];

		let axis_label_y = self.axis_label.y.replace("$", axis.y.submultiple);
		let mut axis_label_y = text(&axis_label_y, bold);
		let top = (scale*axis_label_y.size().y).max((y_label_scale.ceil(label_size.y.y) + 1) / 2).min(size.y/4);
		let [left, right] = [0,1].map(|_i|
			(if let Some(&label_size) = Some(&label_size.y)/*.get(i)*/ { y_label_scale.ceil(label_size.x) } else { 0 })
				.max((x_label_scale.ceil(label_size.x.x)+1)/2).min(size.x/4)
		);
		let axis_label_x = self.axis_label.x.replace("$", axis.x.submultiple);
		let mut axis_label_x = text(&axis_label_x, bold);
		let axis_label_x_inline = scale*axis_label_x.size().x < right;
		let bottom = (x_label_scale * label_size.x.y + if axis_label_x_inline { 0 } else { scale * axis_label_x.size().y }).min(size.y/4);

		(self.left, self.right, self.top, self.bottom) = (left,right,top,bottom);

		// Title
		let mut title = text(&self.title, bold);
		let p = xy{x: (size.x/scale-title.size().x)/2, y: 0};
		title.paint(&mut target, size, scale, p.signed());

		// Key
		for (i, label) in key_labels.iter_mut().enumerate() {
			let y = (i as u32)*(y_label_scale*key_label_size.y);
			//let y = (i as u32)*(size.y-bottom-top)/(set_count as u32);
			label.paint(&mut target, size, y_label_scale, (xy{x: size.x-right-key_label_scale*key_label_size.x, y: top+y}/y_label_scale).signed());
			//xy{x: left+(i as u32)*(size.x-right-left)/(set_count as u32), y: 0}/y_label_scale
		}

		// Horizontal axis
		target.slice_mut(xy{x: left, y: size.y-bottom}, xy{x: size.x-right-left, y: 1}).set(|_| fg.into());
		{
			let p = match axis_label_x_inline {
				true => xy{x: (size.x-right)/scale, y: (size.y-bottom)/scale-axis_label_x.size().y/2},
				false => xy{x: (size.x/scale-axis_label_x.size().x)/2, y: (size.y-bottom+x_label_scale*label_size.x.y)/scale}
			};
			axis_label_x.paint(&mut target, size, scale, p.signed());
		}

		// Vertical axis
		target.slice_mut(xy{x: left, y: top}, xy{x: 1, y: size.y.checked_sub(bottom+top).expect(&format!("{} {} {}", size.y, bottom, top))}).set(|_| fg.into());
		{
			let p = xy{x: (left/scale).checked_sub(axis_label_y.size().x/2).unwrap_or(0), y: top/scale-axis_label_y.size().y};
			axis_label_y.paint(&mut target, size, scale, p.signed());
		}

		//target.slice_mut(xy{x: size.x-right, y: top}, xy{x: 1, y: size.y.checked_sub(bottom+top).unwrap()}).set(|_| fg); // right vertical axis

		let tick_length = 16;

		assert!(!range.x.is_empty());
		for (&(value,_), tick_label) in axis.x.labels.iter().zip(ticks.x.iter_mut()) {
			let p = xy{x: map_x(size, left, right, &range.x, value) as u32, y: size.y-bottom};
			target.slice_mut(p-xy{x:0, y:tick_length}, xy{x:1, y:tick_length}).set(|_| fg.into());
			let p = p/x_label_scale - xy{x: tick_label.size().x/2, y: 0};
			tick_label.paint(&mut target, size, x_label_scale, p.signed());
		}

		assert!(!range.y.is_empty());
		for (&(value,_), tick_label) in axis.y.labels.iter().zip(ticks.y.iter_mut()) {
			let p = xy{x: [0, size.x-right][0], y: map_y(size, top, bottom, &range.y, value) as u32};
			target.slice_mut(p + xy{x: [left,0][0], y:0} - xy{x: [0,tick_length][0], y:0}, xy{x:tick_length, y:1}).set(|_| fg.into());
			let sub = |a,b| (a as i32 - b as i32).max(0) as u32;
			let p = p/scale + xy{x: [sub(left/scale, tick_label.size().x), 0][0], y: 0} - xy{x: 0, y: tick_label.size().y/2};
			tick_label.paint(&mut target, size, scale, p.signed());
		}
	}

	let (left,right,top,bottom) = (self.left,self.right,self.top,self.bottom);
	let mut frames = (self.last.max(1)-1..self.x_values.len()).map(|i| (
		map_x(size, left, right, &self.range.x, self.x_values[i]),
		{let range = self.range.y.clone(); sets.iter().map(move |values| map_y(size, top, bottom, &range, values[i]))}
	));
	let mut last = {let (x, y) = frames.next().unwrap(); (x, list(y))};
	let mut next = (0., map(sets, |_| 0.));
	fn collect<T>(target: &mut [T], iter: impl IntoIterator<Item=T>) -> &[T] { for (slot, item) in target.iter_mut().zip(iter) { *slot = item; } target }
	for (next_x, next_y) in frames {
		let next_y = collect(&mut next.1, next_y);
		for (i, (&last_y, &next_y)) in last.1.iter().zip(&*next_y).enumerate() { self::line(&mut target, xy{x: last.0, y: last_y}, xy{x: next_x, y: next_y}, colors[i]) }
		last.0 = next_x; std::mem::swap(&mut last.1, &mut next.1);
	}
	self.last = self.x_values.len();
}
}
