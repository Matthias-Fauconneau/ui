use ui::{*, text::*, edit::*};
fn main() -> Result { run("edit", &mut Edit::new(default_font(), Cow::new("ffi ff fi"))) }
