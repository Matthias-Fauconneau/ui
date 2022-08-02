use {std::fs::read, ui::{*, text::*, edit::*}};
fn main() -> Result { run(&mut Edit::new(default_font(), Cow::Owned(Owned{text: String::from_utf8(read("examples/edit.rs")?)?, style: Vec::new()}))) }
