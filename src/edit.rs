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

use {std::cmp::min, core::{error::{throws, Error}, num::{Zero, clamp}}, ::xy::{xy, int2, Rect}, image::bgra8};

#[derive(PartialEq)] struct LineColumn {
	line: usize,
	column: usize // May be on the right of the corresponding line (preserves horizontal affinity during line up/down movement)
}
impl Zero for LineColumn { fn zero() -> Self { Self{line: 0, column: 0} } }

use crate::{widget::{Widget, size, Target, Event, Key}, text::{Text, line_ranges, layout, Glyph}};

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
    fn event(&mut self, key: &Event) -> bool {
    	let Self{text: Text{text, ..}, cursor} = self;
    	if text.is_empty() { return false; }
    	let LineColumn{line, column} = *cursor;
    	let (line_text, line_count) = { let mut line_iterator = line_ranges(text); (line_iterator.nth(line).unwrap(), (line+1+line_iterator.count()) as i32) };
		let line = line as i32;
		use Key::*;
		let (line, column) = match key {
			Up => (line - 1, column),
			Down => (line + 1, column),
			PageUp => (line - 30, column),
			PageDown => (line + 30, column),
			Left => if column == 0 { if line == 0 { return false; } (line-1, line_ranges(text).nth((line-1) as usize).unwrap().len()) }
						else { (line, prev_boundary(&line_text, column)) },
			Right => if column >= line_text.len() { if line >= line_count-1 { return false; } (line+1, 0) } else { (line, next_boundary(&line_text, column)) },
			Home => (line, 0),
			End => (line, line_text.len()),
			_ => return false,
		};
		let next = LineColumn{line: clamp(0, line, line_count-1) as usize, column};
		if next == *cursor { false } else { *cursor = next; true }
	}
}
