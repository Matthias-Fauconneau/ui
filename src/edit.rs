use {std::cmp::{min,max}, core::{error::{throws, Error}, num::Zero, iter::NthOrLast}, ::xy::{xy, uint2, Rect}};
use crate::{text::{LineColumn, Attribute, Style, line_ranges, layout, Glyph, View, default_style}, widget::{Event, EventContext, Widget, size, Target, ModifiersState, ButtonState::Pressed}};

#[derive(PartialEq,Clone,Copy)] struct Span {
	start: LineColumn,
	end: LineColumn,
}
impl Zero for Span { fn zero() -> Self { Self{start: Zero::zero(), end: Zero::zero()} } }
impl Span {
	fn new(end: LineColumn) -> Self { Self{start: end, end} }
	fn min(&self) -> LineColumn { min(self.start, self.end) }
	fn max(&self) -> LineColumn { max(self.start, self.end) }
}

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

impl Cow<'t> { pub fn new(text: &'t str) -> Self { Cow::Borrowed(Borrowed{text, style: &default_style}) } }

struct State {
	text: String, // fixme: diff
	cursor: LineColumn
}

#[derive(PartialEq)] enum Change { None, Cursor, Insert, Remove, Other }

pub struct Edit<'f, 't> {
	view: View<'f, Cow<'t>>,
	selection: Span,
	history: Vec<State>,
	history_index: usize,
	last_change: Change,
}

impl<'f, 't> Edit<'f, 't> {
	pub fn new(font: &'f ttf_parser::Face<'font>, data: Cow<'t>) -> Self {
		Self{view: View{font, data}, selection: Zero::zero(), history: Vec::new(), history_index: 0, last_change: Change::Other}
	}
}

const fn nothing() -> String { String::new() }
use std::{sync::Mutex, lazy::SyncLazy}; pub static CLIPBOARD : SyncLazy<Mutex<String>> = SyncLazy::new(|| Mutex::new(nothing()));

