mod none;
use {std::cmp::{min,max}, fehler::throws, error::Error, num::{Zero, zero, Ratio}, iter::Single, xy::{uint2, Rect}};
use crate::{text::{self, unicode_segmentation::{self, GraphemeIndex, prev_word, next_word},
														LineColumn, Span, Attribute, Style, line_ranges, View, default_style},
									 widget::{Event, EventContext, Widget, size, Target, ModifiersState, ButtonState::Pressed}};

pub struct Buffer<T, S> {
	pub text : T,
	pub style: S,
}
pub type Borrowed<'t> = Buffer<&'t str, &'t [Attribute<Style>]>;

impl AsRef<str> for Borrowed<'_> { fn as_ref(&self) -> &str { self.text } }
impl AsRef<[Attribute<Style>]> for Borrowed<'_> {  fn as_ref(&self) -> &[Attribute<Style>] { self.style } }

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
impl Cow<'_> {
	fn get_mut(&mut self) -> &mut Owned { if let Cow::Borrowed(b) = self  { *self = Cow::Owned(b.to_owned()) } if let Cow::Owned(o) = self { o } else { unreachable!() } }
}
/*impl<'f,'t> View<'f, Cow<'t>> {
	pub fn get_mut(&mut self) -> &mut Cow { self.size = None; &mut self.data }
}*/

impl AsRef<str> for Cow<'_> { fn as_ref(&self) -> &str { match self { Cow::Borrowed(b) => b.text, Cow::Owned(o) => &o.text} } }
impl AsRef<[Attribute<Style>]> for Cow<'_> { fn as_ref(&self) -> &[Attribute<Style>] { match self { Cow::Borrowed(b) => b.style, Cow::Owned(o) => &o.style} } }

impl Cow<'t> { pub fn new(text: &'t str) -> Self { Cow::Borrowed(Borrowed{text, style: &default_style}) } }

struct State {
	text: String, // fixme: diff
	cursor: LineColumn
}

#[derive(PartialEq,Clone,Copy)] pub enum Change { None, Scroll, Cursor, Insert, Remove, Other }

pub struct Edit<'f, 't> {
	pub view: View<'f, Cow<'t>>,
	pub selection: Span,
	history: Vec<State>,
	history_index: usize,
	last_change: Change,
	compose: Option<Vec<char>>,
}

const fn empty() -> String { String::new() }
const fn nothing() -> String { String::new() }
use std::{sync::Mutex, lazy::SyncLazy}; pub static CLIPBOARD : SyncLazy<Mutex<String>> = SyncLazy::new(|| Mutex::new(empty()));

pub static KEYMAP: SyncLazy<Vec<(char, (char,char))>> = SyncLazy::new(|| {
	std::str::from_utf8(&std::fs::read(dirs::config_dir().unwrap().join("keymap")).unwrap()).unwrap().lines().map(|line| {
		let mut chars = line.chars();
		(chars.next().unwrap(), (chars.next().unwrap(), chars.next().unwrap()))
	}).collect()
});

pub static COMPOSE: SyncLazy<Vec<(Vec<char>, char)>> = SyncLazy::new(|| {
	std::str::from_utf8(&std::fs::read(dirs::config_dir().unwrap().join("compose")).unwrap()).unwrap().lines().map(|line| {
		let mut fields = line.split_ascii_whitespace();
		(fields.next().unwrap().chars().collect(), fields.next().unwrap().chars().single().unwrap())
	}).collect()
});

