//#![cfg_attr(feature="", feature(trait_alias))]
#![cfg_attr(feature="array", allow(incomplete_features),feature(const_generics,maybe_uninit_extra,maybe_uninit_uninit_array))]
//#![cfg_attr(feature="lazy_static", feature(maybe_uninit_ref))]
//#![cfg_attr(feature="try_extend", feature(try_trait))]
//#![cfg_attr(feature="process", feature(try_trait,termination_trait_lib))]
//#![cfg_attr(feature="vector", feature(cmp_min_max_by))]
#![cfg_attr(feature="window", feature(async_closure))]
//#![cfg_attr(feature="thread", feature(thread_spawn_unchecked))]
//#![cfg_attr(all(feature="thread",feature="image"), feature(slice_index_methods))]
#![cfg_attr(feature="font", feature(type_alias_impl_trait,associated_type_bounds))] //box_syntax

pub mod error; pub use error::{/*Error,*/ Result/*, OkOr, Ok*/};
//#[cfg(feature="anyhow")] pub use error::{bail, ensure};
#[cfg(feature="fehler")] pub use error::throws;
pub use cfg_if::cfg_if;
mod iter;
mod slice;
#[cfg(feature="array")] mod array; //pub use array::map;
mod num; //pub use num::Zero;
//cfg_if! { if #[cfg(feature="try_extend")] { mod try_extend; /*pub use try_extend::TryExtend;*/ } }
pub fn log<T:std::fmt::Debug>(v: T) { println!("{:?}", v); }
#[macro_export] macro_rules! log { ($($A:expr),+) => ( $crate::core::log(($($A),+)) ) }
//cfg_if! { if #[cfg(feature="trace_sigint")] { mod trace_sigint; /*pub use trace_sigint::{rstack_self, signal_hook};*/ } }
//#[cfg(feature="process")] pub mod process;
cfg_if! { if #[cfg(feature="vector")] { #[macro_use] pub mod vector; /*pub use vector::{uint2, size2, vec2, sq};*/ } }
cfg_if! { if #[cfg(feature="image")] { pub mod image; /*pub use image::{Image, bgra8};*/ } }
//#[cfg(feature="color")] pub mod color;
cfg_if! { if #[cfg(feature="widget")] { pub mod widget; pub use widget::{Target, Widget}; } }
cfg_if! { if #[cfg(feature="window")] { pub mod window; } }
cfg_if! { if #[cfg(feature="text")] { pub mod text; pub use text::{Text, default_style}; } }
