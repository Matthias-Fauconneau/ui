#![cfg_attr(feature="array", allow(incomplete_features),feature(const_generics,maybe_uninit_extra,maybe_uninit_uninit_array))]
#![cfg_attr(feature="timeout", feature(thread_spawn_unchecked,duration_zero))]
#![cfg_attr(feature="vector", feature(iterator_fold_self))]
#![cfg_attr(feature="window", feature(async_closure))]
//#![cfg_attr(feature="font", feature(associated_type_bounds))]
//#![cfg_attr(feature="font", feature(non_ascii_idents))]
//#![cfg_attr(feature="font", allow(uncommon_codepoints))]
#![cfg_attr(feature="text", feature(once_cell,in_band_lifetimes))]

//#[macro_export] macro_rules! dbg { ( $first:expr $(,$A:expr)* ) => ( eprint!("{} = {:?}", stringify!($first), $first); $( eprint!(", {} = {:?}", stringify!($A), $A); )* eprintln!(""); ) }
pub mod error; pub use error::{Error, Result/*bail, ensure, Ok*/}; #[cfg(feature="fehler")] pub use error::throws;
pub use cfg_if::cfg_if;
pub mod iter;
mod slice;
#[cfg(feature="array")] pub mod array; //pub use array::{Iterator, map};
pub mod num; pub use num::{Zero, Ratio, abs};
cfg_if! { if #[cfg(feature="vector")] { #[macro_use] pub mod vector; pub use vector::{Bounds, MinMax, xy, int2, uint2, size, vec2}; }}
pub fn time<T>(task: impl FnOnce() -> T) -> T {
	let start = std::time::Instant::now();
	let result = task();
	eprintln!("{}", start.elapsed().as_millis());
	result
}
cfg_if! { if #[cfg(feature="trace")] { mod trace; pub use trace::rstack_self; }}
cfg_if! { if #[cfg(feature="timeout")] { mod timeout; pub use timeout::timeout; }}
#[cfg(feature="signal-hook")] pub use trace::sigint_trace;
cfg_if! { if #[cfg(feature="image")] { pub mod image; pub use image::{Image, bgra8}; }}
cfg_if! { if #[cfg(feature="sRGB")] { pub use image::sRGB; }}
#[cfg(feature="color")] pub mod color;
cfg_if! { if #[cfg(feature="widget")] { pub mod widget; pub use widget::{Target, Widget}; }}
cfg_if! { if #[cfg(feature="window")] { pub mod window; }}
cfg_if! { if #[cfg(feature="font")] {
	mod quad;
	mod cubic;
	mod raster;
	pub mod font;
}}
cfg_if! { if #[cfg(feature="text")] { pub mod text; pub use text::{Text, default_font, default_style}; }}
cfg_if! { if #[cfg(feature="text-edit")] { pub mod edit; pub use edit::TextEdit; }}
cfg_if! { if #[cfg(feature="graphic")] { pub mod graphic; pub use graphic::{Rect, Glyph, Graphic}; }}
