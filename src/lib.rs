#![feature(try_trait)]
#![cfg_attr(feature="type_ascription", feature(type_ascription))]
#![cfg_attr(feature="const_fn", feature(const_fn))]
#![cfg_attr(feature="array", allow(incomplete_features),feature(const_generics,maybe_uninit_extra))]
#![cfg_attr(feature="lazy_static", feature(maybe_uninit_ref))]
//#![cfg_attr(feature="fn_traits", feature(unboxed_closures,fn_traits))]
#![cfg_attr(feature="process", feature(termination_trait_lib,try_trait,try_blocks))]
#![cfg_attr(feature="thread", feature(thread_spawn_unchecked))]
#![cfg_attr(all(feature="thread",feature="image"), feature(slice_index_methods))]

pub mod core; pub use crate::core::{Result, Ok, TryExtend};
#[cfg(feature="process")] pub mod process;
#[cfg(feature="vector")] pub mod vector; pub use vector::int2;
#[cfg(feature="image")] pub mod image; //pub use image::{Image,bgra8};
#[cfg(feature="color")] pub mod color; //pub use color::*;
#[cfg(feature="window")] pub mod window;
#[cfg(feature="text")] pub mod text;
