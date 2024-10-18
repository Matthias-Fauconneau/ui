pub use text_size::TextSize;
pub type GraphemeIndex = usize;
pub use unicode_segmentation::UnicodeSegmentation;
pub fn index(s: &str, grapheme_index: GraphemeIndex) -> usize/*TextSize*/ {
	match (grapheme_index as usize) .cmp( &s.grapheme_indices(true).count() ) {
		std::cmp::Ordering::Less => s.grapheme_indices(true).nth(grapheme_index as usize).unwrap().0,
		std::cmp::Ordering::Equal => s.len(),
		_ => panic!()
	}//.try_into().unwrap()
}
#[track_caller] pub fn find(s: &str, byte_index: TextSize) -> GraphemeIndex {
	let byte_index = byte_index.into();
	//assert!(byte_index<=s.len());
	let mut grapheme_index = 0;
	for (grapheme_byte_index,_) in s.grapheme_indices(true) {
		if grapheme_byte_index == byte_index { return grapheme_index; }
		grapheme_index += 1;
	}
	//assert_eq!(byte_index, s.len()); // byte_index>s.len() returns last grapheme index
	grapheme_index
}

#[derive(PartialEq,Clone,Copy)] enum Class { Space, Alphanumeric, Symbol }
fn classify(g: &str) -> Class {
	use {super::iter::Single, Class::*};
	g.chars().single().map_or(Symbol, |c| if c.is_whitespace() {Space} else if c.is_alphanumeric() {Alphanumeric} else {Symbol} )
}
fn run_class<'t>(graphemes: &mut std::iter::Peekable<impl Iterator<Item=(GraphemeIndex, &'t str)>>, class: Class) -> Option<GraphemeIndex> {
	use super::iter::PeekableExt;
	graphemes.peeking_take_while(|&(_,g)| classify(g) == class).last().map(|(last,_)| last)
}

fn run<'t>(graphemes: &mut std::iter::Peekable<impl Iterator<Item=(GraphemeIndex, &'t str)>>) -> Option<(GraphemeIndex, Class)> {
	let (last, class) = { let (last, g) = graphemes.next()?; (last, classify(g)) };
	Some((run_class(graphemes, class).unwrap_or(last), class))
}

fn last_word_start(text: &str) -> Option<GraphemeIndex> {
	let mut graphemes = text.graphemes(true).rev().scan(text.grapheme_indices(true).count() as GraphemeIndex, |before, g| { *before -= 1; Some((*before, g)) }).peekable();
	Some(match run(&mut graphemes)? {
		(_, Class::Space) => { let (last, _) = run(&mut graphemes)?; last },
		(last, _) => last,
	})
}
fn next_word_start(text: &str) -> Option<GraphemeIndex> {
	let mut graphemes = text.graphemes(true).enumerate().map(|(i,e)| (i as GraphemeIndex, e)).peekable();
	Some(match run(&mut graphemes)? {
		(last, Class::Space) => last+1,
		(last, _) => { run_class(&mut graphemes, Class::Space).unwrap_or(last)+1 },
	})
}

pub fn prev_word(text: &str, from: GraphemeIndex) -> GraphemeIndex { last_word_start(&text[..index(text, from).into()]).unwrap_or(from) }
pub fn next_word(text: &str, from: GraphemeIndex) -> GraphemeIndex { from + next_word_start(&text[index(text, from).into()..]).unwrap_or(from) }
