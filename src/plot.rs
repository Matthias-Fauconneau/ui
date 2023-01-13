use {::vector::{xy,vec2}, image::{Image, bgrf}};
pub fn list<T>(iter: impl std::iter::IntoIterator<Item=T>) -> Box<[T]> { iter.into_iter().collect() }
fn map<T,U>(iter: impl std::iter::IntoIterator<Item=T>, f: impl Fn(T)->U) -> Box<[U]> { list(iter.into_iter().map(f)) }

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

use vector::MinMax;
type Frame = (f64, Box<[Box<[f64]>]>);
pub struct Plot<'t> {
	keys: &'t [&'t [&'t str]],
	pub values: &'t [Frame],
	x_minmax: MinMax<f64>,
	sets_minmax: Box<[MinMax<f64>]>,
	top: u32, bottom: u32, left: u32, right: u32,
	last: usize,
}

impl<'t> Plot<'t> {
	pub fn new(keys: &'t [&'t [&'t str]], values: &'t [Frame]) -> Self {
		for (_, value) in values { for (keys, set) in keys.iter().zip(value.iter()) { assert_eq!(keys.len(), set.len(), "{keys:?} {set:?}"); } }
		use num::zero; Self{keys, values, x_minmax: MinMax{min: zero(), max: zero()}, sets_minmax: [].into(), top: 0, bottom: 0, left: 0, right: 0, last: 0}
	}
}

impl crate::Widget for Plot<'_> {
#[fehler::throws(crate::Error)] fn paint(&mut self, mut target: &mut crate::Target, _: crate::size, _: crate::int2) {
	let (keys, values) = {
		let filter = self.values.iter()
			.map(|(_,sets)| map(&**sets, |set| map(&**set, |&_y| /*_y>1e-6*/true)))
			.reduce(|a,b| map(a.iter().zip(b.iter()), |(a,b)| map(a.iter().zip(b.iter()), |(a,b)| a|b))).unwrap();
		let keys = map(self.keys.iter().zip(filter.iter()), |(sets, filter)| map(sets.iter().zip(filter.iter()).filter(|(_,&filter)| filter), |(set,_)| set));
		let values = map(self.values, |(x,sets)| (x, map(sets.iter().zip(filter.iter()), |(set,filter)| map(set.iter().zip(filter.iter()).filter(|(_,&filter)| filter), |(&set,_)| set))));
		(keys, values)
	};
	let set_count = keys.iter().map(|set| set.len()).sum::<usize>();

	let sets_colors = map(&*keys, |set|
		if set.len() == 1 { [bgrf{b:0., g:0., r:0.}].into() }
		//if set.len() == 1 { [bgrf{b:1., g:1., r:1.}].into() }
		else { map(0..set.len(), |i| bgrf::from(crate::color::LCh{L:53., C:179., h: 2.*std::f32::consts::PI*(i as f32)/(set.len() as f32)})) }
	);

	let ticks = |MinMax{max,..}| {
		if max == 0. { return (vector::MinMax{min: 0., max: 0.}, [(0.,"0".to_string())].into()); }
		let log10 = f64::log10(f64::abs(max));
		let exp_fract_log10 = num::exp10(log10 - f64::floor(log10));
		let (max, tick_count) = *[(1.,5),(1.2,6),(1.4,7),(2.,10),(2.5,5),(3.,3),(3.2,8),(4.,8),(5.,5),(6.,6),(8.,8),(10.,5)].iter().find(|(max,_)| exp_fract_log10-f64::exp2(-52.) <= *max).unwrap();
		let max = max*num::exp10(f64::floor(log10));
		let precision = if max/(tick_count as f64) < 1. { 1 } else { 0 };
		(vector::MinMax{min: 0., max}, map((0..=tick_count).map(|i| max*(i as f64)/(tick_count as f64)), |value| (value, format!("{:.1$}", value, precision))))
	};

	let x_minmax = vector::minmax(values.iter().map(|&(x,_)| *x)).unwrap();
	let (x_minmax, x_labels) = ticks(x_minmax);

