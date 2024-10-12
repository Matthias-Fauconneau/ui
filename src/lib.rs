#![feature(coroutines, iter_from_coroutine)]
pub type Error = Box<dyn core::error::Error>;
pub type Result<T=(),E=Error> = core::result::Result<T, E>;
pub use fehler::throws;

pub use image::bgrf;
#[allow(non_upper_case_globals)] pub const black : bgrf = bgrf{b: 0., g: 0., r: 0.};
#[allow(non_upper_case_globals)] pub const white : bgrf = bgrf{b: 1., g: 1., r: 1.};
#[allow(non_upper_case_globals)] pub fn background() -> bgrf { white }
#[allow(non_upper_case_globals)] pub fn foreground() -> bgrf { black }

pub mod widget; pub use widget::{xy,size,int2, Widget, Target, Event, EventContext, ModifiersState};
mod app; pub use app::{App, run};

pub mod font;
pub mod text; pub use text::{fit,Text,text};
pub mod graphic; pub use {graphic::Graphic, vector::num::{Ratio,unit}};
pub mod line; pub use line::{line, parallelogram};