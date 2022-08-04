#![allow(non_camel_case_types,non_upper_case_globals, dead_code)]

#[derive(Debug)] pub(crate) enum Arg { UInt(u32), Int(i32), Array(Box<[u8]>), String(std::string::String) }

#[repr(C)] #[derive(Clone, Copy, Debug)] pub(crate) struct Message {
	pub(crate) id: u32,
	pub(crate) opcode: u16,
	size: u16
}
unsafe impl bytemuck::Zeroable for Message {}
unsafe impl bytemuck::Pod for Message {}

pub(crate) fn message(s: &mut impl std::io::Read) -> Message {
	let mut buf = [0; std::mem::size_of::<Message>()]; std::io::Read::read(s, &mut buf).unwrap(); *bytemuck::from_bytes(&buf)
}

pub(crate) enum Type { UInt, Int, Array, String }
fn args<const N: usize>(s: &mut impl std::io::Read, types: [Type; N]) -> [Arg; N] { types.map(|r#type| {
	let arg = {let mut buf = [0; 4]; s.read(&mut buf).unwrap(); *bytemuck::from_bytes::<u32>(&buf)};
	use Type::*;
	match r#type {
		UInt => Arg::UInt(arg),
		Int => Arg::Int(arg as i32),
		Array => {
			let array = {let mut buf = {let mut vec = Vec::new(); vec.resize((arg as usize+3)/4*4, 0); vec}; s.read(&mut buf).unwrap(); buf.truncate(arg as usize); buf};
			Arg::Array(array.into_boxed_slice())
		},
		String => {
			let string = {let mut buf = {let mut vec = Vec::new(); vec.resize((arg as usize+3)/4*4, 0); vec}; s.read(&mut buf).unwrap(); buf.truncate(arg as usize-1); buf};
			Arg::String(std::string::String::from_utf8(string).unwrap())
		}
	}
}) }

pub struct Server {
	pub(super) server: std::cell::RefCell<std::os::unix::net::UnixStream>,
	last_id: std::sync::atomic::AtomicU32,
}
impl Server {
	pub(crate) fn from(server: std::os::unix::net::UnixStream) -> Self { Self{server: std::cell::RefCell::new(server), last_id: std::sync::atomic::AtomicU32::new(2)} }
	pub(crate) fn next_id(&self) -> u32 { self.last_id.fetch_add(1, std::sync::atomic::Ordering::Relaxed) }
	pub fn new<'s: 't, 't, T: From<(&'t Self, u32)>>(&'s self) -> T { (self, self.next_id()).into() }
	#[track_caller] fn sendmsg<const N: usize>(&self, id: u32, opcode: u16, args: [Arg; N], fd: Option<std::os::unix::io::RawFd>) {
		assert!(opcode < 10);
		dbg!(id, opcode, &args);
		let mut request = Vec::new();
		use std::io::Write;
		let size = (2+N as u32+args.iter().map(|arg| if let Arg::String(arg) = arg { (arg.as_bytes().len() as u32+1+3)/4 } else { 0 }).sum::<u32>())*4;
		request.write(bytemuck::bytes_of(&Message{id, size: size as u16, opcode})).unwrap();
		for arg in args { use Arg::*; match arg {
			UInt(arg) => { request.write(bytemuck::bytes_of(&arg)).unwrap(); },
			String(arg) => {
				request.write(bytemuck::bytes_of::<u32>(&(arg.as_bytes().len() as u32+1))).unwrap();
				request.write(arg.as_bytes()).unwrap();
				request.write(&[0]).unwrap();
				while request.len()%4!=0 { request.write(&[0]).unwrap(); }
			}
			_ => unimplemented!("{arg:?}"),
		}; }
		assert!(request.len()==size as usize);
		if let Some(fd) = fd {
			use {std::os::unix::io::AsRawFd, nix::sys::socket::{sendmsg,ControlMessage,MsgFlags}};
			sendmsg::<()>(self.server.borrow_mut().as_raw_fd(), &[std::io::IoSlice::new(&request)], &[ControlMessage::ScmRights(&[fd])], //MsgFlags::empty()
			MsgFlags::MSG_DONTWAIT|MsgFlags::MSG_NOSIGNAL
			, None).unwrap();
		} else {
			self.server.borrow_mut().write(&request).unwrap();
		}
	}
	#[track_caller] fn request<const N: usize>(&self, id: u32, opcode: u16, args: [Arg; N]) { self.sendmsg(id, opcode, args, None) }
	pub(crate) fn args<const N: usize>(&self, types: [Type; N]) -> [Arg; N] { args(&mut*self.server.borrow_mut(), types) }
	pub(crate) fn globals<const N: usize>(&self, registry: &Registry, interfaces: [&str; N]) -> [u32; N] {
		let mut globals = [0; N];
		while globals.iter().any(|&item| item==0) {
			let Message{id, opcode, ..} = message(&mut*self.server.borrow_mut());
			assert!(id == registry.id && opcode == registry::global);
			use Arg::*;
			let args = {use Type::*; self.args([UInt, String, UInt])};
			let [UInt(name), String(interface), UInt(version)] = args else { panic!("{args:?}") };
			if let Some(index) = interfaces.iter().position(|&item| item==interface) {
				let id = self.next_id();
				registry.bind(name, &interface, version, id);
				globals[index] = id;
			}
		}
		globals
	}
}
/*impl std::io::Read for Server {
	fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> { self.server.read(buf) }
}*/

