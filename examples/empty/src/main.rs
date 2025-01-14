struct Empty; impl ui::Widget for Empty { fn paint(&mut self, target: &mut ui::Target, _: ui::uint2, _: ui::int2) -> ui::Result<()> {
	for x in 0..target.size.x {
		let i = u8::try_from(x*0x100/target.size.x).unwrap();
		let c = u32::from(ui::bgr{b: i, g: i, r: i});
		for y in 0..target.size.y { target[ui::xy{x,y}] = c; }
	}
	Ok(())
} }
fn main() { ui::run("empty", &mut Empty) }
