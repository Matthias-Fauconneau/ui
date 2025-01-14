#![feature(coroutines, iter_from_coroutine)]
pub type Error = Box<dyn core::error::Error>;
pub type Result<T=(),E=Error> = core::result::Result<T, E>;
pub use vector::{xy, uint2, int2};
pub use image::{bgr, bgrf};
#[allow(non_upper_case_globals)] pub const black : bgrf = bgrf{b: 0., g: 0., r: 0.};
#[allow(non_upper_case_globals)] pub const white : bgrf = bgrf{b: 1., g: 1., r: 1.};
#[allow(non_upper_case_globals)] pub fn background() -> bgrf { white }
#[allow(non_upper_case_globals)] pub fn foreground() -> bgrf { black }

pub mod widget; pub use widget::{Widget, Target, Event, EventContext, ModifiersState};
mod app; pub use app::run;

#[cfg(feature="text")] pub mod font;
#[cfg(feature="text")] pub mod text; #[cfg(feature="text")] pub use text::{fit,Text,text};
#[cfg(feature="graphic")] pub mod line; #[cfg(feature="graphic")] pub use line::{line, parallelogram};
#[cfg(feature="graphic")] pub mod graphic; #[cfg(feature="graphic")] pub use {graphic::Graphic, vector::num::{Ratio,unit}};
