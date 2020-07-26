#![cfg_attr(feature="window", feature(async_closure))]
//#![cfg_attr(feature="font", feature(associated_type_bounds))]
//#![cfg_attr(feature="font", feature(non_ascii_idents))]
//#![cfg_attr(feature="font", allow(uncommon_codepoints))]
#![cfg_attr(feature="text", feature(once_cell,in_band_lifetimes))]
#![cfg_attr(feature="text-edit", feature(or_patterns))]

pub use image::{Image, bgra8};
use cfg_if::cfg_if;
#[cfg(feature="core/array")] pub use image::sRGB;
#[cfg(feature="color")] pub mod color;
cfg_if! { if #[cfg(feature="widget")] { pub mod widget; pub use widget::{Target, Widget}; }}
#[cfg(feature="window")] pub mod window;
cfg_if! { if #[cfg(feature="font")] {
	mod quad;
	mod cubic;
	mod raster;
	pub mod font;
}}
cfg_if! { if #[cfg(feature="text")] { pub mod text; pub use text::{Text, default_font, default_style}; }}
cfg_if! { if #[cfg(feature="text-edit")] { pub mod edit; pub use edit::TextEdit; }}
cfg_if! { if #[cfg(feature="graphic")] { pub mod graphic; pub use graphic::{Rect, Glyph, Graphic}; }}