impl<'f, 't> Edit<'f, 't> {
pub fn new(font: &'f ttf_parser::Face<'font>, data: Cow<'t>) -> Self {
	Self{view: View{font, data, size: None}, selection: Zero::zero(), history: Vec::new(), history_index: 0, last_change: Change::Other, compose: None}
}
pub fn event(&mut self, size : size, offset: uint2, EventContext{modifiers_state: ModifiersState{ctrl,shift,alt,..}, pointer}: &EventContext, event: &Event) -> Change {
	let Self{ref mut view, selection, history, history_index, last_change, compose, ..} = self;
	let change = match event {
			&Event::Key{key} => (||{
				let View{data, ..} = view;
				match key {
					'⎋' => { *compose = None; if selection.start != selection.end && !shift { *selection=Span::new(selection.end); return Change::Cursor; } else { return Change::None; } }
					'⇧'|'⇪'|'⌃'|'⌘'|'⎇'|'⎀'|'⎙' => return Change::None,
					'←'| '→' if *alt => return Change::None,
					'z' if *ctrl => {
						if !shift && *history_index > 0 {
							if *history_index == history.len() { history.push(State{text: std::mem::replace(&mut data.get_mut().text, String::new()), cursor: selection.end}); }
							*history_index -= 1;
						}
						else if *shift && *history_index+1 < history.len() { *history_index += 1; }
						else { return Change::None; }
						let State{text, cursor} = &history[*history_index];
						data.get_mut().text = text.clone();
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

				let text = AsRef::<str>::as_ref(&data);
				if text.is_empty() { return Change::None; }

				#[derive(Default,PartialEq)] struct ReplaceRange { range: std::ops::Range<GraphemeIndex>, replace_with: String }
				impl none::Default for ReplaceRange {}
				let mut replace_range = none::None::none();

				let mut change = Change::Other;

				let index = |c| text::index(text, c);
				let index = |span:&Span| index(span.min())..index(span.max());

				let line_text = |line| line_ranges(text).nth(line).unwrap();
				let line_count = line_ranges(text).count();

				let LineColumn{line, column} = selection.end;
				let prev = || {
					if column > 0 { LineColumn{line, column: if *ctrl { prev_word(&line_text(line), column) } else { column-1 } } }
					else if line > 0 { LineColumn{line: line-1, column: line_text(line-1).len()} }
					else { LineColumn{line, column} }
				};
				let next = || {
					if column < line_text(line).len() { LineColumn{line, column: if *ctrl { next_word(&line_text(line), column) } else { column+1 }} }
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
					'⇤' => LineColumn{line: if *ctrl {0} else {line}, column: 0},
					'⇥' => {
						if *ctrl {LineColumn{line: line_count-1, column: line_text(line_count-1).len()}}
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
					'c' if *ctrl && selection.start != selection.end => {
						*CLIPBOARD.lock().unwrap() = text[index(selection)].to_owned();
						return Change::None;
					}
					'x' if *ctrl && selection.start != selection.end => {
						change = Change::Remove;
						*CLIPBOARD.lock().unwrap() = text[index(selection)].to_owned();
						replace_range = ReplaceRange{range: index(selection), replace_with: nothing()};
						selection.min()
					}
					'v' if *ctrl => {
						let clipboard = CLIPBOARD.lock().unwrap();
						let line_count = line_ranges(&clipboard).count();
						let column = if line_count == 1 { selection.min().column+clipboard.len() } else { line_ranges(&clipboard).nth(line_count-1).unwrap().len() };
						replace_range = ReplaceRange{range: index(selection), replace_with: clipboard.clone()};
						LineColumn{line: selection.min().line+line_count-1, column} // after deletion+insertion
					}
					key if (!key.is_control() || key=='\t') && !ctrl => {
						let sym = KEYMAP.iter().find(|(from,_)| *from == key).map(|(_,sym)| *sym).unwrap_or((key, key.to_uppercase().single().unwrap()));
						let char = if *shift { sym.1 } else { sym.0 };
						//println!("{:?} {:?} {:?}", key, sym, char);
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
						use ::unicode_segmentation::UnicodeSegmentation;
						fn clamp(text: &str, LineColumn{line, column}: &mut LineColumn) { *column = min(*column, line_ranges(text).nth(*line).unwrap().graphemes(true).count()) }
						clamp(text, &mut selection.start);
						clamp(text, &mut selection.end);
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
					view.size = None;
					view.data.get_mut().text.replace_range(range, &replace_with);
					*selection = Span::new(end);
					change
				} else {
					let next = Span{start: if *shift { selection.start } else { end }, end};
					if next == *selection { Change::None } else { *selection = next; Change::Cursor }
				}
			})(),
			&Event::Motion{position, mouse_buttons} => {
				if let Some(pointer) = pointer { let _ = pointer.set_cursor("text", None); }
				if mouse_buttons != 0 {
					let end = view.cursor(size, offset+position);
					let next = Span{end, ..*selection};
					if next == *selection { Change::None } else { *selection = next; Change::Cursor }
				} else { Change::None }
			},
			&Event::Button{button: 0, position, state: Pressed} => {
				let end = view.cursor(size, offset+position);
				let next = Span{start: if *shift { selection.start } else { end }, end};
				if next == *selection { Change::None } else { *selection = next; Change::Cursor }
			},
			_ => { Change::None },
		};
		if change != Change::None { *last_change = change; }
		change
	}
}

impl Widget for Edit<'_,'_> {
	fn size(&mut self, size : size) -> size { Widget::size(&mut self.view, size) }
	#[throws] fn paint(&mut self, target : &mut Target) {
		let Self{view, selection, ..} = self;
		let scale = view.paint_fit(target, zero());
		view.paint_span(target, scale, zero(), *selection, image::bgr{b: true, g: true, r: true});
	}
	#[throws] fn event(&mut self, size: size, event_context: &EventContext, event: &Event) -> bool { if self.event(size, zero(), event_context, event) != Change::None { true } else { false } }
}

#[derive(derive_more::Deref)] pub struct Scroll<'f,'t> { #[deref] pub edit: Edit<'f, 't>, pub offset: uint2 }
impl Scroll<'f,'t> {
	pub fn new(edit: Edit<'f,'t>) -> Self { Self{edit, offset: zero()} }
	pub fn paint_fit(&mut self, target : &mut Target) -> Ratio { let Self{edit: Edit{view, ..}, offset} = self; view.paint_fit(target, *offset) }
	pub fn event(&mut self, size: size, event_context: &EventContext, event: &Event) -> Change {
		let Self{edit, offset} = self;
		let (scroll_size, scale) = edit.view.size_scale(size);
		let change = edit.event(size, scale**offset, event_context, event);
		if change != Change::None {
			let Edit{view, selection, ..} = edit;
			let Rect{min,max} = view.span(selection.min(), selection.max());
			let (min, max) = (min.y as u32, max.y as u32);
			offset.y = offset.y.min(min);
			offset.y = offset.y.max(0.max(max as i32 - (size.y/scale) as i32) as u32);
		}
		if let &Event::Scroll(value) = event { if scroll_size.y > size.y/scale {
			offset.y = min(max(0, offset.y as i32+(value*16./scale) as i32) as u32, scroll_size.y - size.y/scale);
			return Change::Scroll;
		}}
		change
	}
}
impl Widget for Scroll<'_,'_> {
	fn size(&mut self, size : size) -> size { self.edit.size(size) }
	#[throws] fn paint(&mut self, target : &mut Target) {
		let scale = self.paint_fit(target);
		let Scroll{edit: Edit{view, selection, ..}, offset} = self;
		view.paint_span(target, scale, *offset, *selection, image::bgr{b: true, g: true, r: true});
	}
	#[throws] fn event(&mut self, size: size, event_context: &EventContext, event: &Event) -> bool { if self.event(size, event_context, event) != Change::None { true } else { false } }
}
