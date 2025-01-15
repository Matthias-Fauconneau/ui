//#![feature(coroutines, iter_from_coroutine)]
pub fn default<T: Default>() -> T { Default::default() }
pub type Error = Box<dyn core::error::Error>;
pub type Result<T=(),E=Error> = core::result::Result<T, E>;
pub use vector::{xy, uint2, int2};
pub use image::{self, Image};
pub mod vulkan;
pub mod widget; pub use widget::{Widget, Event, EventContext, ModifiersState};
mod app; pub use app::run;

#[cfg(feature="text")] pub mod font;
#[cfg(feature="text")] pub mod text; #[cfg(feature="text")] pub use text::{fit,Text,text};
#[cfg(feature="graphic")] pub mod line; #[cfg(feature="graphic")] pub use line::{line, parallelogram};
#[cfg(feature="graphic")] pub mod graphic; #[cfg(feature="graphic")] pub use {graphic::Graphic, vector::num::{Ratio,unit}};
