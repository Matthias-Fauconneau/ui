mod none;
use {std::cmp::{min,max}, fehler::throws, error::Error, num::Zero, iter::{Single, NthOrLast}, ::xy::{xy, uint2, Rect}};
use crate::{text::{unicode_segmentation::{self, GraphemeIndex, UnicodeSegmentation, prev_word, next_word},
														LineColumn, Attribute, Style, line_ranges, layout, Glyph, View, default_style},
									 widget::{Event, EventContext, Widget, size, Target, ModifiersState, ButtonState::Pressed}};

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
	x: layout(font, line_ranges(text).nth(line).unwrap().graphemes(true).enumerate()).nth_or_last(column as usize).map_or_else(
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
pub type Owned = Buffer<String, Vec<Attribute<Style>>>;
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

type Changed<T> = Option<Box<dyn Fn(&mut T)>>;

pub struct Edit<'f, 't> {
	view: View<'f, Cow<'t>>,
	selection: Span,
	history: Vec<State>,
	history_index: usize,
	last_change: Change,
	compose: Option<Vec<char>>,
	text_changed: Changed<Owned>,
}

impl<'f, 't> Edit<'f, 't> {
	pub fn new(font: &'f ttf_parser::Face<'font>, data: Cow<'t>, text_changed: Changed<Owned>) -> Self {
		Self{view: View{font, data}, selection: Zero::zero(), history: Vec::new(), history_index: 0, last_change: Change::Other, compose: None, text_changed}
	}
}

const fn empty() -> String { String::new() }
const fn nothing() -> String { String::new() }
use std::{sync::Mutex, lazy::SyncLazy}; pub static CLIPBOARD : SyncLazy<Mutex<String>> = SyncLazy::new(|| Mutex::new(empty()));

pub static COMPOSE: SyncLazy<Vec<(Vec<char>, char)>> = SyncLazy::new(|| {
	std::str::from_utf8(&std::fs::read("compose").unwrap()).unwrap().lines().map(|line| {
		let mut fields = line.split_ascii_whitespace();
		(fields.next().unwrap().chars().collect(), fields.next().unwrap().chars().single().unwrap())
	}).collect()
});

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
		let Self{ref mut view, selection, history, history_index, last_change, compose, text_changed, ..} = self;
		let change = match event {
			&Event::Key{key} => (||{
				match key {
					'⎋' => { *compose = None; if selection.start != selection.end && !shift { *selection=Span::new(selection.end); return Change::Cursor; } else { return Change::None; } }
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
					'⎄' => { if let Some(compose) = compose { compose.push(key); } else { *compose = Some(Vec::new()); } return Change::None; }
					_ => {}
				}
				if selection.start != selection.end && !shift { // Left|Right clear moves caret to selection min/max
					if key == '←' { *selection=Span::new(selection.min()); return Change::Cursor; }
					if key == '→' { *selection=Span::new(selection.max()); return Change::Cursor; }
				}

				let text = AsRef::<str>::as_ref(&view.data);
				if text.is_empty() { return Change::None; }

				#[derive(Default,PartialEq)] struct ReplaceRange { range: std::ops::Range<GraphemeIndex>, replace_with: String }
				impl none::Default for ReplaceRange {}
				let mut replace_range = none::None::none();

				let mut change = Change::Other;

				let index = |LineColumn{line, column}| line_ranges(text).nth(line).unwrap().range.start+column;
				let index = |span:&Span| index(span.min())..index(span.max());

				let line_text = |line| line_ranges(text).nth(line).unwrap();
				let line_count = line_ranges(text).count();

				let LineColumn{line, column} = selection.end;
				let prev = || {
					if column > 0 { LineColumn{line, column: if ctrl { prev_word(&line_text(line), column) } else { column-1 } } }
					else if line > 0 { LineColumn{line: line-1, column: line_text(line-1).len()} }
					else { LineColumn{line, column} }
				};
				let next = || {
					if column < line_text(line).len() { LineColumn{line, column: if ctrl { next_word(&line_text(line), column) } else { column+1 }} }
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
						return Change::None;
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
					char if (!key.is_control() || key=='\t') && !ctrl => {
						let char = if shift { char.to_uppercase().single().unwrap() } else { char };
						let char = if let Some(sequence) = compose {
							sequence.push(char);
							let mut candidates = COMPOSE.iter().filter(|(k,_)| k.starts_with(sequence));
							match (candidates.next(), candidates.next()) {
								(Some(&(_, result)), None) => { *compose = None; result }
								(None, _) => { *compose = None; char }
								(Some(_), Some(_)) => { return Change::None; }
							}
						} else { char };
						change = Change::Insert;
						replace_range = ReplaceRange{range: index(selection), replace_with: char.to_string()};
						LineColumn{line: selection.min().line, column: selection.min().column+1} // after insertion
					}
					key => { println!("{:?}", key); selection.end },
				};
				use none::IsNone;
				if let Some(ReplaceRange{range, replace_with}) = replace_range.to_option() {
					history.truncate(*history_index);
					if !((change==Change::Insert || change==Change::Remove) && change == *last_change) { history.push(State{text: text.to_owned(), cursor: selection.end}); }
					*history_index = history.len();
					let range = unicode_segmentation::index(text, range.start)..unicode_segmentation::index(text, range.end);
					view.data.get_mut().text.replace_range(range, &replace_with);
					*selection = Span::new(end);
					if let Some(f) = text_changed { f(view.data.get_mut()); }
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
