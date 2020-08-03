use {std::cmp::{min,max}, core::{error::{throws, Error}, num::{Zero, clamp}, iter::NthOrLast}, ::xy::{xy, uint2, Rect}};

#[derive(PartialEq,Eq,PartialOrd,Ord,Clone,Copy)] struct LineColumn {
	line: usize,
	column: usize // May be on the right of the corresponding line (preserves horizontal affinity during line up/down movement)
}
impl Zero for LineColumn { fn zero() -> Self { Self{line: 0, column: 0} } }

#[derive(PartialEq)] struct Span {
	start: LineColumn,
	end: LineColumn,
}
impl Zero for Span { fn zero() -> Self { Self{start: Zero::zero(), end: Zero::zero()} } }
impl Span {
	fn new(end: LineColumn) -> Self { Self{start: end, end} }
	fn min(&self) -> LineColumn { min(self.start, self.end) }
	fn max(&self) -> LineColumn { max(self.start, self.end) }
}
use crate::{text::{Attribute, Style, line_ranges, layout, Glyph, View, default_style}, widget::{Event, Widget, size, Target, ModifiersState}};

fn position(font: &ttf_parser::Face<'_>, text: &str, LineColumn{line, column}: LineColumn) -> uint2 { xy{
	x: layout(font, line_ranges(text).nth(line).unwrap().char_indices()).nth_or_last(column).map_or_else(
		|last| last.map_or(0, |Glyph{x,id,..}| x+(font.glyph_hor_advance(id).unwrap() as i32)),
		|layout| layout.x
	) as u32,
	y: (line as u32)*(font.height() as u32)
}}
fn span(font: &ttf_parser::Face<'_>, text: &str, min: LineColumn, max: LineColumn) -> Rect {
	Rect{min: position(font, text, min).signed(), max: (position(font, text, max)+xy{x:0, y: font.height() as u32}).signed()}
}

pub struct Buffer<T, S> {
	pub text : T,
	pub style: S,
}
type Borrowed<'t> = Buffer<&'t str, &'t [Attribute<Style>]>;
type Owned = Buffer<String, Vec<Attribute<Style>>>;
trait ToOwned { type Owned; fn to_owned(&self) -> Self::Owned; }
impl ToOwned for Borrowed<'_> {
	type Owned = Owned;
	fn to_owned(&self) -> Self::Owned { Self::Owned{text: self.text.to_owned(), style: self.style.to_owned()} }
}
pub enum Cow<'t> { 
    Borrowed(Borrowed<'t>),
    Owned(Owned)
}
impl Cow<'_> { fn get_mut(&mut self) -> &mut Owned { if let Cow::Borrowed(b) = self  { *self = Cow::Owned(b.to_owned()) } if let Cow::Owned(o) = self { o } else { unreachable!() } }  }

impl AsRef<str> for Cow<'_> { fn as_ref(&self) -> &str { match self { Cow::Borrowed(b) => b.text, Cow::Owned(o) => &o.text} } }
impl AsRef<[Attribute<Style>]> for Cow<'_> { fn as_ref(&self) -> &[Attribute<Style>] { match self { Cow::Borrowed(b) => b.style, Cow::Owned(o) => &o.style} } }

impl Cow<'t> { pub fn new(text: &'t str) -> Self { Cow::Borrowed(Borrowed{text, style: &*default_style}) } }

pub struct Edit<'f, 't> {
	view: View<'f, Cow<'t>>,
	selection: Span,
}

impl<'f, 't> Edit<'f, 't> {	pub fn new(font: &'f ttf_parser::Face<'font>, data: Cow<'t>) -> Self { Self{view: View{font, data}, selection: Zero::zero()} } }

impl Widget for Edit<'_,'_> {
	fn size(&mut self, size : size) -> size { Widget::size(&mut self.view, size) }
	#[throws] fn paint(&mut self, target : &mut Target) {
		let Self{view, selection} = self;
		let scale = view.scale(target.size);
		view.paint(target, scale);
		/*if has_focus*/ {
			let [min, max] = [selection.min(), selection.max()];
			let View{font, data} = view;
			let text = AsRef::<str>::as_ref(&data);
			if min.line < max.line { image::invert(&mut target.slice_mut_clip(scale*span(font,text,min,LineColumn{line: min.line, column: usize::MAX}))); }
			if min.line == max.line { image::invert(&mut target.slice_mut_clip(scale*span(font,text,min,max))); }
			else { for line in min.line+1..max.line {
				image::invert(&mut target.slice_mut_clip(scale*span(font,text,LineColumn{line, column: 0},LineColumn{line, column: usize::MAX})));
			}}
			if max.line > min.line { image::invert(&mut target.slice_mut_clip(scale*span(font,text,LineColumn{line: max.line, column: 0}, max))); }
			pub fn widen(l: Rect, dx: u32) -> Rect { Rect{min: l.min-xy{x:dx/2,y:0}.signed(), max:l.max+xy{x:dx/2,y:0}.signed()} }
			image::invert(&mut target.slice_mut_clip(scale*widen(span(font,text,selection.end,selection.end), font.height() as u32/16)));
		}
	}
	fn event(&mut self, &Event{key, modifiers_state: ModifiersState{ctrl,shift,..}}: &Event) -> bool {
		let Self{view: View{data, ..}, selection, ..} = self;
		let text = AsRef::<str>::as_ref(&data);
		if text.is_empty() { return false; }
		if selection.start != selection.end && !shift { // Left|Right clear moves caret to selection min/max
			if key == '←' { *selection=Span::new(selection.min()); return true; }
			if key == '→' { *selection=Span::new(selection.max()); return true; }
		}
		let mut edit = None;
		enum Change { Insert(usize, char) } use Change::*;
		let LineColumn{line, column} = selection.end;
		let (line_text, line_count) = { let mut line_iterator = line_ranges(text); (line_iterator.nth(line).unwrap(), line+1+line_iterator.count()) };
		let line = line as i32;
		use core::unicode_segmentation::{prev_boundary,next_boundary,prev_word,next_word};
		let (line, column) = match key {
			'↑' => (line - 1, column),
			'↓' => (line + 1, column),
			'⇞' => (line - 30, column),
			'⇟' => (line + 30, column),
			'←' => if column == 0 { if line == 0 { return false; } (line-1, line_ranges(text).nth((line-1) as usize).unwrap().len()) }
						else { (line, if ctrl {prev_word} else {prev_boundary}(&line_text, column)) },
			'→' => if column >= line_text.len() { if line >= line_count as i32-1 { return false; } (line+1, 0) }
						else { (line, if ctrl {next_word} else {next_boundary}(&line_text, column)) },
			'⇱' => (if ctrl {0} else {line}, 0),
			'⇲' => if ctrl {(line_count as i32-1, line_ranges(text).nth(line_count-1).unwrap().len())} else {(line, line_text.len())},
			char if !key.is_control() => {
				edit = Some(Insert(line_text.range.start+column, char));
				(line, column+1)
			}
			_ => unimplemented!(),
		};
		drop(text);
		if let Some(edit) = edit { 
			let text = &mut data.get_mut().text; // todo: style
			match edit {
				Insert(at, char) => text.insert(at, char),
			}
		}
		let end = LineColumn{line: clamp(0, line, line_count as i32-1) as usize, column};
		let next = Span{start: if shift { selection.start } else { end }, end};
		if next == *selection { false } else { *selection = next; true }
	}
}
