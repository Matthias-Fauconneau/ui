#![feature(async_closure,let_else,box_syntax,once_cell,type_alias_impl_trait,crate_visibility_modifier,try_blocks)]
#![cfg_attr(feature="text", feature(derive_default_enum))]
pub type Error = Box<dyn std::error::Error>;
pub type Result<T=(),E=Error> = std::result::Result<T, E>;

pub mod color;
pub use num::Ratio;
pub mod widget; pub use widget::{Widget, RenderContext, size};
mod as_raw_poll_fd;
mod app; pub use app::run;
#[cfg(feature="font")] pub mod font;
#[cfg(feature="text")] pub mod text; //cfg_if! { if #[cfg(fecd ature="text")] { pub mod text; pub use text::{Text, default_font, default_style}; }}
#[cfg(feature="text")] pub mod graphic; #[cfg(feature="text")] pub use graphic::Graphic; 
#[cfg(feature="edit")] pub mod edit; //cfg_if! { if #[cfg(feature="text")] { pub mod edit; pub use edit::TextEdit; }}
#[cfg(feature="plot")] pub mod plot; //cfg_if! { if #[cfg(feature="graphic")] { pub mod graphic; pub use graphic::{Glyph, Graphic}; }}

pub fn time<T>(id: &str, task: impl FnOnce() -> T) -> T {
	let time = std::time::Instant::now();
	let result = task();
	eprintln!("{:?}: {:?}", id, time.elapsed());
	result
}
#[macro_export] macro_rules! time { ($arg:expr) => { $crate::time(stringify!($arg), || $arg) } }
