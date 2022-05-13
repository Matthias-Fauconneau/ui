struct Empty; impl ui::Widget for Empty { fn paint(&mut self, _: &mut ui::RenderContext, _: ui::size, _: ui::int2) -> ui::Result<()> { Ok(()) } }
fn main() -> Result<(), impl std::fmt::Debug> { ui::run(Box::new(Empty)) }
