pub fn list<T>(iter: impl std::iter::IntoIterator<Item=T>) -> Box<[T]> { iter.into_iter().collect() }
pub fn map<T,U>(iter: impl std::iter::IntoIterator<Item=T>, f: impl Fn(T)->U) -> Box<[U]> { list(iter.into_iter().map(f)) }

use {std::ops::Range, num::{zero, IsZero}, vector::Rect};
#[derive(Debug)] pub struct Plot {
	title: &'static str,
	axis_label: xy<&'static str>,
	keys: Box<[String]>,
	pub x_values: Vec<f64>,
	pub sets: Box<[Vec<f64>]>,
	range: xy<Range<f64>>,
	top: u32, bottom: u32, left: u32, right: u32,
	last: usize,
	key: Rect,
}

impl Plot {
	pub fn new(title: &'static str, axis_label: xy<&'static str>, keys: Box<[String]>) -> Self {
		let sets = map(&*keys, |_| Vec::new());
		Self{title, axis_label, keys, x_values: Vec::new(), sets, range: zero(), top: 0, bottom: 0, left: 0, right: 0, last: 0, key: zero()}
	}
	pub fn need_update(&mut self) { self.last = 0; }
}

use crate::*;
impl Widget for Plot {
#[throws] fn paint(&mut self, mut target: &mut Target, _: size, _: int2) {
	for set in &*self.sets { assert_eq!(self.x_values.len(), set.len(), "{:?}", (&self.x_values, &self.sets)); }

	let colors =
		if self.sets.len() == 1 { [foreground].into() }
		else { map(0..self.sets.len(), |i| bgrf::from(crate::color::LCh{L: if foreground>background { 100. } else { 66.6 }, C:179., h: 2.*std::f32::consts::PI*(i as f32)/(self.sets.len() as f32)})) };

