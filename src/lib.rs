/*#![feature(let_chains)]
#![cfg_attr(feature="array_windows", feature(array_windows))]
#![cfg_attr(feature="coroutines", feature(coroutines,iter_from_coroutine))]
#![no_std] use origin_studio as std;

pub type Error = Box<dyn std::error::Error>;
pub type Result<T=(),E=Error> = std::result::Result<T, E>;
#[cfg(feature="fehler")] pub use fehler::throws;
pub mod prelude {
	pub use super::{Result,Error,size,int2, Target, Widget, App, run};
	#[cfg(feature="fehler")] pub use super::throws;
}

#[cfg(feature="coroutines")] pub mod line; #[cfg(feature="coroutines")] pub use line::{line, parallelogram};
pub mod color; pub use color::{bgrf,black,white,dark,background,foreground};
pub mod widget; pub use widget::{xy,size,int2, Widget, Target, Event, EventContext, ModifiersState};
mod app; pub use app::{App, run};
#[cfg(feature="font")] pub mod font;
#[cfg(feature="text")] pub mod text; #[cfg(feature="text")] pub use text::{fit,Text,text};
#[cfg(feature="graphic")] pub mod graphic; #[cfg(feature="graphic")] pub use {graphic::Graphic, num::{Ratio,unit}};
#[cfg(feature="edit")] pub mod edit; //pub use edit::TextEdit;
#[cfg(feature="plot")] pub mod plot; #[cfg(feature="plot")] pub use plot::{list, Plot};

pub fn time<T>(task: impl FnOnce() -> T) -> (T, std::time::Duration) { let time = std::time::Instant::now(); (task(), time.elapsed()) }*/
