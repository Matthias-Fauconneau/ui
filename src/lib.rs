#![cfg_attr(feature="app", feature(async_closure,box_syntax))]
#![cfg_attr(any(feature="app",feature="text"), feature(once_cell))]
//#![cfg_attr(feature="text", feature(in_band_lifetimes))]
//#![cfg_attr(feature="font", feature(non_ascii_idents))]//#![cfg_attr(feature="font", allow(uncommon_codepoints))]

//pub use image::{Image, bgra8};
//#[cfg(feature="core/array")] pub use image::sRGB;
#[cfg(feature="color")] pub mod color;
#[cfg(feature="widget")] pub mod widget; //cfg_if! { if #[cfg(feature="widget")] { pub mod widget; pub use widget::{Target, Widget}; }}
#[cfg(feature="app")] pub mod app;
#[cfg(feature="font")] pub mod font;
#[cfg(feature="text")] pub mod text; //cfg_if! { if #[cfg(feature="text")] { pub mod text; pub use text::{Text, default_font, default_style}; }}
#[cfg(feature="edit")] pub mod edit; //cfg_if! { if #[cfg(feature="text-edit")] { pub mod edit; pub use edit::TextEdit; }}
#[cfg(feature="graphic")] pub mod graphic; //cfg_if! { if #[cfg(feature="graphic")] { pub mod graphic; pub use graphic::{Glyph, Graphic}; }}
