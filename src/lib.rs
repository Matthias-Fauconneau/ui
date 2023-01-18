#![feature(async_closure,once_cell,type_alias_impl_trait,array_methods,unix_socket_ancillary_data,stmt_expr_attributes,closure_track_caller,let_chains,const_trait_impl,const_convert, array_windows)]
pub type Error = Box<dyn std::error::Error>;
pub type Result<T=(),E=Error> = std::result::Result<T, E>;
pub(crate) mod prelude { pub use {fehler::throws, super::{Error, Result}}; }

pub mod color;
pub use num::Ratio;
pub mod widget; pub use widget::{Widget, Target, size, int2};
mod app; pub use app::{Device, run};
#[cfg(feature="font")] pub mod font;
#[cfg(feature="text")] pub mod text; #[cfg(feature="text")] pub use text::Text; //cfg_if! { if #[cfg(feature="text")] { pub mod text; pub use text::{Text, default_font, default_style}; }}
#[cfg(feature="graphic")] pub mod graphic; #[cfg(feature="graphic")] pub use graphic::Graphic;
#[cfg(feature="edit")] pub mod edit; //cfg_if! { if #[cfg(feature="text")] { pub mod edit; pub use edit::TextEdit; }}
#[cfg(feature="plot")] pub mod plot; #[cfg(feature="plot")] pub use plot::Plot; //cfg_if! { if #[cfg(feature="graphic")] { pub mod graphic; pub use graphic::{Glyph, Graphic}; }}

pub fn time<T>(id: &str, task: impl FnOnce() -> T) -> T {
	let time = std::time::Instant::now();
	let result = task();
	eprintln!("{:?}: {:?}", id, time.elapsed());
	result
}
#[macro_export] macro_rules! time { ($arg:expr) => { $crate::time(stringify!($arg), || $arg) } }