// display: sync, get_registry(registry); error(id, code, message: string)
pub(crate) mod display {
	pub const error: u16 = 0; pub const delete_id: u16 = 1;
	enum Requests { sync, get_registry }
	use super::{Server, Arg::*, *};
	pub struct Display<'t>{pub(crate) server: &'t Server, pub(crate) id: u32}
	impl<'t> From<(&'t Server, u32)> for Display<'t> { fn from((server, id): (&'t Server, u32)) -> Self { Self{server, id} }}
	impl Display<'_> {
		pub fn get_registry(&self, registry: &Registry) { self.server.request(self.id, Requests::get_registry as u16, [UInt(registry.id)]); }
	}
}
pub use display::Display;

// registry: bind(name, interface: string, version, id); global(name, interface: string, version)
pub(crate) mod registry {
	pub const global: u16 = 0;
	enum Requests { bind }
	use super::{Server, Arg::*};
	pub struct Registry<'t>{pub(crate) server: &'t Server, pub(crate) id: u32}
	impl<'t> From<(&'t Server, u32)> for Registry<'t> { fn from((server, id): (&'t Server, u32)) -> Self { Self{server, id} }}
	impl Registry<'_> {
		pub fn bind(&self, name: u32, interface: &str, version: u32, id: u32) {
			self.server.request(self.id, Requests::bind as u16, [UInt(name),String(interface.into()),UInt(version),UInt(id)]);
		}
	}
}
pub use registry::Registry;

// compositor: create_surface(surface)
pub(crate) mod compositor {
	enum Requests { create_surface }
	use super::{Server, Arg::*, *};
	pub struct Compositor<'t>{pub(crate) server: &'t Server, pub(crate) id: u32}
	impl<'t> From<(&'t Server, u32)> for Compositor<'t> { fn from((server, id): (&'t Server, u32)) -> Self { Self{server, id} }}
	impl Compositor<'_> {
		pub fn create_surface(&self, surface: &Surface) { self.server.request(self.id, Requests::create_surface as u16, [UInt(surface.id)]); }
	}
}
pub  use compositor::Compositor;

//output: ; geometry(x, y, w_mm, h_mm, subpixel, make: string, model: string, transform), mode(flags, width, height, refresh), done, scale(factor)
pub(crate) mod output {
	pub const geometry: u16 = 0; pub const mode: u16 = 1; pub const done: u16 = 2; pub const scale: u16 = 3; pub const name: u16 = 4; pub const description: u16 = 5;
	enum Requests { }
	use super::{Server};
	pub struct Output<'t>{pub(crate) server: &'t Server, pub(crate) id: u32}
	impl<'t> From<(&'t Server, u32)> for Output<'t> { fn from((server, id): (&'t Server, u32)) -> Self { Self{server, id} }}
	impl Output<'_> {
	}
}
pub use output::Output;

// surface: attach(buffer, x, y), commit, set_buffer_scale(factor), damage_buffer(x,y,w,h); enter(output), leave(output)
pub(crate) mod surface {
	pub const enter: u16 = 0;
	enum Requests { destroy, attach, damage, frame, set_opaque_region, set_input_region, commit, set_buffer_transform, set_buffer_scale, damage_buffer }
	use super::{Server, Arg::*, *};
	pub struct Surface<'t>{server: &'t Server, pub(crate) id: u32}
	impl<'t> From<(&'t Server, u32)> for Surface<'t> { fn from((server, id): (&'t Server, u32)) -> Self { Self{server, id} }}
	impl Surface<'_> {
		pub fn attach(&self, buffer: &Buffer, x: u32, y: u32) { self.server.request(self.id, Requests::attach as u16, [UInt(buffer.id),UInt(x),UInt(y)]); }
		pub fn commit(&self) { self.server.request(self.id, Requests::commit as u16, []); }
		pub fn set_buffer_scale(&self, factor: u32) { self.server.request(self.id, Requests::set_buffer_scale as u16, [UInt(factor)]); }
		pub fn damage_buffer(&self, x: u32, y: u32, w: u32, h: u32) { self.server.request(self.id, Requests::damage_buffer as u16, [UInt(x),UInt(y),UInt(w),UInt(h)]); }
	}
}
pub use surface::Surface;

