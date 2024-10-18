#![allow(non_snake_case)]
use {std::{cmp::{min, max}, ops::Range}, crate::{throws, Error}, vector::{num::{zero, Ratio, unit}, xy, uint2, int2, size, Rect}};
use {image::bgrf as Color, crate::{foreground, font::{self, Face, GlyphId}}};
pub mod unicode_segmentation;
use self::unicode_segmentation::UnicodeSegmentation;
type TextIndex = usize;//GraphemeIndex
#[derive(derive_more::Deref)] pub(crate) struct LineRange<'t> { #[deref] line: &'t str, pub(crate) range: Range<TextIndex> }

pub(crate) fn line_ranges<'t>(text: &'t str) -> impl Iterator<Item=LineRange<'t>> {
	let mut iter = text.grapheme_indices(true).map(|(_,c)| c)/*bytes()*/.enumerate().map(|(i,c)|(i,(i,c))).peekable();
	std::iter::from_fn(move || {
		let &(start, (byte_start,_)) = iter.peek()?;
		let (end, byte_end) = iter.find(|&(_,(_,c))| c=="\n"/*'\n' as u8*/).map_or((text.len(),text.len()/*fixme*/), |(end,(byte_end,_))| (end, byte_end));
		Some(LineRange{line: &text[byte_start..byte_end], range: start..end})
	})
}

pub type Font<'t> = [&'t Face<'t>; 2];
/*#[derive(Clone,Copy)] pub struct Glyph<'t> {byte_index: usize, pub x: u32, face: &'t Face<'t>, pub id: GlyphId }
pub fn layout<'t>(font: &'t Font<'t>, str: &'t str) -> impl 't+IntoIterator<Item=Glyph<'t>> {
	let mut buffer = rustybuzz::UnicodeBuffer::new();
	buffer.set_cluster_level(rustybuzz::BufferClusterLevel::Characters);
	buffer.set_direction(rustybuzz::Direction::LeftToRight);
	buffer.push_str(str);
	let buffer = rustybuzz::shape(font[0], &[], buffer);
	let mut clusters = buffer.glyph_infos().into_iter().zip(buffer.glyph_positions()).scan(0,
			|x, (&rustybuzz::GlyphInfo{glyph_id,cluster:byte_index,..},&rustybuzz::GlyphPosition{x_offset,x_advance,..})| {
		let (face, id, x_offset, x_advance) = if glyph_id>0 { (&font[0], GlyphId(glyph_id as u16), x_offset, x_advance) } else {
			let c = str[byte_index as usize..].chars().next().unwrap();
			let (face, id) = font.iter().find_map(|face| face.glyph_index(c/*if c == '\t' { ' ' } else{ c }*/).map(|id| (face, id))).unwrap_or_else(||panic!("Missing glyph for '{c}' {:x?}", c as u32));
			(face, id, face.glyph_hor_side_bearing(id).unwrap() as i32, face.glyph_hor_advance(id)? as i32)
		};
		let next = Glyph{byte_index: byte_index as usize, x: (*x+x_offset) as u32, face, id};
		*x += x_advance;
		Some((next, x_advance))
	}).peekable();
	let layout = str.graphemes(true)./*bytes().*/enumerate().scan((Glyph{byte_index:0, x:0, id:GlyphId(0), face:font[0]},0), move |(cluster,advance), (byte_index,_)| {
		let (next_cluster, next_advance) = clusters.next()?;
		Some(if next_cluster.byte_index == byte_index {
			(*cluster, *advance) = (next_cluster, next_advance);
			*cluster
		} else { // Divide cluster horizontally
			let next_index = clusters.peek().map_or(str.graphemes(true).count()/*len()*/, |(cluster,_)| cluster.byte_index);
			Glyph{byte_index, x: cluster.x+(byte_index-cluster.byte_index)as u32* (*advance as u32)/(next_index-cluster.byte_index) as u32, id: font[0].glyph_index(' ').unwrap(), face: font[0]}
		})
	}).collect::<Vec<_>>();
	//assert!(!layout.is_empty());
	layout
}*/
use self::unicode_segmentation::GraphemeIndex;
#[derive(Clone,Copy)] pub struct Glyph<'t> {_index: GraphemeIndex, pub x: u32, face: &'t Face<'t>, pub id: GlyphId }
pub fn layout<'t>(font: &'t Font<'t>, str: &'t str) -> impl 't+IntoIterator<Item=Glyph<'t>> {
	pub fn layout<'t>(font: &'t Font<'t>, iter: impl Iterator<Item=(GraphemeIndex, &'t str)>+'t) -> impl 't+Iterator<Item=Glyph<'t>> {
		iter.scan((None, 0), move |(last_id, x), (index, g)| {
			let c = iter::Single::single(g.chars()).unwrap();
			//let [c] = arrayvec::ArrayVec::from_iter(g.chars()).into_inner().unwrap();
			let (face, id) = font.iter().find_map(|face| face.glyph_index(if c == '\t' { ' ' } else { c }).map(|id| (face, id))).unwrap_or_else(||panic!("Missing glyph for '{c}' {:x?}", c as u32));
			//if let Some(last_id) = *last_id { *x += face.tables().kern.unwrap().subtables.into_iter().next().map_or(0, |x| x.glyphs_kerning(last_id, id).unwrap_or(0) as i32); }
			*last_id = Some(id);
			let next = Glyph{_index: index, x: *x, id, face};
			*x += face.glyph_hor_advance(id)? as u32;
			Some(next)
		})
	}
	layout(font, str.graphemes(true).enumerate())
}

