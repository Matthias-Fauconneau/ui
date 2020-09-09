pub type GraphemeIndex = usize;
pub use unicode_segmentation::UnicodeSegmentation;
pub fn index(s: &str, grapheme_index: GraphemeIndex) -> usize {
	match (grapheme_index as usize) .cmp( &s.grapheme_indices(true).count() ) {
		std::cmp::Ordering::Less => s.grapheme_indices(true).nth(grapheme_index as usize).unwrap().0,
		std::cmp::Ordering::Equal => s.len(),
		_ => panic!()
	}
}
#[throws(as Option)] pub fn find(s: &str, byte_index: usize) -> GraphemeIndex { s.grapheme_indices(true).enumerate().find(|&(_,(i,_))| i == byte_index)?.0 }

#[derive(PartialEq,Clone,Copy)] enum Class { Space, Alphanumeric, Symbol }
fn classify(g: &str) -> Class {
	use {iter::Single, Class::*};
	g.chars().single().map_or(Symbol, |c| if c.is_whitespace() {Space} else if c.is_alphanumeric() {Alphanumeric} else {Symbol} )
}
fn run_class<'t>(graphemes: &mut std::iter::Peekable<impl Iterator<Item=(GraphemeIndex, &'t str)>>, class: Class) -> Option<GraphemeIndex> {
	use iter::PeekableExt;
	graphemes.peeking_take_while(|&(_,g)| classify(g) == class).last().map(|(last,_)| last)
}
use fehler::throws;
#[throws(as Option)] fn run<'t>(graphemes: &mut std::iter::Peekable<impl Iterator<Item=(GraphemeIndex, &'t str)>>) -> (GraphemeIndex, Class) {
	let (last, class) = { let (last, g) = graphemes.next()?; (last, classify(g)) };
	(run_class(graphemes, class).unwrap_or(last), class)
}

#[throws(as Option)] fn last_word_start(text: &str) -> GraphemeIndex {
	let mut graphemes = text.graphemes(true).rev().scan(text.grapheme_indices(true).count() as GraphemeIndex, |before, g| { *before -= 1; Some((*before, g)) }).peekable();
	match run(&mut graphemes)? {
		(_, Class::Space) => { let (last, _) = run(&mut graphemes)?; last },
		(last, _) => last,
	}
}
#[throws(as Option)] fn next_word_start(text: &str) -> GraphemeIndex {
	let mut graphemes = text.graphemes(true).enumerate().map(|(i,e)| (i as GraphemeIndex, e)).peekable();
	match run(&mut graphemes)? {
		(last, Class::Space) => last+1,
		(last, _) => { run_class(&mut graphemes, Class::Space).unwrap_or(last)+1 },
	}
}

pub fn prev_word(text: &str, from: GraphemeIndex) -> GraphemeIndex { last_word_start(&text[..index(text, from)]).unwrap_or(from) }
pub fn next_word(text: &str, from: GraphemeIndex) -> GraphemeIndex { from + next_word_start(&text[index(text, from)..]).unwrap_or(from) }
