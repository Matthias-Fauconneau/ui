extern crate ui;
use ui::{*, text::*, edit::*};
fn main() -> Result { run("edit", Box::new(|_,_| Ok(Box::new(Edit::new(default_font(), Cow::new("ffi ff fi")))))) }
