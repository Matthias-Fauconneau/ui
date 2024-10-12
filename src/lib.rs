/*#![feature(let_chains)]
#![cfg_attr(feature="array_windows", feature(array_windows))]
#![cfg_attr(feature="coroutines", feature(coroutines,iter_from_coroutine))]*/
#![cfg_attr(feature="no-std", no_std)] 
use std::boxed::Box;

pub type Error = Box<dyn core::error::Error>;
pub type Result<T=(),E=Error> = core::result::Result<T, E>;
//#[cfg(feature="fehler")] pub use fehler::throws;
pub mod prelude {
	pub use super::{Result,Error,size,int2, Target, Widget, App, run};
	//#[cfg(feature="fehler")] pub use super::throws;
}

//#[cfg(feature="coroutines")] pub mod line; #[cfg(feature="coroutines")] pub use line::{line, parallelogram};
pub use image::{bgr, bgrf};
//let [black, white] : [Color; 2]  = [0., 1.].map(Into::into);
#[allow(non_upper_case_globals)] pub const black : bgrf = /*(0.).into()*/bgrf{b: 0., g: 0., r: 0.};
#[allow(non_upper_case_globals)] pub const white : bgrf = /*(1.).into()*/bgrf{b: 1., g: 1., r: 1.};
#[allow(non_upper_case_globals)] pub static dark : bool = false;
//const [background, foreground] : [Color; 2] = if dark { [black, white] } else { [white, black] };
#[allow(non_upper_case_globals)] pub fn background() -> bgrf { if dark { black } else { white } }
#[allow(non_upper_case_globals)] pub fn foreground() -> bgrf { if dark { white } else { black } }

//pub mod color; pub use color::{bgrf,black,white,dark,background,foreground};
pub mod widget; pub use widget::{xy,size,int2, Widget, Target, Event, EventContext, ModifiersState};
mod app; pub use app::{App, run};
/*#[cfg(feature="font")] pub mod font;
#[cfg(feature="text")] pub mod text; #[cfg(feature="text")] pub use text::{fit,Text,text};
#[cfg(feature="graphic")] pub mod graphic; #[cfg(feature="graphic")] pub use {graphic::Graphic, num::{Ratio,unit}};
#[cfg(feature="edit")] pub mod edit; //pub use edit::TextEdit;
#[cfg(feature="plot")] pub mod plot; #[cfg(feature="plot")] pub use plot::{list, Plot};

pub fn time<T>(task: impl FnOnce() -> T) -> (T, std::time::Duration) { let time = std::time::Instant::now(); (task(), time.elapsed()) }*/
