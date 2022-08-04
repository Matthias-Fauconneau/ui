use ui::{*, text::*, edit::*};
fn main() -> Result { run(&mut Edit::new(default_font(), Cow::new("buffer find"))) }
