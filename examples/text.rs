fn main() -> ui::Result { ui::run(ui::text::View::new(ui::text::Plain(std::str::from_utf8(&std::fs::read("examples/text.rs")?)?))) }
