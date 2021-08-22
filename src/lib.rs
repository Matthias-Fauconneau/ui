#![cfg_attr(feature="app", feature(async_closure, box_syntax))]
#![cfg_attr(feature="text", feature(derive_default_enum, once_cell))]
#![cfg_attr(feature="plot", feature(bool_to_option))]

#[cfg(feature="color")] pub mod color;
#[cfg(feature="widget")] pub mod widget; //cfg_if! { if #[cfg(feature="widget")] { pub mod widget; pub use widget::{Target, Widget}; }}
#[cfg(feature="app")] mod as_raw_poll_fd;
#[cfg(feature="app")] mod input;
#[cfg(feature="app")] pub mod app;
#[cfg(feature="font")] pub mod font;
#[cfg(feature="text")] pub mod text; //cfg_if! { if #[cfg(feature="text")] { pub mod text; pub use text::{Text, default_font, default_style}; }}
#[cfg(feature="text")] pub mod edit; //cfg_if! { if #[cfg(feature="text")] { pub mod edit; pub use edit::TextEdit; }}
#[cfg(feature="graphic")] pub mod graphic; //cfg_if! { if #[cfg(feature="graphic")] { pub mod graphic; pub use graphic::{Glyph, Graphic}; }}
#[cfg(feature="plot")] pub mod plot; //cfg_if! { if #[cfg(feature="graphic")] { pub mod graphic; pub use graphic::{Glyph, Graphic}; }}

pub fn time<T>(id: &str, task: impl FnOnce() -> T) -> T {
	let time = std::time::Instant::now();
	let result = task();
	eprintln!("{:?}: {:?}", id, time.elapsed());
	result
}
#[macro_export] macro_rules! time { ($arg:expr) => { $crate::time(stringify!($arg), || $arg) } }
