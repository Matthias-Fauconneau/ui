#![feature(type_ascription)]
#![cfg_attr(feature="lazy_static", feature(maybe_uninit_ref))]
#![cfg_attr(feature="const_generics", allow(incomplete_features),feature(const_generics,maybe_uninit_extra))]
#![cfg_attr(feature="const_fn", feature(const_fn))]
#![cfg_attr(feature="fn_traits", feature(unboxed_closures,fn_traits))]
#![cfg_attr(feature="thread", feature(thread_spawn_unchecked))]
#![cfg_attr(all(feature="thread",feature="image"), feature(slice_index_methods))]

pub mod core; //pub use crate::core::*;
pub mod vector; //pub use vector::*;
pub mod image; //pub use image::{Image,bgra8};
pub mod color; //pub use color::*;
#[cfg(feature="window")] pub mod window;
#[cfg(feature="text")] pub mod text;