// seat: get_pointer(pointer), get_keyboard(keyboard); capabilities(capabilities), name(name: string)
pub(crate) mod seat {
	pub const capabilities: u16 = 0; pub const name: u16 = 1;
	enum Requests { get_pointer, get_keyboard }
	use super::{Server, Arg::*, *};
	pub struct Seat<'t>{pub(crate) server: &'t Server, pub(crate) id: u32}
	impl<'t> From<(&'t Server, u32)> for Seat<'t> { fn from((server, id): (&'t Server, u32)) -> Self { Self{server, id} }}
	impl Seat<'_> {
		pub fn get_pointer(&self, pointer: &Pointer) { self.server.request(self.id, Requests::get_pointer as u16, [UInt(pointer.id)]); }
		pub fn get_keyboard(&self, keyboard: &Keyboard) { self.server.request(self.id, Requests::get_keyboard as u16, [UInt(keyboard.id)]); }
	}
}
pub use seat::Seat;

// pointer: set_cursor(enter: serial, surface, hotspot_x, hotspot_y); enter(serial, surface, surface_x, surface_y), leave(serial, surface), motion(time, surface_x, surface_y), button(serial, time, button, state), axis(time, axis, value), frame, axis_source(_), axis_stop(time, axis)
pub(crate) mod pointer {
	pub const enter: u16 = 0;
	pub const leave: u16 = 1;
	pub const motion: u16 = 2;
	pub const button: u16 = 3;
	pub const axis: u16 = 4;
	pub const frame: u16 = 5;
	pub const axis_source: u16 = 6;
	pub const axis_stop: u16 = 7;
	enum Requests { set_cursor }
	use super::{Server, Arg::*, *};
	pub struct Pointer<'t>{server: &'t Server, pub(crate) id: u32}
	impl<'t> From<(&'t Server, u32)> for Pointer<'t> { fn from((server, id): (&'t Server, u32)) -> Self { Self{server, id} }}
	impl Pointer<'_> {
		pub fn set_cursor(&self, serial: u32, surface: &Surface, hotspot_x: u32, hotspot_y: u32) { self.server.request(self.id, Requests::set_cursor as u16, [UInt(serial),UInt(surface.id),UInt(hotspot_x),UInt(hotspot_y)]); }
	}
}
pub use pointer::Pointer;

// keyboard: ; keymap(format, fd, size), enter(serial, surface, keys: array), leave(serial, surface), key(serial, time, key, state), modifiers(serial, depressed, latched, locked, group), repeat_info(rate, delay)
pub(crate) mod keyboard {
	pub const keymap: u16 = 0; pub const enter: u16 = 1; pub const leave: u16 = 2; pub const key: u16 = 3; pub const modifiers: u16 = 4; pub const repeat_info: u16 = 5;
	enum Requests {  }
	use super::{Server};
	pub struct Keyboard<'t>{server: &'t Server, pub(crate) id: u32}
	impl<'t> From<(&'t Server, u32)> for Keyboard<'t> { fn from((server, id): (&'t Server, u32)) -> Self { Self{server, id} }}
	impl Keyboard<'_> {
	}
}
pub use keyboard::Keyboard;

// shm: create_pool(shm_pool, fd, size); format(uint)
pub(crate) mod shm {
	pub const format: u16 = 0;
	enum Requests { create_pool }
	pub struct Shm<'t>{pub(crate) server: &'t Server, pub(crate) id: u32}
	impl<'t> From<(&'t Server, u32)> for Shm<'t> { fn from((server, id): (&'t Server, u32)) -> Self { Self{server, id} }}
	use super::{Arg::*, *};
	impl Shm<'_> {
		pub fn create_pool(&self, shm_pool: &ShmPool, fd: std::os::unix::io::RawFd, size: u32) {
			self.server.sendmsg(self.id, Requests::create_pool as u16, [UInt(shm_pool.id),UInt(size)], Some(fd));
		}
	}
}
pub use shm::Shm;