pub(crate) fn bbox<'t>(iter: impl Iterator<Item=Glyph<'t>>) -> impl Iterator<Item=(Rect, Glyph<'t>)> { iter.filter_map(move |g| Some((g.face.bbox(g.id)?, g))) }

struct LineMetrics {pub width: u32, pub ascent: i16, pub descent: i16}
fn metrics<'t>(iter: impl Iterator<Item=Glyph<'t>>) -> LineMetrics {
	bbox(iter).fold(LineMetrics{width: 0, ascent: 0, descent: 0}, |metrics: LineMetrics, (bbox, Glyph{x, id, face, ..})| LineMetrics{
		width: (x as i32 + face.glyph_hor_side_bearing(id).unwrap() as i32 + bbox.max.x) as u32,
		ascent: max(metrics.ascent, bbox.max.y as i16),
		descent: min(metrics.descent, bbox.min.y as i16)
	})
}

#[derive(Clone,Copy,Default,Debug)] pub enum FontStyle { #[default] Normal, Bold, /*Italic, BoldItalic*/ }
#[derive(Clone,Copy,Default,Debug)] pub struct Style { pub color: Color, pub style: FontStyle }
pub type TextRange = std::ops::Range<usize>;
#[derive(Clone,derive_more::Deref,Debug)] pub struct Attribute<T> { #[deref] pub range: TextRange, pub attribute: T }
impl /*const*/ From<Style> for Attribute<Style> { fn from(attribute: Style) -> Self { Attribute{range: 0../*GraphemeIndex*/usize::MAX, attribute} } }
impl From<Color> for Attribute<Style> { fn from(color: Color) -> Self { Style{color, style: FontStyle::Normal}.into() } }

#[allow(non_upper_case_globals)] pub static default_font_files : std::sync::LazyLock<[font::File<'static>; 2]> = std::sync::LazyLock::new(|| ["-Regular","Symbols-Regular"].map(|_v| {
	#[cfg(not(target_os="linux"))] return font::open(font_kit::source::SystemSource::new().select_best_match(&[font_kit::family_name::FamilyName::SansSerif], &Default::default()).unwrap().load().unwrap().copy_font_data().unwrap()).unwrap();
	#[cfg(target_os="linux")] font::open(&glob::glob(&format!("/usr/share/fonts/**/noto/NotoSans{_v}.ttf")).unwrap().next().unwrap().unwrap()).unwrap()
}));
pub fn default_font() -> Font<'static> { default_font_files.each_ref().map(|x| std::ops::Deref::deref(x)) }

#[allow(non_upper_case_globals)] pub fn default_color() -> Color { foreground() }
#[allow(non_upper_case_globals)] pub fn bold() -> [Attribute::<Style>; 1] { [Style{color: default_color(), style: FontStyle::Bold}.into()] }

use {std::{sync::Mutex, collections::BTreeMap}, image::Image};
pub static CACHE: Mutex<BTreeMap<(Ratio, GlyphId),(Image<Box<[u8/*u16*/]>>,Image<Box<[u8/*u16*/]>>,Image<Box<[f32]>>)>> = Mutex::new(BTreeMap::new());

pub struct View<'t, D> {
    pub font : Font<'t>,
	pub color: Color,
	pub data: D,
    pub size : Option<size>
}

