struct Empty; impl ui::Widget for Empty { fn paint(&mut self, _: &mut ui::RenderContext, _: ui::size) -> ui::Result<()> { Ok(()) } }
fn main() -> Result<(), impl std::fmt::Debug> { ui::run(Empty) }
