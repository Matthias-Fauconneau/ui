trait NthOrLast : Iterator {
	fn nth_or_last(&mut self, mut n: usize) -> Result<Self::Item, Option<Self::Item>> {
		let mut last = None;
		for x in self {
			if n == 0 { return Ok(x); }
			n -= 1;
			last = Some(x);
		}
		Err(last)
	}
}
impl<I:Iterator> NthOrLast for I {}

use unicode_segmentation::GraphemeCursor;
fn prev_boundary(text: &str, from: usize) -> usize { GraphemeCursor::new(min(from, text.len()), text.len(), true).prev_boundary(&text, 0).unwrap().unwrap() }
fn next_boundary(text: &str, from: usize) -> usize { GraphemeCursor::new(min(from, text.len()), text.len(), true).next_boundary(&text, 0).unwrap().unwrap() }

#[derive(PartialEq,Clone,Copy)] enum Class { Space, Alphanumeric, Symbol }
fn classify(g:&str) -> Class {
	use {core::iter::Single, Class::*};
	g.chars().single().map_or(Symbol, |c| if c.is_whitespace() {Space} else if c.is_alphanumeric() {Alphanumeric} else {Symbol} )
}
fn run_class<'t>(graphemes: &mut std::iter::Peekable<impl Iterator<Item=(usize, &'t str)>>, class: Class) -> Option<usize> {
	use core::iter::PeekableExt;
	graphemes.peeking_take_while(|&(_,g)| classify(g) == class).last().map(|(last,_)| last)
}
#[throws(as Option)] fn run<'t>(graphemes: &mut std::iter::Peekable<impl Iterator<Item=(usize, &'t str)>>) -> (usize, Class) {
	let (last, class) = { let (last, g) = graphemes.next()?; (last, classify(g)) };
	(run_class(graphemes, class).unwrap_or(last), class)
}

use unicode_segmentation::UnicodeSegmentation;
#[throws(as Option)] fn last_word_start(text: &str) -> usize {
	let mut graphemes = text.graphemes(true).rev().scan(text.len(), |before, g| { *before -= g.len(); Some((*before, g)) }).peekable();
	match run(&mut graphemes)? {
		(_, Class::Space) => { let (last, _) = run(&mut graphemes)?; last },
		(last, _) => last,
	}
}
#[throws(as Option)] fn next_word_start(text: &str) -> usize {
	let mut graphemes = text.graphemes(true).scan(0, |after, g| { *after += g.len(); Some((*after, g)) }).peekable();
	match run(&mut graphemes)? {
		(last, Class::Space) => last,
		(last, _) => { run_class(&mut graphemes, Class::Space).unwrap_or(last) },
	}
}

fn prev_word(text: &str, from: usize) -> usize { last_word_start(&text[..from]).unwrap_or(from) }
fn next_word(text: &str, from: usize) -> usize { from + next_word_start(&text[from..]).unwrap_or(from) }

use {std::cmp::min, core::{error::{throws, Error}, num::{Zero, clamp}}, ::xy::{xy, int2, Rect}, image::bgra8};

#[derive(PartialEq)] struct LineColumn {
	line: usize,
	column: usize // May be on the right of the corresponding line (preserves horizontal affinity during line up/down movement)
}
impl Zero for LineColumn { fn zero() -> Self { Self{line: 0, column: 0} } }

use crate::{widget::{Widget, size, Target, Event, Key, ModifiersState}, text::{Text, line_ranges, layout, Glyph}};

pub struct TextEdit<'font, 'text> {
	text: Text<'font, 'text>,
	cursor: LineColumn
}

impl<'font, 'text> TextEdit<'font, 'text> { pub fn new(text : Text<'font, 'text>) -> Self { Self{text, cursor: Zero::zero()} } }

impl Widget for TextEdit<'_,'_> {
	fn size(&mut self, size : size) -> size { Widget::size(&mut self.text, size) }
	#[throws] fn paint(&mut self, target : &mut Target) {
		Widget::paint(&mut self.text, target)?;
		/*if self.has_focus()*/ {
			let Self{text: Text{text, font, ..}, cursor} = self;
			let &mut LineColumn{line: line_index, column} = cursor;
			let line = line_ranges(text).nth(line_index).unwrap();
			let height = font.height() as u32;
			let position = xy{
				x: layout(font, line.char_indices()).nth_or_last(column).map_or_else(
					|last| last.map_or(0, |Glyph{x,id,..}| x+(font.glyph_hor_advance(id).unwrap() as i32)),
					|layout| layout.x
				) as u32,
				y: (line_index as u32)*height
			};
			let scale = self.text.scale(&target);
			pub fn top_mid_size(top_mid: int2, size: size) -> Rect { Rect{min: top_mid-xy{x:(size.x/2) as i32, y:0}, max: top_mid+xy{x:(size.x/2) as i32, y:size.y as i32} } }
			target.slice_mut_clip(scale*top_mid_size(position.signed(), xy{x: height/16, y: height})).modify(|bgra8{b,g,r,..}| bgra8{b:0xFF-b, g:0xFF-g, r:0xFF-r, a:0xFF});
		}
	}
	fn event(&mut self, &Event{key, modifiers_state: ModifiersState{ctrl,..}}: &Event) -> bool {
		let Self{text: Text{text, ..}, cursor} = self;
		if text.is_empty() { return false; }
		let LineColumn{line, column} = *cursor;
		let (line_text, line_count) = { let mut line_iterator = line_ranges(text); (line_iterator.nth(line).unwrap(), line+1+line_iterator.count()) };
		let line = line as i32;
		use Key::*;
		let (line, column) = match key {
			Up => (line - 1, column),
			Down => (line + 1, column),
			PageUp => (line - 30, column),
			PageDown => (line + 30, column),
			Left => if column == 0 { if line == 0 { return false; } (line-1, line_ranges(text).nth((line-1) as usize).unwrap().len()) }
						else { (line, if ctrl {prev_word} else {prev_boundary}(&line_text, column)) },
			Right => if column >= line_text.len() { if line >= line_count as i32-1 { return false; } (line+1, 0) }
						else { (line, if ctrl {next_word} else {next_boundary}(&line_text, column)) },
			Home => (if ctrl {0} else {line}, 0),
			End => if ctrl {(line_count as i32-1, line_ranges(text).nth(line_count-1).unwrap().len())} else {(line, line_text.len())},
			_ => return false,
		};
		let next = LineColumn{line: clamp(0, line, line_count as i32-1) as usize, column};
		if next == *cursor { false } else { *cursor = next; true }
	}
}
