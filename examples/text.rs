use framework::*;
fn main() -> Result { window(&mut Text::new(std::str::from_utf8(&std::fs::read("examples/text.rs")?)?)) }
