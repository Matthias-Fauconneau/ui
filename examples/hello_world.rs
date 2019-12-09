#![allow(incomplete_features)]#![feature(const_generics,maybe_uninit_extra,maybe_uninit_ref,non_ascii_idents,try_trait)]
mod core; use crate::core::{Result, Ok}; use std::{rc::Rc, cell::RefCell};
mod image;
mod window; use window::run;
mod raster;
mod text; use text::{Font, Text};

fn main() -> Result { run(Rc::new(RefCell::new(Text{font: Rc::new(Font::map()?), text: "Hello World! ⅞".to_string()}))) }
