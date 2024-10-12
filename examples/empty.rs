//#[cfg_attr(feature="no-std", no_std)]
//#[cfg_attr(feature="no-std", no_main)]
//#[cfg(feature="no-std")] std::no_problem!();
//struct Empty; impl ui::Widget for Empty { fn paint(&mut self, _: &mut ui::Target, _: ui::size, _: ui::int2) -> ui::Result<()> { Ok(()) } }
//fn main() -> ui::Result { println!("?"); dbg!(); let r = ui::run("empty", &mut Empty); println!("OK"); r }
fn main() { println!("Hello World!"); }