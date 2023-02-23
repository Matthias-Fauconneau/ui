#![feature(async_closure, once_cell, type_alias_impl_trait, array_methods, unix_socket_ancillary_data, stmt_expr_attributes, closure_track_caller, let_chains, const_trait_impl, const_convert, array_windows, int_roundings, generic_arg_infer, array_zip, generators, iter_from_generator, default_free_fn, is_some_and, div_duration)]
pub type Error = Box<dyn std::error::Error>;
pub type Result<T=(),E=Error> = std::result::Result<T, E>;
pub use fehler::throws;
pub mod prelude { pub use super::{Result,Error,throws, size,int2, Target, Widget, App, run}; }

mod line; pub use line::{line, parallelogram};
pub mod color; pub use color::{bgrf,black,white,dark,background,foreground};
pub mod widget; pub use widget::{xy,size,int2, Widget, Target, Event, EventContext};
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
