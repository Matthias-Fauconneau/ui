#![feature(try_trait,nll)]
#![cfg_attr(feature="vector", feature(cmp_min_max_by))]
#![cfg_attr(feature="type_ascription", feature(type_ascription))]
#![cfg_attr(feature="const_fn", feature(const_fn))]
#![cfg_attr(feature="array", allow(incomplete_features),feature(const_generics,maybe_uninit_extra,maybe_uninit_uninit_array))]
#![cfg_attr(feature="lazy_static", feature(maybe_uninit_ref))]
#![cfg_attr(feature="process", feature(termination_trait_lib,try_blocks))]
#![cfg_attr(feature="thread", feature(thread_spawn_unchecked))]
#![cfg_attr(all(feature="thread",feature="image"), feature(slice_index_methods))]
#![cfg_attr(feature="text", feature(type_alias_impl_trait,associated_type_bounds))]
#![cfg_attr(feature="window", feature(generic_associated_types))]
#![cfg_attr(feature="text", feature(box_syntax))] #[cfg(feature="text")] #[macro_use] extern crate rental;

pub mod core; pub use crate::core::{Zero, Error, Result, Ok, TryExtend, sqrt};
#[cfg(feature="fehler")] pub use fehler::throws;
#[cfg(feature="process")] pub mod process;
#[cfg(feature="vector")] pub mod vector; #[cfg(feature="vector")] pub use vector::{uint2, size2, vec2, min, max, sq};
#[cfg(feature="image")] pub mod image; #[cfg(feature="image")] pub use image::{Image, bgra8};
#[cfg(feature="color")] pub mod color; //pub use color::*;
#[cfg(feature="window")] pub mod window; #[cfg(feature="window")] pub use window::window;
#[cfg(feature="text")] pub mod text; #[cfg(feature="text")] pub use text::Text;
