#![cfg_attr(feature="lazy_static", feature(maybe_uninit_extra,maybe_uninit_ref))]
#![cfg_attr(feature="const_generics", allow(incomplete_features),feature(const_generics))]
#![cfg_attr(feature="compose", feature(unboxed_closures,fn_traits))]

pub mod core; pub use crate::core::*;
mod vector; pub use vector::*;
#[cfg(feature="compose")] pub mod compose;
mod image; pub use image::Image;
#[cfg(feature="window")] pub mod window;
#[cfg(feature="text")] pub mod text;
