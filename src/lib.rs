#![cfg_attr(feature="lazy_cell", feature(lazy_cell))]
#![cfg_attr(feature="int_roundings", feature(int_roundings))]
#![cfg_attr(feature="array_methods", feature(array_methods))]
//#![feature(async_closure, lazy_cell, type_alias_impl_trait, array_methods, stmt_expr_attributes, closure_track_caller, const_trait_impl, array_windows, 
//						int_roundings, generic_arg_infer, generators, iter_from_generator, default_free_fn, div_duration/*, const_convert*/)]
//#![cfg_attr(feature="wayland", feature(unix_socket_ancillary_data))]
pub type Error = Box<dyn std::error::Error>;
pub type Result<T=(),E=Error> = std::result::Result<T, E>;
pub use fehler::throws;
pub mod prelude { pub use super::{Result,Error,throws, size,int2, Target, Widget, App, run}; }

//mod line; pub use line::{line, parallelogram};
pub mod color; pub use color::{bgrf,black,white,dark,background,foreground};
pub mod widget; pub use widget::{xy,size,int2, Widget, Target, Event, EventContext, ModifiersState};
mod app; pub use app::{App, run};
#[cfg(feature="font")] pub mod font;
#[cfg(feature="text")] pub mod text; #[cfg(feature="text")] pub use text::{fit,Text,text};
#[cfg(feature="graphic")] pub mod graphic; #[cfg(feature="graphic")] pub use {graphic::Graphic, num::{Ratio,unit}};
#[cfg(feature="edit")] pub mod edit; //pub use edit::TextEdit;
#[cfg(feature="plot")] pub mod plot; #[cfg(feature="plot")] pub use plot::{list, Plot};

pub fn time<T>(id: &str, task: impl FnOnce() -> T) -> T {
	let time = std::time::Instant::now();
	let result = task();
	eprintln!("{:?}: {:?}", id, time.elapsed());
	result
}
#[macro_export] macro_rules! time { ($arg:expr) => { $crate::time(stringify!($arg), || $arg) } }
