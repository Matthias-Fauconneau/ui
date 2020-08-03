use core::{throws,Error};
struct Empty;
impl ui::widget::Widget for Empty {
	#[throws] fn paint(&mut self, _: &mut ui::widget::Target) {}
}
#[throws] fn main() { ui::app::run(Empty)? }
