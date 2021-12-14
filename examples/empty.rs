struct Empty; impl app::Widget for Empty { fn paint(&mut self, _: &mut app::RenderContext) -> app::Result<()> { Ok(()) } }
fn main() -> Result<(), impl std::fmt::Debug> { app::run(Empty) }
