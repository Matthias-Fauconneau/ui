fn main() -> core::error::Result { ui::window::run(&mut ui::text::TextView::new(&ui::text::default_font, ui::text::Buffer::new(std::str::from_utf8(&std::fs::read("examples/text.rs")?)?))) }
