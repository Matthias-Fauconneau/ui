#![allow(non_camel_case_types,non_upper_case_globals)]

#[derive(Debug)] enum Arg { UInt(u32), Int(i32), Array(Box<[u8]>), String(std::string::String) }
pub use Arg::*;

#[repr(C)] #[derive(Clone, Copy, Debug)] struct Message {
	id: u32,
	opcode: u16,
	size: u16
}
unsafe impl bytemuck::Zeroable for Message {}
unsafe impl bytemuck::Pod for Message {}

fn message(s: &mut impl std::io::Read) -> Message {
	let mut buf = [0; std::mem::size_of::<Message>()]; std::io::Read::read(s, &mut buf).unwrap(); *bytemuck::from_bytes(&buf)
}

enum Type { UInt, Int, Array, String }
fn args<const N: usize>(s: &mut impl std::io::Read, types: [Type; N]) -> [Arg; N] { types.map(|r#type| {
	//use std::io::Read;
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
	server: std::os::unix::net::UnixStream,
	last_id: std::sync::atomic::AtomicU32,
	names: Vec<std::string::String>,
}
impl Server {
	pub fn new(&mut self, name: &str) -> u32 {
		self.names.push(name.into());
		self.last_id.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
	}
	#[track_caller] pub fn sendmsg<const N: usize>(&mut self, id: u32, opcode: u16, args: [Arg; N], fd: Option<std::os::unix::io::RawFd>) {
		let mut request = Vec::new();
		use std::io::Write;
		let size = (2+N as u32+args.iter().map(|arg| if let String(arg) = arg { (arg.as_bytes().len() as u32+1+3)/4 } else { 0 }).sum::<u32>())*4;
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
			sendmsg::<()>(self.server.as_raw_fd(), &[std::io::IoSlice::new(&request)], &[ControlMessage::ScmRights(&[fd])], MsgFlags::empty(), None).unwrap();
		} else {
			self.server.write(&request).unwrap();
		}
	}
	#[track_caller] pub fn request<const N: usize>(&mut self, id: u32, opcode: u16, args: [Arg; N]) {
		//dbg!(&self.names[id as usize], opcode, &args);
		self.sendmsg(id, opcode, args, None)
	}
}
impl std::io::Read for Server {
	fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> { self.server.read(buf) }
}

// display: sync, get_registry(registry); error(id, code, message: string)
enum Display { sync, get_registry }
mod display { pub const error: u16 = 0; pub const delete_id: u16 = 1; }


// registry: bind(name, interface: string, version, id); global(name, interface: string, version)
enum Registry { bind }
mod registry { pub const global: u16 = 0; }


//output: ; geometry(x, y, w_mm, h_mm, subpixel, make: string, model: string, transform), mode(flags, width, height, refresh), done, scale(factor)
mod output { pub const geometry: u16 = 0; pub const mode: u16 = 1; pub const done: u16 = 2; pub const scale: u16 = 3; pub const name: u16 = 4; pub const description: u16 = 5; }

// surface: attach(buffer, x, y), set_buffer_scale(factor), damage_buffer(x,y,w,h); enter(output), leave(output)
enum Surface { destroy, attach, damage, frame, set_opaque_region, set_input_region, commit, set_buffer_transform, set_buffer_scale, damage_buffer }
mod surface { pub const enter: u16 = 0; }

enum Compositor { create_surface }

// seat: get_pointer(pointer), get_keyboard(keyboard); capabilities(capabilities), name(name: string)
enum Seat { get_pointer, get_keyboard }
mod seat { pub const capabilities: u16 = 0; pub const name: u16 = 1; }

// pointer: set_cursor(enter: serial, surface, hotspot_x, hotspot_y); enter(serial, surface, surface_x, surface_y), leave(serial, surface), motion(time, surface_x, surface_y), button(serial, time, button, state), axis(time, axis, value), frame, axis_source(_), axis_stop(time, axis)
enum Pointer { set_cursor }
mod pointer {
	pub const enter: u16 = 0;
	pub const leave: u16 = 1;
	pub const motion: u16 = 2;
	pub const button: u16 = 3;
	pub const axis: u16 = 4;
	pub const frame: u16 = 5;
	pub const axis_source: u16 = 6;
	pub const axis_stop: u16 = 7;
}