	let ticks = |Range{end,..}| {
		if end == 0. { return (zero(), [(0.,"0".to_string())].into(), ""); }
		assert!(end > 0.);
		let end = f64::abs(end);
		let log10 = f64::log10(end);
		let floor10 = f64::floor(log10); // order of magnitude
		let part10 = num::exp10(log10 - floor10); // remaining magnitude part within the decade: x / 10^⌊log(x)⌋
		//assert!(part10 >= 1.. "{part10}");
		let (end, tick_count) = *[(1.,5),(1.2,6),(1.4,7),(2.,10),(2.5,5),(3.,3),(3.2,8),(4.,8),(5.,5),(6.,6),(8.,8),(10.,5)].iter().find(|(end,_)| part10-f64::exp2(-52.) <= *end).unwrap_or_else(||panic!("{end}"));
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
	let xy{x: Some(x), y: Some(y)} = xy{x: vector::minmax(self.x_values.iter().copied()).unwrap(), y: vector::minmax(self.sets.iter().flatten().copied()).unwrap()}.map(|minmax| {
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
	let range = axis.each_ref().map(|a| a.range.clone()).into();
	if range != self.range {
		(self.range, self.last) = (range, 0);
		let ref range = self.range;

		image::fill(&mut target, background.into());

		let ref bold = [crate::text::Style{color: foreground, style: crate::text::FontStyle::Bold}.into()];
		let text = |text, style| crate::text::with_color(foreground, text, style);

		let labels = xy{x: map(&*axis.x.labels, |(_,label)| label.as_ref()), y: map(&*axis.y.labels, |(_,label)| label.as_ref())};
		let mut ticks = labels.map(|labels| map(labels.iter(), |label| text(label, &[])));
		let label_size = ticks.map_mut(|ticks| vector::max(ticks.iter_mut().map(|tick| tick.size())).unwrap());

		let styles = map(&*colors, |&color| Box::from([color.into()]));
		let mut key_labels = map(self.keys.iter().zip(&*styles), |(key,style)| text(key, style));
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
		let mut key : Rect = zero();
		for (i, label) in key_labels.iter_mut().enumerate() {
			let y = (i as u32)*(y_label_scale*key_label_size.y);
			//let y = (i as u32)*(size.y-bottom-top)/(set_count as u32);
			let offset = xy{x: size.x-right-key_label_scale*key_label_size.x, y: top+y};
			label.paint(&mut target, size, key_label_scale, (offset/y_label_scale).signed());
			//xy{x: left+(i as u32)*(size.x-right-left)/(set_count as u32), y: 0}/y_label_scale
			let r = offset.signed()+Rect::from(y_label_scale*label.size());
			key = if key.is_zero() { r } else { key.minmax( r ) };
		}
		self.key = key;

		// Horizontal axis
		target.slice_mut(xy{x: left, y: size.y-bottom}, xy{x: size.x-right-left, y: 1}).set(|_| foreground.into());
		{
			let p = match axis_label_x_inline {
				true => xy{x: (size.x-right)/scale, y: (size.y-bottom)/scale-axis_label_x.size().y/2},
				false => xy{x: (size.x/scale-axis_label_x.size().x)/2, y: (size.y-bottom+x_label_scale*label_size.x.y)/scale}
			};
			axis_label_x.paint(&mut target, size, scale, p.signed());
		}

		// Vertical axis
		target.slice_mut(xy{x: left, y: top}, xy{x: 1, y: size.y.checked_sub(bottom+top).expect(&format!("{} {} {}", size.y, bottom, top))}).set(|_| foreground.into());
		{
			let p = xy{x: (left/scale).checked_sub(axis_label_y.size().x/2).unwrap_or(0), y: (top/scale).checked_sub(axis_label_y.size().y).unwrap_or(0)};
			axis_label_y.paint(&mut target, size, scale, p.signed());
		}

		let tick_length = 16;

		assert!(!range.x.is_empty());
		for (&(value,_), tick_label) in axis.x.labels.iter().zip(ticks.x.iter_mut()) {
			let p = xy{x: map_x(size, left, right, &range.x, value) as u32, y: size.y-bottom};
			target.slice_mut(p-xy{x:0, y:tick_length}, xy{x:1, y:tick_length}).set(|_| foreground.into());
			let p = p/x_label_scale - xy{x: tick_label.size().x/2, y: 0};
			tick_label.paint(&mut target, size, x_label_scale, p.signed());
		}

		assert!(!range.y.is_empty());
		for (&(value,_), tick_label) in axis.y.labels.iter().zip(ticks.y.iter_mut()) {
			let p = xy{x: [0, size.x-right][0], y: map_y(size, top, bottom, &range.y, value) as u32};
			target.slice_mut(p + xy{x: [left,0][0], y:0} - xy{x: [0,tick_length][0], y:0}, xy{x:tick_length, y:1}).set(|_| foreground.into());
			let sub = |a,b| (a as i32 - b as i32).max(0) as u32;
			let p = p/scale + xy{x: [sub(left/scale, tick_label.size().x), 0][0], y: 0} - xy{x: 0, y: tick_label.size().y/2};
			tick_label.paint(&mut target, size, scale, p.signed());
		}
	} else if self.last == 0 {
		let offset = xy{x: self.left+1, y: self.top}.signed();
		let key = {let mut key = self.key; key.translate(-offset); vector::MinMax{min: key.min.unsigned(), max: key.max.unsigned()}};
		let mut target = target.slice_mut(offset.unsigned(), xy{x: size.x-self.right, y: size.y-self.bottom}-offset.unsigned());
		let size = target.size;
		image::fill(&mut target.slice_mut(zero(), xy{x: std::cmp::min(size.x, key.min.x), y: size.y}), background.into());
	}

	let (left,right,top,bottom) = (self.left,self.right,self.top,self.bottom);
	for (values, &color) in self.sets.iter().zip(&*colors) {
		let points = map(self.last.max(1)-1..self.x_values.len(), |i| xy{
			x: map_x(size, left, right, &self.range.x, self.x_values[i]),
			y: map_y(size, top, bottom, &self.range.y, values[i])
		});
		let thickness = 1.; // FIXME: orthogonal to line not vertical
		let thick_line = points.iter().map(|p| p-xy{x: 0., y: thickness/2.}).chain(points.iter().rev().map(|p| p+xy{x: 0., y: thickness/2.}));
		let mut a = points[0]+xy{x: 0., y: 1.}; // Starts by closing the loop with left edge
		for b in thick_line {
			crate::line(&mut target, a, b, color);
			a = b;
		}
	}
	self.last = self.x_values.len();
}
#[throws] fn event(&mut self, _: size, _: &mut Option<EventContext>, event: &Event) -> bool { if let Event::Stale = event { self.range = zero(); true } else { false } }
}
