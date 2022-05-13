use {fehler::throws, super::Error, std::cmp::{min,max}, num::{IsZero, zero, Ratio}, text::iter::Single, vector::{uint2, int2, Rect}};
use crate::{text::{self, unicode_segmentation::{self, GraphemeIndex, prev_word, next_word},
														LineColumn, Span, Attribute, Style, line_ranges, Font, View, Buffer, Borrowed},
									 widget::{Event, EventContext, Widget, size, RenderContext, ModifiersState, ButtonState::Pressed}};

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
impl AsRef<str> for Cow<'_> { fn as_ref(&self) -> &str { match self { Cow::Borrowed(b) => b.as_ref(), Cow::Owned(o) => o.as_ref()} } }
impl AsRef<[Attribute<Style>]> for Cow<'_> { fn as_ref(&self) -> &[Attribute<Style>] { match self { Cow::Borrowed(b) => b.as_ref(), Cow::Owned(o) => o.as_ref()} } }
//impl<'t> Cow<'t> { pub fn new(text: &'t str) -> Self { Cow::Borrowed(Borrowed{text, style: &default_style}) } }

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
pub fn new(font: Font<'f>, data: Cow<'t>) -> Self {
	Self{view: View{font, data, size: None}, selection: Span::new(LineColumn{line: 0, column: 0}), history: Vec::new(), history_index: 0, last_change: Change::Other, compose: None}
}
pub fn event(&mut self, size : size, offset: uint2, EventContext{modifiers_state, cursor}: &mut EventContext, event: &Event) -> Change {
	let ModifiersState{ctrl,shift,alt,..} = *modifiers_state;
	let Self{ref mut view, selection, history, history_index, last_change, compose, ..} = self;
	let change = match event {
			&Event::Key(key) => (||{
				let View{data, ..} = view;
				match key {
					'⎋' => { *compose = None; if selection.start != selection.end && !shift { *selection=Span::new(selection.end); return Change::Cursor; } else { return Change::None; } }
					'⇧'|'⇪'|'⌃'|'⌘'|'⎇'|'⎀'|'⎙' => return Change::None,
					'←'| '→' if alt => return Change::None,
					'z' if ctrl => {
						if !shift && *history_index > 0 {
							if *history_index == history.len() { history.push(State{text: std::mem::replace(&mut data.get_mut().text, String::new()), cursor: selection.end}); }
							*history_index -= 1;
						}
						else if shift && *history_index+1 < history.len() { *history_index += 1; }
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
				let mut replace_range = None;

				let mut change = Change::Other;

				let index = |c| text::index(text, c);
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
						replace_range = Some(ReplaceRange{range: index(selection), replace_with: '\n'.to_string()});
						// todo: indentation
						LineColumn{line: line+1, column: 0}
					}
					'⌫' => {
						change = Change::Remove;
						let mut selection = *selection;
						if selection.start == selection.end { selection.end = prev(); }
						replace_range = Some(ReplaceRange{range: index(&selection), replace_with: nothing()});
						selection.min()
					}
					'⌦' => {
						change = Change::Remove;
						let mut selection = *selection;
						if selection.start == selection.end { selection.end = next(); }
						replace_range = Some(ReplaceRange{range: index(&selection), replace_with: nothing()});
						selection.min() // after deletion
					}
					'c' if ctrl && selection.start != selection.end => {
						*CLIPBOARD.lock().unwrap() = text[index(selection)].to_owned();
						return Change::None;
					}
					'x' if ctrl && selection.start != selection.end => {
						change = Change::Remove;
						*CLIPBOARD.lock().unwrap() = text[index(selection)].to_owned();
						replace_range = Some(ReplaceRange{range: index(selection), replace_with: nothing()});
						selection.min()
					}
					'v' if ctrl => {
						let clipboard = CLIPBOARD.lock().unwrap();
						let line_count = line_ranges(&clipboard).count();
						let column = if line_count == 1 { selection.min().column+clipboard.len() } else { line_ranges(&clipboard).nth(line_count-1).unwrap().len() };
						replace_range = Some(ReplaceRange{range: index(selection), replace_with: clipboard.clone()});
						LineColumn{line: selection.min().line+line_count-1, column} // after deletion+insertion
					}
					key if (!key.is_control() || key=='\t') && !ctrl => {
						let sym = KEYMAP.iter().find(|(from,_)| *from == key).map(|(_,sym)| *sym).unwrap_or((key, key.to_uppercase().single().unwrap()));
						let char = if shift { sym.1 } else { sym.0 };
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
						//impl LineColumn { fn clamp(&self, text: &str) -> Self { Self{line: self.line, column: min(self.column, line_ranges(text).nth(self.line).unwrap().graphemes(true).count()) } } }
						//selection.start = selection.start.clamp(text);
						//selection.end = selection.start.clamp(text);
						//fn clamp(LineColumn{line, column}: LineColumn, text: &str) -> LineColumn { LineColumn{line, column: min(column, line_ranges(text).nth(line).unwrap().graphemes(true).count()) } }
						fn clamp(s: LineColumn, text: &str) -> LineColumn { LineColumn{line: s.line, column: min(s.column, line_ranges(text).nth(s.line).unwrap().graphemes(true).count()) } }
						selection.start = clamp(selection.start, text);
						selection.end = clamp(selection.start, text);
						replace_range = Some(ReplaceRange{range: index(selection), replace_with: char.to_string()});
						LineColumn{line: selection.min().line, column: selection.min().column+1} // after insertion
					}
					key => { println!("{:?}", key); selection.end },
				};
				if let Some(ReplaceRange{range, replace_with}) = replace_range {
					history.truncate(*history_index);
					if !((change==Change::Insert || change==Change::Remove) && change == *last_change) { history.push(State{text: text.to_owned(), cursor: selection.end}); }
					*history_index = history.len();
					let range = unicode_segmentation::index(text, range.start)..unicode_segmentation::index(text, range.end);
					view.size = None;
					view.data.get_mut().text.replace_range(range, &replace_with);
					*selection = Span::new(end);
					change
				} else {
					let next = Span{start: if shift { selection.start } else { end }, end};
					if next == *selection { Change::None } else { *selection = next; Change::Cursor }
				}
			})(),
			&Event::Motion{position, mouse_buttons} => {
				if let Some(cursor) = cursor { let _ = cursor.set("text"); }
				if mouse_buttons != 0 {
					let end = view.cursor(size, offset+uint2::from(position));
					let next = Span{end, ..*selection};
					if next == *selection { Change::None } else { *selection = next; Change::Cursor }
				} else { Change::None }
			},
			&Event::Button{button: 0, position, state: Pressed} => {
				let end = view.cursor(size, offset+uint2::from(position));
				let next = Span{start: if shift { selection.start } else { end }, end};
				if next == *selection { Change::None } else { *selection = next; Change::Cursor }
			},
			_ => { Change::None },
		};
		if change != Change::None { *last_change = change; }
		change
	}
}

