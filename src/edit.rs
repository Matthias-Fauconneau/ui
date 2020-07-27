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
use crate::{text::{Text, line_ranges, layout, Glyph}, widget::{Widget, size, Target, Event, ModifiersState}};

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

pub struct TextEdit<'font, 'text> {
	text: Text<'font, 'text>,
	selection: Span,
}

impl<'font, 'text> TextEdit<'font, 'text> { pub fn new(text : Text<'font, 'text>) -> Self { Self{text, selection: Zero::zero()} } }

impl Widget for TextEdit<'_,'_> {
	fn size(&mut self, size : size) -> size { Widget::size(&mut self.text, size) }
	#[throws] fn paint(&mut self, target : &mut Target) {
		Widget::paint(&mut self.text, target)?;
		/*if self.has_focus()*/ {
			let scale = self.text.scale(&target);
			let Self{text: Text{text, font, ..}, selection} = self;
			let [min, max] = [selection.min(), selection.max()];
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
		let Self{text: Text{text, ..}, selection} = self;
		if text.is_empty() { return false; }
		if selection.start != selection.end && !shift { // Left|Right clear moves caret to selection min/max
			if key == '←' { *selection=Span::new(selection.min()); return true }
			if key == '→' { *selection=Span::new(selection.max()); return true }
		}
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
			_ => return false,
		};
		let end = LineColumn{line: clamp(0, line, line_count as i32-1) as usize, column};
		let next = Span{start: if shift { selection.start } else { end }, end};
		if next == *selection { false } else { *selection = next; true }
	}
}