	let mut serie_of_sets = values.iter().map(|(_,sets)| sets);
	let mut sets_data_minmax = map(&**serie_of_sets.next().unwrap(), |set| vector::minmax(set.iter().copied()).unwrap());
	for sets in serie_of_sets { for (minmax, set) in sets_data_minmax.iter_mut().zip(sets.iter()) { *minmax = minmax.minmax(vector::minmax(set.iter().copied()).unwrap()) } }
	let sets_minmax = map(&*sets_data_minmax, |&minmax| ticks(minmax).0); // fixme

	let size = target.size;
	fn linear_step(MinMax{min,max} : MinMax<f64>, v: f64) -> Option<f32> { if min < max { Some(((v-min) / (max-min)) as f32) } else { None } }
	use crate::size;
	fn map_x(size: size, left: u32, right: u32, minmax: MinMax<f64>, v: f64) -> Option<f32> { Some(left as f32+linear_step(minmax, v)?*(size.x-left-right-1) as f32) }
	fn map_y(size: size, top: u32, bottom: u32, minmax: MinMax<f64>, v: f64) -> Option<f32> { Some((size.y-bottom-1) as f32-linear_step(minmax, v)?*(size.y-top-bottom-1) as f32) }

	if (x_minmax, &sets_minmax) != (self.x_minmax, &self.sets_minmax) {
		image::fill(&mut target, (0x3FF << 20) | (0x3FF << 10) |0x3FF);

		let mut x_ticks = map(&*x_labels, |(_,label)| crate::text::View::new(crate::text::Borrowed{text: label,style:&[]}));
		let x_label_size = vector::max(x_ticks.iter_mut().map(|tick| tick.size())).unwrap();

		let sets_tick_labels = map(&*sets_data_minmax, |&minmax| ticks(minmax).1);
		let mut sets_ticks = map(&*sets_tick_labels, |set| map(&**set, |(_,label)| crate::text::View::new(crate::text::Borrowed{text: label,style:&[]})));
		let sets_tick_label_size = map(&mut *sets_ticks, |set_ticks| { vector::max(set_ticks.iter_mut().map(|tick| tick.size())).unwrap() });

		let sets_styles = map(&*sets_colors, |set| map(&**set, |&color| (Box::from([color.into()]))));
		let mut sets_labels = map(keys.iter().zip(sets_styles.iter()),
			|(keys, styles)| map(keys.iter().zip(styles.iter()), |(key,style)| crate::text::View::new(crate::text::Borrowed{text: key, style}))
		);
		let sets_label_size = vector::max( sets_labels.iter_mut().map(|set| { vector::max(set.iter_mut().map(|label| label.size())).unwrap() }) ).unwrap();
		//assert!(sets_label_size.y < 1145, "{} {}", format!("{:?}", sets_labels), sets_labels.iter_mut().map(|set| { vector::max(set.iter_mut().map(|label| label.size())).unwrap() }).format(" "));

		let x_label_scale = num::Ratio{num: size.x/(x_labels.len() as u32*2).max(5)-1, div: x_label_size.x-1};
		let y_label_scale = sets_tick_labels.iter().zip(sets_tick_label_size.iter()).map(|(labels, label_size)| num::Ratio{num: size.y/4/(labels.len() as u32)-1, div: label_size.y-1}).min().unwrap();
		let scale = std::cmp::min(x_label_scale, y_label_scale);
		let [x_label_scale, y_label_scale, sets_label_scale] = [scale; 3]; //(set_count>0).then(|| num::Ratio{num: size.x/(set_count as u32*2).max(5)-1, div: sets_label_size.x-1});

		self.top = (y_label_scale*sets_label_size.y).max(((sets_tick_label_size.iter().map(|&label_size| y_label_scale.ceil(label_size.y)).max().unwrap() + 1) / 2).min(size.y/4));
		//assert!(self.top < 1829, "{:?}", (self.top, y_label_scale, sets_label_size.y, size.y/4));
		self.bottom = (x_label_scale * x_label_size.y).min(size.y/4);
		let [left, right] = [0,1].map(|i|
			(if let Some(&label_size) = sets_tick_label_size.get(i) { y_label_scale.ceil(label_size.x) } else { 0 })
				.max((x_label_scale.ceil(x_label_size.x)+1)/2).min(size.x/4)
		);
		self.left = left;
		self.right = right;

		for (i, label) in sets_labels.iter_mut().map(|set| set.iter_mut()).flatten().enumerate() {
			label.paint(&mut target, size, sets_label_scale, (xy{x: left+(i as u32)*(size.x-right-left)/(set_count as u32), y: 0}/sets_label_scale).signed());
		}

		let fg = 0;//(0x3FF << 20) | (0x3FF << 10) | 0x3FF;
		target.slice_mut(xy{x: left, y: size.y-self.bottom}, xy{x: size.x-right-left, y: 1}).set(|_| fg); // horizontal axis
		target.slice_mut(xy{x: left, y: self.top}, xy{x: 1, y: size.y.checked_sub(self.bottom+self.top).expect(&format!("{} {} {}", size.y, self.bottom, self.top))}).set(|_| fg); // vertical axis
		//target.slice_mut(xy{x: size.x-right, y: self.top}, xy{x: 1, y: size.y.checked_sub(self.bottom+self.top).unwrap()}).set(|_| fg); // right vertical axis

		let tick_length = 16;
		if x_minmax.min < x_minmax.max {
			for (&(value,_), tick_label) in x_labels.iter().zip(x_ticks.iter_mut()) {
				let p = xy{x: map_x(size, left, right, x_minmax, value).unwrap() as u32, y: size.y-self.bottom};
				target.slice_mut(p-xy{x:0, y:tick_length}, xy{x:1, y:tick_length}).set(|_| fg);
				let p = p/x_label_scale - xy{x: tick_label.size().x/2, y: 0};
				tick_label.paint(&mut target, size, x_label_scale, p.signed());
			}
		}

		for i in 0..keys.len() {
			if let (Some(&minmax), Some(labels), Some(ticks)) = (sets_minmax.get(i), sets_tick_labels.get(i), sets_ticks.get_mut(i)) {
				if minmax.min < minmax.max {
					for (&(value,_), tick_label) in labels.iter().zip(ticks.iter_mut()) {
						let p = xy{x: [0, size.x-right][i], y: map_y(size, self.top, self.bottom, minmax, value).unwrap() as u32};
						target.slice_mut(p+xy{x:[left,0][i],y:0}-xy{x:[0,tick_length][i],y:0}, xy{x:tick_length, y:1}).set(|_| fg);
						let sub = |a,b| (a as i32 - b as i32).max(0) as u32;
						let p = p/scale + xy{x: [sub(left/scale, tick_label.size().x),0][i], y: 0} - xy{x: 0, y: tick_label.size().y/2};
						tick_label.paint(&mut target, size, scale, p.signed());
					}
				}
			}
		}

		self.x_minmax = x_minmax;
		self.sets_minmax = sets_minmax;
		self.last = 0;
	}

	use itertools::Itertools;
	let (left,right,top,bottom) = (self.left,self.right,self.top,self.bottom);
	values[self.last.max(1)-1..].iter().map(|(&x, sets)| sets.iter().zip(self.sets_minmax.iter()).zip(sets_colors.iter()).map(
		move |((set, &minmax), colors)| set.iter().zip(colors.iter()).map(move |(&y, color)| Some((xy{x: map_x(size, left, right, x_minmax, x)?, y: map_y(size, top, bottom, minmax, y)?}, color)))
	))
	.tuple_windows().for_each(|(sets0, sets1)| sets0.zip(sets1).for_each(|(s0,s1)| s0.zip(s1).for_each(
		|line| if let (Some((p0, &color)), Some((p1, _))) = line { self::line(&mut target, p0, p1, color) }
	)));
	self.last = values.len();
}
}