impl<'t, D> View<'t, D> {
	pub fn new(data: D) -> Self { Self{font: default_font(), color: default_color(), data, size: None} }
	pub fn with_color(color: Color, data: D) -> Self { Self{font: default_font(), color, data, size: None} }
	pub fn with_face(face : &'t Face<'t>, data: D) -> Self { Self{font: [&face, &face], color: default_color(),data, size: None} }
}

pub fn fit_width(width: u32, from : size) -> size { if from.x == 0 { return zero(); } xy{x: width, y: u32::div_ceil(width * from.y, from.x)} }
pub fn fit_height(height: u32, from : size) -> size { if from.y == 0 { return zero(); } xy{x: u32::div_ceil(height * from.x, from.y), y: height} }
pub fn fit(size: size, from: size) -> size { if size.x*from.y < size.y*from.x { fit_width(size.x, from) } else { fit_height(size.y, from) } }

impl<D:AsRef<str>> View<'_, D> {
	pub fn size(&mut self) -> size {
		let &mut Self{ref mut font, ref mut data, ref mut size, ..} = self;
		*size.get_or_insert_with(||{
			let text = data.as_ref();
			let (line_count, max_width) = line_ranges(&text).fold((0,0),|(line_count, width), line| (line_count+1, max(width, metrics(layout(font, &line).into_iter()).width)));
			//assert!(max_width > 0);
			xy{x: max_width, y: line_count * (font[0].height() as u32)}
		})
	}
	#[track_caller] pub fn size_scale(&mut self, fit: size) -> (size, Ratio) {
		if self.data.as_ref().is_empty() { return (zero(), unit) }
		let size = Self::size(self);
		assert!(size > zero() && fit > zero(), "{size} {fit} '{}'", self.data.as_ref());
		(size, if fit.x*size.y < fit.y*size.x { Ratio{num: fit.x-1, div: size.x-1} } else { Ratio{num: fit.y-1, div: size.y-1} }) // Fit
		//(size, if size.is_zero() { Ratio{num: 1, div: 1} } else { Ratio{num: fit.x-1, div: size.x-1} }) // Fit width
	}
	#[track_caller] pub fn scale(&mut self, fit: size) -> Ratio { self.size_scale(fit).1 }
}

impl<D:AsRef<str>> std::fmt::Debug for View<'_, D> { fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { self.data.as_ref().fmt(f) } }

#[derive(PartialEq,Eq,PartialOrd,Ord,Clone,Copy,Debug)] pub struct LineColumn {
	pub line: usize,
	pub column: TextIndex, //GraphemeIndex // May be on the right of the corresponding line (preserves horizontal affinity during line up/down movement)
}

pub fn index(text: &str, LineColumn{line, column}: LineColumn) -> /*GraphemeIndex*/TextIndex {
	let Range{start, end} = line_ranges(text).nth(line).unwrap().range;
	assert!(start+column <= end);
	start+column
}

impl LineColumn {
	pub fn from_text_index(text: &str, index: TextIndex/*GraphemeIndex*/) -> Option<Self> {
		let (line, LineRange{range: Range{start,..}, ..}) = line_ranges(text).enumerate().find(|&(_,LineRange{range: Range{start,end},..})| start <= index && index <=/*\n*/ end)?;
		Some(Self{line, column: index-start})
	}
}

#[derive(PartialEq,Clone,Copy,Debug)] pub struct Span {
	pub start: LineColumn,
	pub end: LineColumn,
}
impl Span {
	pub fn new(end: LineColumn) -> Self { Self{start: end, end} }
	pub fn min(&self) -> LineColumn { min(self.start, self.end) }
	pub fn max(&self) -> LineColumn { max(self.start, self.end) }
}

pub(crate) mod iter;
use image::bgr;
use iter::NthOrLast;
fn position(font: &Font<'_>, text: &str, LineColumn{line, column}: LineColumn) -> uint2 {
	if text.is_empty() { assert!(line==0&&column==0); zero() } else {
	xy{
		x: layout(font, &line_ranges(text).nth(line).unwrap()).into_iter().nth_or_last(column as usize).map_or_else(
			|last| last.map_or(0, |Glyph{x,id,face,..}| x+face.glyph_hor_advance(id).unwrap() as u32),
			|layout| layout.x
		) as u32,
		y: (line as u32)*(font[0].height() as u32)
	}
}}

