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

use {core::{error::{throws, Error}, num::Zero}, xy::xy, image::bgra8, crate::{widget::{Widget, size, Target, Event, Key}, text::{Text, line_ranges, layout, Glyph}}};

struct LineColumn{line: usize, column: usize}
impl Zero for LineColumn { fn zero() -> Self { Self{line: 0, column: 0} } }

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
            let scale = self.text.scale(&target);
            let Self{text: Text{text, font, ..}, cursor} = self;
            let &mut LineColumn{line: line_index, column} = cursor;
            let line = line_ranges(text).nth(line_index).unwrap();
            let position = xy{
                x: layout(font, line.char_indices()).nth_or_last(column).map_or_else(
                        |last| last.map_or(0, |Glyph{x,id,..}| x+(font.glyph_hor_advance(id).unwrap() as i32)),
                        |layout| layout.x
					) as u32,
                y: (line_index as u32)*(font.height() as u32)
            };
            let height = font.height() as u32;
            target.slice_mut(scale*position, scale*xy{x:height/16,y:height}).modify(|bgra8{b,g,r,..}| bgra8{b:0xFF-b, g:0xFF-g, r:0xFF-r, a:0xFF});
        }
    }
    fn event(&mut self, event: &Event) -> bool {
		use Key::*;
		match event {
			Right => {
				let Self{text: Text{text, ..}, cursor: LineColumn{line: line_index, column}} = self;
				let (line, last) = { let mut line_iterator = line_ranges(text); (line_iterator.nth(*line_index).unwrap(), line_iterator.next().is_none()) };
				if let Some(next) = unicode_segmentation::GraphemeCursor::new(*column, line.len(), true).next_boundary(&line, 0).unwrap() { *column = next; true }
				else { if !last { *column = 0; *line_index += 1; true } else { false } }
			},
			_ => false,
		}
	}
}
