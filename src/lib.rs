#![feature(trait_alias)]
#![cfg_attr(feature="array", allow(incomplete_features),feature(const_generics,maybe_uninit_extra,maybe_uninit_uninit_array))]
#![cfg_attr(feature="lazy_static", feature(maybe_uninit_ref))]
#![cfg_attr(feature="try_extend", feature(try_trait))]
#![cfg_attr(feature="process", feature(termination_trait_lib,try_trait,try_blocks))]
#![cfg_attr(feature="vector", feature(cmp_min_max_by))]
#![cfg_attr(feature="window", feature(async_closure))]
//#![cfg_attr(feature="thread", feature(thread_spawn_unchecked))]
//#![cfg_attr(all(feature="thread",feature="image"), feature(slice_index_methods))]
#![cfg_attr(feature="font", feature(box_syntax))]
#![cfg_attr(feature="text", feature(type_alias_impl_trait,associated_type_bounds))]

pub mod error; pub use error::{Error, Result, OkOr, Ok};
//pub use num::Zero;
pub use cfg_if::cfg_if;
#[cfg(feature="fehler")] pub use fehler::throws;
//#[cfg(feature="array")] pub use array::map;
#[cfg(feature="lazy_static")] mod lazy_static;
cfg_if! { if #[cfg(feature="try_extend")] { mod try_extend; /*pub use try_extend::TryExtend;*/ } }
pub fn log<T:std::fmt::Debug>(v: T) { println!("{:?}", v); }
#[macro_export] macro_rules! log { ($($A:expr),+) => ( $crate::core::log(($($A),+)) ) }
cfg_if! { if #[cfg(feature="trace_sigint")] { mod trace_sigint; pub use trace_sigint::{rstack_self, signal_hook}; } }
#[cfg(feature="process")] pub mod process;
cfg_if! { if #[cfg(feature="vector")] { pub mod vector; pub use vector::{uint2, size2, vec2, min, max, sq}; } }
cfg_if! { if #[cfg(feature="image")] { pub mod image; pub use image::{Image, bgra8}; } }
#[cfg(feature="color")] pub mod color;
#[cfg(feature="widget")] pub mod widget;
cfg_if! { if #[cfg(feature="window")] { pub mod window; pub use window::window; } }
cfg_if! { if #[cfg(feature="text")] { pub mod text; pub use text::Text; } }
