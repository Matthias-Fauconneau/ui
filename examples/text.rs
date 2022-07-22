fn main() -> ui::Result { ui::run(&mut ui::text::View::new(ui::text::Plain(String::from_utf8(std::fs::read("examples/text.rs")?)?))) }