impl Widget for Edit<'_,'_> {
	fn size(&mut self, size : size) -> size {
		let size = Widget::size(&mut self.view, size);
		if !size.is_zero() { size } else { (self.view.font[0].height() as u32).into() }
	}
	#[throws] fn paint(&mut self, cx: &mut RenderContext, size: size, offset: int2) {
		let Self{view, selection, ..} = self;
		let scale = view.paint_fit(cx, size, offset);
		view.paint_span(cx, scale, offset, *selection, image::bgr{b: true, g: true, r: true});
	}
	#[throws] fn event(&mut self, size: size, event_context: &mut EventContext, event: &Event) -> bool { if self.event(size, zero(), event_context, event) != Change::None { true } else { false } }
}

#[derive(derive_more::Deref)] pub struct Scroll<'f,'t> { #[deref] pub edit: Edit<'f, 't>, pub offset: uint2 }
impl<'f,'t> Scroll<'f,'t> {
	pub fn new(edit: Edit<'f,'t>) -> Self { Self{edit, offset: zero()} }
	pub fn paint_fit(&mut self, context : &mut RenderContext, size: size, offset: int2) -> Ratio { self.edit.view.paint_fit(context, size, offset-self.offset.signed()) }
	pub fn keep_selection_in_view(&mut self, size: size) {
		let Self{edit: Edit{view, selection, ..}, offset} = self;
		let Rect{min,max} = view.span(selection.min(), selection.max());
		let (min, max) = (min.y as u32, max.y as u32);
		offset.y = offset.y.min(min);
		offset.y = offset.y.max(0.max(max as i32 - (size.y/view.scale(size)) as i32) as u32);
	}
	pub fn event(&mut self, size: size, event_context: &mut EventContext, event: &Event) -> Change {
		let Self{edit, offset} = self;
		let (scroll_size, scale) = edit.view.size_scale(size);
		if let &Event::Scroll(value) = event {
			if scroll_size.y > size.y/scale {
				offset.y = min(max(0, offset.y as i32+(value*16./scale) as i32) as u32, scroll_size.y - size.y/scale);
				Change::Scroll
			} else { Change::None }
		} else {
			let change = edit.event(size, scale**offset, event_context, event);
			if change != Change::None { self.keep_selection_in_view(size); }
			change
		}
	}
}
impl Widget for Scroll<'_,'_> {
	fn size(&mut self, size : size) -> size { self.edit.size(size) }
	#[throws] fn paint(&mut self, cx: &mut RenderContext, size: size, offset: int2) {
		let scale = self.edit.view.paint_fit(cx, size, offset);
		let Scroll{edit: Edit{view, selection, ..}, offset} = self;
		view.paint_span(cx, scale, -offset.signed(), *selection, image::bgr{b: true, g: true, r: true});
	}
	#[throws] fn event(&mut self, size: size, event_context: &mut EventContext, event: &Event) -> bool { if self.event(size, event_context, event) != Change::None { true } else { false } }
}