// shm_pool: create_buffer(buffer, offset, width, height, stride, shm.format), resize(size)
pub mod shm_pool {
	enum Requests { create_buffer, destroy, resize }
	pub struct ShmPool<'t>{server: &'t Server, pub(crate) id: u32}
	impl<'t> From<(&'t Server, u32)> for ShmPool<'t> { fn from((server, id): (&'t Server, u32)) -> Self { Self{server, id} }}
	use super::{Arg::*, *};
	impl ShmPool<'_> {
		pub fn create_buffer(&self, buffer: &Buffer, offset: u32, width: u32, height: u32, stride: u32, format: Format) {
			self.server.request(self.id, Requests::create_buffer as u16, [UInt(buffer.id),UInt(offset),UInt(width),UInt(height),UInt(stride),UInt(format as u32)]);
		}
		pub fn resize(&self, size: u32) { self.server.request(self.id, Requests::resize as u16, [UInt(size)]); }
	}
	pub enum Format { argb8888, xrgb8888 }
}
pub use shm_pool::ShmPool;

// buffer: destroy; release
pub(crate) mod buffer {
	pub const release: u16 = 0;
	enum Requests { destroy }
	use super::{Server};
	pub struct Buffer<'t>{server: &'t Server, pub(crate) id: u32}
	impl<'t> From<(&'t Server, u32)> for Buffer<'t> { fn from((server, id): (&'t Server, u32)) -> Self { Self{server, id} }}
	impl Buffer<'_> {
		pub fn destroy(&self) { self.server.request(self.id, Requests::destroy as u16, []); }
	}
}
pub use buffer::Buffer;

// wm_base: destroy, create_positioner, get_xdg_surface(xdg_surface, surface), pong(serial); ping(serial)
pub(crate) mod wm_base {
	pub const ping: u16 = 0;
	enum Requests { destroy, create_positioner, get_xdg_surface, pong }
	use super::{Server, Arg::*, *};
	pub struct WmBase<'t>{pub(crate) server: &'t Server, pub(crate) id: u32}
	impl<'t> From<(&'t Server, u32)> for WmBase<'t> { fn from((server, id): (&'t Server, u32)) -> Self { Self{server, id} }}
	impl WmBase<'_> {
		pub fn get_xdg_surface(&self, xdg_surface: &XdgSurface, surface: &Surface) {
			self.server.request(self.id, Requests::get_xdg_surface as u16, [UInt(xdg_surface.id),UInt(surface.id)]);
		}
		pub fn pong(&self, serial: u32) { self.server.request(self.id, Requests::pong as u16, [UInt(serial)]) }
	}
}
pub use wm_base::WmBase;

// xdg_surface: destroy, get_toplevel(toplevel), get_popup, set_window_geometry, ack_configure(serial); configure(serial)
pub(crate) mod xdg_surface {
	pub const configure: u16 = 0;
	enum Requests { destroy, get_toplevel, get_popup, set_window_geometry, ack_configure }
	use super::{Server, Arg::*, *};
	pub struct XdgSurface<'t>{server: &'t Server, pub(crate) id: u32}
	impl<'t> From<(&'t Server, u32)> for XdgSurface<'t> { fn from((server, id): (&'t Server, u32)) -> Self { Self{server, id} }}
	impl XdgSurface<'_> {
		pub fn get_toplevel(&self, toplevel: &Toplevel) { self.server.request(self.id, Requests::get_toplevel as u16, [UInt(toplevel.id)]); }
		pub fn ack_configure(&self, serial: u32) { self.server.request(self.id, Requests::ack_configure as u16, [UInt(serial)]) }
	}
}
pub use xdg_surface::XdgSurface;

// toplevel: set_title(title: string); configure(width, height, states: array), close, configure_bounds(width, height), wm_capabilities
pub(crate) mod toplevel {
	pub const configure: u16 = 0; pub const close: u16 = 1; pub const configure_bounds: u16 = 2;
	enum Requests { destroy, set_parent, set_title }
	use super::{Server, Arg::*};
	pub struct Toplevel<'t>{server: &'t Server, pub(crate) id: u32}
	impl<'t> From<(&'t Server, u32)> for Toplevel<'t> { fn from((server, id): (&'t Server, u32)) -> Self { Self{server, id} }}
	impl Toplevel<'_> {
		pub fn set_title(&self, title: &str) { self.server.request(self.id, Requests::set_title as u16, [String(title.into())]); }
	}
}
pub use toplevel::Toplevel;
