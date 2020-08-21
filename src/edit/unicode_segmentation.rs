use std::cmp::min;
use unicode_segmentation::GraphemeCursor;
pub fn prev_boundary(text: &str, from: usize) -> usize { GraphemeCursor::new(min(from, text.len()), text.len(), true).prev_boundary(&text, 0).unwrap().unwrap() }
pub fn next_boundary(text: &str, from: usize) -> usize { GraphemeCursor::new(min(from, text.len()), text.len(), true).next_boundary(&text, 0).unwrap().unwrap() }

#[derive(PartialEq,Clone,Copy)] enum Class { Space, Alphanumeric, Symbol }
fn classify(g:&str) -> Class {
	use {iter::Single, Class::*};
	g.chars().single().map_or(Symbol, |c| if c.is_whitespace() {Space} else if c.is_alphanumeric() {Alphanumeric} else {Symbol} )
}
fn run_class<'t>(graphemes: &mut std::iter::Peekable<impl Iterator<Item=(usize, &'t str)>>, class: Class) -> Option<usize> {
	use iter::PeekableExt;
	graphemes.peeking_take_while(|&(_,g)| classify(g) == class).last().map(|(last,_)| last)
}
use fehler::throws;
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

pub fn prev_word(text: &str, from: usize) -> usize { last_word_start(&text[..from]).unwrap_or(from) }
pub fn next_word(text: &str, from: usize) -> usize { from + next_word_start(&text[from..]).unwrap_or(from) }
