fn main() -> Result<(), impl std::fmt::Debug> { ui::app::run(ui::text::View::new(ui::text::Plain(std::str::from_utf8(&std::fs::read("examples/text.rs")?)?))) }
