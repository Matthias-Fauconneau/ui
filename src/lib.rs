#![cfg_attr(feature="lazy_static", feature(maybe_uninit_extra,maybe_uninit_ref))]
#![cfg_attr(feature="const_generics", allow(incomplete_features),feature(const_generics))]
#![cfg_attr(feature="const_fn", feature(const_fn))]
#![cfg_attr(feature="fn_traits", feature(unboxed_closures,fn_traits))]

pub mod core; //pub use crate::core::*;
pub mod vector; //pub use vector::*;
pub mod image; //pub use image::{Image,bgra8};
#[cfg(feature="window")] pub mod window;
#[cfg(feature="text")] pub mod text;
