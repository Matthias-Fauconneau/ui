use framework::*;
fn main() -> Result { window::run(&mut Text::new(std::str::from_utf8(&std::fs::read("examples/text.rs")?)?, &*default_style)) }
