use {std::{rc::Rc, cell::RefCell}, framework::{core::Result, window::run, text::{Font, Text}}};
fn main() -> Result { run(Rc::new(RefCell::new(Text{font: Rc::new(Font::map()?), text: "Hello World! ⅞".to_string()}))) }
