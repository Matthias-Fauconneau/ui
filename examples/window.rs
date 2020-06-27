use framework::*;
struct Empty;
impl Widget for Empty {
	fn render(&mut self, _: &mut Target) {}
}
fn main() -> Result<()> { window::run(&mut Empty) }