impl<D:AsRef<str>> View<'_, D> {
	pub fn text(&self) -> &str { AsRef::<str>::as_ref(&self.data) }
	fn position(&self, cursor: LineColumn) -> uint2 { self::position(&self.font, self.text(), cursor) }
	pub fn span(&self, min: LineColumn, max: LineColumn) -> Rect {
		Rect{min: self.position(min).signed(), max: (self.position(max)+xy{x:0, y: self.font[0].height() as u32}).signed()}
	}
	/*pub fn cursor(&mut self, size: size, position: uint2) -> LineColumn {
		let position = position / self.scale(size);
		let View{font, ..} = &self;
		let line = ((position.y/font[0].height() as u32) as usize).min(line_ranges(self.text()).count()-1);
		LineColumn{line, column:
			layout(font, &line_ranges(self.text()).nth(line).unwrap()).into_iter()
			.map(|Glyph{byte_index, x, id, face}| (byte_index, x+face.glyph_hor_advance(id).unwrap() as u32/2))
			.take_while(|&(_, x)| x <= position.x).last().map(|(index,_)| index+1).unwrap_or(0)
		}
	}*/
	/*pub fn paint_span(&self, target: &mut Target, scale: Ratio, offset: int2, span: Span, bgr: crate::color::bgr<bool>) {
		let [min, max] = [span.min(), span.max()];
		let mut invert = |r:Rect| Some(image::invert(&mut target.slice_mut_clip(scale*(-offset+r))?, bgr));
		if min.line < max.line { invert(self.span(min,LineColumn{line: min.line, column: usize::MAX})); }
		if min.line == max.line {
			if min != max { invert(self.span(min,max)); } // selection
			else { // cursor
				fn widen(l: Rect, dx: u32) -> Rect { Rect{min: l.min-xy{x:dx/2,y:0}.signed(), max:l.max+xy{x:dx/2,y:0}.signed()} }
				invert(widen(self.span(min,max), self.font[0].height() as u32/16));
			}
		}
		else { for line in min.line+1..max.line {
			invert(self.span(LineColumn{line, column: 0},LineColumn{line, column: usize::MAX}));
		}}
		if max.line > min.line { invert(self.span(LineColumn{line: max.line, column: 0}, max)); }
	}*/
}

