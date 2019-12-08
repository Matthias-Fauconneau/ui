#![feature(non_ascii_idents)]
mod core; use crate::core::Result; use std::{rc::Rc, cell::RefCell};
mod image;
mod window; use window::run;
mod raster;
mod text; use text::{Font, Text};

fn main() -> Result { run(Rc::new(RefCell::new(Text{font: Rc::new(Font::map()?), text: "Hello World! ⅞".to_string()}))) }
