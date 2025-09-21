#![allow(incomplete_features)]#![feature(inherent_associated_types)] // shader uniforms
//#![feature(coroutines, iter_from_coroutine)] // line
//pub fn default<T: Default>() -> T { Default::default() }
pub type Error = Box<dyn core::error::Error>;
pub type Result<T=(),E=Error> = core::result::Result<T, E>;

pub use fehler::throws;
pub use vector::{self, xy, size, int2};
pub use image::{self, Image};
pub mod vulkan;
pub mod widget; pub use widget::{Widget, Event, EventContext, ModifiersState};
mod app; pub use app::{new_trigger, run, trigger};

#[cfg(feature="text")] pub mod font;

pub use image::{bgr, bgrf};
#[allow(non_upper_case_globals)] pub const black : bgrf = bgrf{b: 0., g: 0., r: 0.};
#[allow(non_upper_case_globals)] pub const white : bgrf = bgrf{b: 1., g: 1., r: 1.};
#[allow(non_upper_case_globals)] pub fn background() -> bgrf { white }
#[allow(non_upper_case_globals)] pub fn foreground() -> bgrf { black }

#[cfg(feature="text")] pub mod text; #[cfg(feature="text")] pub use text::{fit,Text,text};
#[cfg(feature="edit")] pub mod edit; #[cfg(feature="edit")] pub use edit::Edit;
#[cfg(feature="graphic")] pub mod line; #[cfg(feature="graphic")] pub use line::{line, parallelogram};
#[cfg(feature="graphic")] pub mod graphic; #[cfg(feature="graphic")] pub use {graphic::Graphic, vector::num::{Ratio,unit}};

pub fn time<T>(id: &str, task: impl FnOnce() -> T) -> T {
	let start = std::time::Instant::now();
	let result = task();
	let time = start.elapsed();
	if time >= std::time::Duration::/*SECOND*/from_secs(1) { eprintln!("{id}: {time:?}")  };
	result
}
#[macro_export] macro_rules! time { ($arg:expr) => { $crate::time(stringify!($arg), || $arg) } }