impl<D:AsRef<str>+AsRef<[Attribute<Style>]>> View<'_, D> {
	pub fn paint(&mut self, target: &mut Target, size: size, scale: Ratio, offset: int2) {
		let Self{font, data, ..} = &*self;
		//let (mut style, mut styles) = (None, AsRef::<[Attribute<Style>]>::as_ref(&data).iter().peekable());
		for (line_index, line) in line_ranges(&data.as_ref()).enumerate()
																						.take_while({let clip = (size.y/scale) as i32 - offset.y; move |&(line_index,_)| (((line_index as u32)*(font[0].height() as u32)) as i32) < clip}) {
			for (bbox, Glyph{/*byte_*/_index: _, x, id, face}) in bbox(layout(font, &line).into_iter()) {
				//let byte_index = line.range.start+byte_index;
				/*style = style.filter(|style:&&Attribute<Style>| style.contains(&byte_index));
				while let Some(next) = styles.peek() {
					if next.end <= byte_index { styles.next(); } // skips whitespace style
					else if next.contains(&byte_index) { style = styles.next(); }
					else { break; }
				}*/

				let mut cache = CACHE.lock().unwrap();
				//let (coverage_pq10, _one_minus_coverage_pq10, coverage) = cache.entry((scale, id)).or_insert_with(|| {
				let (coverage_sRGB8, _one_minus_coverage_sRGB8, coverage) = cache.entry((scale, id)).or_insert_with(|| {
					let linear = font::rasterize(face, scale, id, bbox);
					//(image::PQ10_from_linear(&linear.as_ref()), Image::from_iter(linear.size, linear.data.iter().map(|&v| image::PQ10(1.-v))), linear)
					(image::sRGB8_from_linear(&linear.as_ref()), Image::from_iter(linear.size, linear.data.iter().map(|&v| image::sRGB8(1.-v))), linear)
				});

				let position = xy{
					x: (x as i32+face.glyph_hor_side_bearing(id).unwrap() as i32),
					y: (line_index as i32)*(font[0].height() as i32) + font[0].ascender() as i32
				};

				let offset = vector::ifloor(scale, offset + position) - xy{x:0, y: scale.iceil(bbox.max.y)};
				let target_size = target.size.signed() - offset;
				let target_offset = vector::component_wise_max(zero(), offset).unsigned();
				let source_offset = vector::component_wise_max(zero(), -offset);
				let source_size = coverage.size.signed() - source_offset;
				let size = vector::component_wise_min(source_size, target_size);
				if size.x > 0 && size.y > 0 {
					let size = size.unsigned();
					let color = /*style.map(|x|x.attribute.color)*/None.unwrap_or(self.color);
					if true { // Interpolate in compressed instead of linear domain (approximation but avoid EOTF+OETF transfer function evaluation)
						if color == crate::white {
							let white = u32::from(crate::white);
							target.slice_mut(target_offset, size).zip_map(&coverage_sRGB8.slice(source_offset.unsigned(), size),
								|target, &t| match t {
									0 => *target,
									0xFF => white,
									_ => image::bgr8::from(*target).map(|target| {
										// using sRGB gamma compressed value as linear interpolation coefficient.
										// Corrects interpolation for black background case (same as linear coefficient with linear domain points)
										((255-t) as u16*(target as u16)/255) as u8 + t
									}).into(),
								}
							);
							/*target.slice_mut(target_offset, size).zip_map(&coverage.slice(source_offset.unsigned(), size),
								|&target, &t| {
									let t = (t*255.) as u8;
									let tt = (255-t) as u16;
									image::bgr8::from(target).map(|target| (tt*(target as u16)/255) as u8 + t).into()
							});*/
						} else {
							let color = image::bgr8::from(color); // sRGB8_OETF: linear -> sRGB
							//let time = std::time::Instant::now();
							/*target.slice_mut(target_offset, size).zip_map(&coverage.slice(source_offset.unsigned(), size),
								|&target, &t| image::bgr8::from(target).zip(color).map(|(target, color)| num::lerp(t, target as f32, color as f32) as u8).into());*/
							target.slice_mut(target_offset, size).zip_map(&coverage.slice(source_offset.unsigned(), size),
								|&target, &t| {
									let t = (t*256.) as u16;
									let tt = 256 - t;
									image::bgr8::from(target).zip(color).map(|(target, color)| ((tt * (target as u16) + t * (color as u16))/256) as u8).collect::<bgr<_>>().into()
								}
							);
							//eprintln!("{:?}", time.elapsed());
						}
					} else {
						panic!("{color:?}");
						//image::blend(&coverage.slice(source_offset.unsigned(), size), &mut target.slice_mut(target_offset, size), color);
					}
				}
			}
		}
	}
	#[track_caller] pub fn paint_fit(&mut self, target: &mut Target, size: size, offset: int2) -> Ratio {
		let scale = self.scale(size);
		self.paint(target, size, scale, offset);
		scale
	}
}
use crate::widget::{Widget, Target};
impl<'f, D:AsRef<str>+AsRef<[Attribute<Style>]>> Widget for View<'f, D> {
	fn size(&mut self, size: size) -> size { fit(size/*fit_width(size.x*/, Self::size(self)) }
	#[track_caller] #[throws] fn paint(&mut self, target: &mut Target, size: size, offset: int2) { self.paint_fit(target, size, offset); }
}

pub struct Plain<T>(pub T);
impl<T:AsRef<str>> AsRef<str> for Plain<T> { fn as_ref(&self) -> &str { self.0.as_ref() } }
impl<T> AsRef<[Attribute<Style>]> for Plain<T> {  fn as_ref(&self) -> &[Attribute<Style>] { &[] } }
pub fn text<'t>(text: &'t str) -> View<'static, Plain<&'t str>> { View::new(Plain(text)) }
pub type Text = View<'static, Plain<&'static str>>;

pub struct Buffer<T, S> {
	pub text : T,
	pub style: S,
}
impl<T:AsRef<str>,S> AsRef<str> for Buffer<T,S> { fn as_ref(&self) -> &str { self.text.as_ref() } }
impl<T,S:AsRef<[Attribute<Style>]>> AsRef<[Attribute<Style>]> for Buffer<T,S> { fn as_ref(&self) -> &[Attribute<Style>] { self.style.as_ref() } }
pub type Borrowed<'t> = Buffer<&'t str, &'t [Attribute<Style>]>;
//pub fn text<'t>(text: &'t str, style: &'t [Attribute<Style>]) -> View<'static, Borrowed<'t>> { View::new(Borrowed{text, style}) }
pub fn with_color<'t>(color: Color, text: &'t str, style: &'t [Attribute<Style>]) -> View<'static, Borrowed<'t>> { View::with_color(color, crate::text::Borrowed{text, style}) }