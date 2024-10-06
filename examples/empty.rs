#![no_std]#![no_main]std::no_problem!();
struct Empty; impl ui::Widget for Empty { fn paint(&mut self, _: &mut ui::Target, _: ui::size, _: ui::int2) -> ui::Result<()> { Ok(()) } }
fn main() -> ui::Result { let r = ui::run("empty", &mut Empty); println!("OK"); r }
