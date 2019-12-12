//#![allow(incomplete_features)]#![feature(const_generics,maybe_uninit_extra,maybe_uninit_ref,non_ascii_idents)]
mod core; pub use crate::core::*;
mod vector; pub use vector::*;
//pub use std::{env, process::{Command, Stdio}};