impl Widget for Edit<'_,'_> {
	fn size(&mut self, size : size) -> size { Widget::size(&mut self.view, size) }
	#[throws] fn paint(&mut self, target : &mut Target) {
		let Self{view, selection, ..} = self;
		let scale = view.scale(target.size);
		view.paint(target, scale);
		/*if has_focus*/ {
			let [min, max] = [selection.min(), selection.max()];
			let View{font, data} = view;
			let text = AsRef::<str>::as_ref(&data);
			if min.line < max.line { image::invert(&mut target.slice_mut_clip(scale*span(font,text,min,LineColumn{line: min.line, column: usize::MAX}))); }
			if min.line == max.line {
				if min == max { // cursor
					pub fn widen(l: Rect, dx: u32) -> Rect { Rect{min: l.min-xy{x:dx/2,y:0}.signed(), max:l.max+xy{x:dx/2,y:0}.signed()} }
					image::invert(&mut target.slice_mut_clip(scale*widen(span(font,text,selection.end,selection.end), font.height() as u32/16)));
				}
				if min != max { // selection
					image::invert(&mut target.slice_mut_clip(scale*span(font,text,min,max)));
				}
			}
			else { for line in min.line+1..max.line {
				image::invert(&mut target.slice_mut_clip(scale*span(font,text,LineColumn{line, column: 0},LineColumn{line, column: usize::MAX})));
			}}
			if max.line > min.line { image::invert(&mut target.slice_mut_clip(scale*span(font,text,LineColumn{line: max.line, column: 0}, max))); }
		}
	}
	fn event(&mut self, size : size, EventContext{modifiers_state: ModifiersState{ctrl,shift,..}, pointer}: EventContext, event: &Event) -> bool {
		let Self{view, selection, history, history_index, last_change, ..} = self;
		let change = match event {
			&Event::Key{key} => (||{
				match key {
					'⎋' => if selection.start != selection.end && !shift { *selection=Span::new(selection.end); return Change::Cursor; } else { return Change::None; }
					'⇧'|'⇪'|'⌃'|'⌘'|'⎇'|'⎀'|'⎙' => return Change::None,
					'z' if ctrl => {
						if !shift && *history_index > 0 {
							if *history_index == history.len() { history.push(State{text: std::mem::replace(&mut view.data.get_mut().text, String::new()), cursor: selection.end}); }
							*history_index -= 1;
						}
						else if shift && *history_index+1 < history.len() { *history_index += 1; }
						else { return Change::None; }
						let State{text, cursor} = &history[*history_index];
						view.data.get_mut().text = text.clone();
						// todo: style
						*selection = Span::new(*cursor);
						return Change::Other;
					},
					_ => {}
				}
				if selection.start != selection.end && !shift { // Left|Right clear moves caret to selection min/max
					if key == '←' { *selection=Span::new(selection.min()); return Change::Cursor; }
					if key == '→' { *selection=Span::new(selection.max()); return Change::Cursor; }
				}

				let text = AsRef::<str>::as_ref(&view.data);
				if text.is_empty() { return Change::None; }

				#[derive(Default,PartialEq)] struct ReplaceRange { range: std::ops::Range<usize>, replace_with: String }
				impl core::none::Default for ReplaceRange {}
				let mut replace_range = core::none::None::none();

				let mut change = Change::Other;

				use core::unicode_segmentation::{prev_boundary,next_boundary,prev_word,next_word};
				let index = |LineColumn{line, column}| line_ranges(text).nth(line).unwrap().range.start+column;
				let index = |span:&Span| index(span.min())..index(span.max());

				let line_text = |line| line_ranges(text).nth(line).unwrap();
				let line_count = line_ranges(text).count();

				let LineColumn{line, column} = selection.end;
				let prev = || {
					if column > 0 { LineColumn{line, column: if ctrl {prev_word} else {prev_boundary}(&line_text(line), column)} }
					else if line > 0 { LineColumn{line: line-1, column: line_text(line-1).len()} }
					else { LineColumn{line, column} }
				};
				let next = || {
					if column < line_text(line).len() { LineColumn{line, column: if ctrl {next_word} else {next_boundary}(&line_text(line), column)} }
					else if line < line_count-1 { LineColumn{line: line+1, column: 0} }
					else { LineColumn{line, column} }
				};

				let end = match key {
					'↑' => LineColumn{line: max(line as i32 - 1, 0) as usize, column},
					'↓' => LineColumn{line: min(line+1, line_count-1), column},
					'⇞' => LineColumn{line: max(line as i32 - 30, 0) as usize, column},
					'⇟' => LineColumn{line: min(line+30, line_count-1), column},
					'←' => prev(),
					'→' => next(),
					'⇤' => LineColumn{line: if ctrl {0} else {line}, column: 0},
					'⇥' => {
						if ctrl {LineColumn{line: line_count-1, column: line_text(line_count-1).len()}}
						else {LineColumn{line, column: line_text(line).len()}}
					},
					'\n' => {
						replace_range = ReplaceRange{range: index(selection), replace_with: '\n'.to_string()};
						// todo: indentation
						LineColumn{line: line+1, column: 0}
					}
					'⌫' => {
						change = Change::Remove;
						let mut selection = *selection;
						if selection.start == selection.end { selection.end = prev(); }
						replace_range = ReplaceRange{range: index(&selection), replace_with: nothing()};
						selection.min()
					}
					'⌦' => {
						change = Change::Remove;
						let mut selection = *selection;
						if selection.start == selection.end { selection.end = next(); }
						replace_range = ReplaceRange{range: index(&selection), replace_with: nothing()};
						selection.min() // after deletion
					}
					'c' if ctrl && selection.start != selection.end => {
						*CLIPBOARD.lock().unwrap() = text[index(selection)].to_owned();
						selection.end
					}
					'x' if ctrl && selection.start != selection.end => {
						change = Change::Remove;
						*CLIPBOARD.lock().unwrap() = text[index(selection)].to_owned();
						replace_range = ReplaceRange{range: index(selection), replace_with: nothing()};
						selection.min()
					}
					'v' if ctrl => {
						let clipboard = CLIPBOARD.lock().unwrap();
						let line_count = line_ranges(&clipboard).count();
						let column = if line_count == 1 { selection.min().column+clipboard.len() } else { line_ranges(&clipboard).nth(line_count-1).unwrap().len() };
						replace_range = ReplaceRange{range: index(selection), replace_with: clipboard.clone()};
						LineColumn{line: selection.min().line+line_count-1, column} // after deletion+insertion
					}
					char if !key.is_control() && !ctrl => {
						change = Change::Insert;
						replace_range = ReplaceRange{range: index(selection), replace_with: if shift { char.to_uppercase().to_string() } else { char.to_string() }};
						LineColumn{line: selection.min().line, column: selection.min().column+1} // after insertion
					}
					key => { println!("{:?}", key); selection.end },
				};
				use core::none::IsNone;
				if let Some(ReplaceRange{range, replace_with}) = replace_range.to_option() {
					history.truncate(*history_index);
					if !((change==Change::Insert || change==Change::Remove) && change == *last_change) { history.push(State{text: text.to_owned(), cursor: selection.end}); }
					*history_index = history.len();
					view.data.get_mut().text.replace_range(range, &replace_with); // todo: style
					*selection = Span::new(end);
					change
				} else {
					let next = Span{start: if shift { selection.start } else { end }, end};
					if next == *selection { Change::None } else { *selection = next; Change::Cursor }
				}
			})(),
			&Event::Motion{position, mouse_buttons} => {
				if let Some(pointer) = pointer { let _ = pointer.set_cursor("text", None); }
				if mouse_buttons != 0 {
					let end = view.cursor(size, position);
					let next = Span{end, ..*selection};
					if next == *selection { Change::None } else { *selection = next; Change::Cursor }
				} else { Change::None }
			},
			&Event::Button{button: 0, position, state: Pressed} => {
				let end = view.cursor(size, position);
				let next = Span{start: if shift { selection.start } else { end }, end};
				if next == *selection { Change::None } else { *selection = next; Change::Cursor }
			},
			_ => { Change::None },
		};
		if change != Change::None { *last_change = change; true } else { false }
	}
}
