use {std::fs::read, ui::{*, text::*}};
fn main() -> Result { run(&mut View::new(Plain(String::from_utf8(read("examples/text.rs")?)?))) }
