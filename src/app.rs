use {vector::xy, image::{Image, bgra}, crate::{prelude::*, widget::Widget}};

#[repr(C)] #[derive(Clone, Copy, Debug)] struct Message {
	id: u32,
	opcode: u16,
	size: u16
}
unsafe impl bytemuck::Zeroable for Message {}
unsafe impl bytemuck::Pod for Message {}
#[derive(Debug)] enum Arg { UInt(u32), Array(Box<[u8]>), String(String) }

fn message(s: &mut impl std::io::Read) -> Message {
	let mut buf = [0; std::mem::size_of::<Message>()]; std::io::Read::read(s, &mut buf).unwrap(); *bytemuck::from_bytes(&buf)
}

enum Type { UInt, Array, String }
fn args<const N: usize>(s: &mut impl std::io::Read, types: [Type; N]) -> [Arg; N] { types.map(|r#type| {
	//use std::io::Read;
	let arg = {let mut buf = [0; 4]; s.read(&mut buf).unwrap(); *bytemuck::from_bytes::<u32>(&buf)};
	use Type::*;
	match r#type {
		UInt => Arg::UInt(arg),
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

struct Server {
	server: std::os::unix::net::UnixStream,
	last_id: std::sync::atomic::AtomicU32,
	names: /*std::cell::Cell<*/Vec<String>,//>,
}
impl Server {
	fn new(&mut self, name: &str) -> u32 {
		self.names.push(name.into());
		self.last_id.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
	}
	fn sendmsg<const N: usize>(&mut self, id: u32, opcode: u16, args: [Arg; N], fd: Option<std::os::unix::io::RawFd>) {
		//println!("{} {opcode} {args:?}", &self.names[id as usize]);
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
			sendmsg::<()>(self.server.as_raw_fd(), &[std::io::IoSlice::new(&request)], &[ControlMessage::ScmRights(&[fd])], MsgFlags::empty(), None).unwrap();
		} else {
			self.server.write(&request).unwrap();
		}
	}
	fn request<const N: usize>(&mut self, id: u32, opcode: u16, args: [Arg; N]) { self.sendmsg(id, opcode, args, None) }
}
impl std::io::Read for Server {
	fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> { self.server.read(buf) }
}

#[allow(non_camel_case_types, non_upper_case_globals,dead_code,unreachable_code)] #[throws] pub fn run(widget: &mut dyn Widget/*, idle: &mut dyn FnMut(&mut dyn Widget)->Result<bool>*/) {
	let server = std::os::unix::net::UnixStream::connect({
		let mut path = std::path::PathBuf::from(std::env::var_os("XDG_RUNTIME_DIR").unwrap());
		path.push(std::env::var_os("WAYLAND_DISPLAY").unwrap());
		path
	})?;

	let display = 1;
	let last_id = std::sync::atomic::AtomicU32::new(display+1);
	let ref mut server = Server{server, last_id, names: ["".into(),"display".into()].into()};

	// display: sync, get_registry(registry); error(id, code, message: string)
	enum Display { sync, get_registry }
	const error : u16 = 0;

	use Arg::*;

	let registry = server.new("registry");
	server.request(display, Display::get_registry as u16, [UInt(registry)]);

	// registry: bind(name, interface: string, version, id); global(name, interface: string, version)
	enum Registry { bind }
	const global : u16 = 0;

	fn globals<const N: usize>(server: &mut Server, registry: u32, interfaces: [&str; N]) -> [u32; N] {
		let mut globals = [0; N];
		while globals.iter().any(|&item| item==0) {
			let Message{id, opcode, ..} = message(server);
			assert!(id == registry && opcode == global);
			use Arg::*;
			let args = args(server, {use Type::*; [UInt, String, UInt]});
			let [UInt(name), String(interface), UInt(version)] = args else { panic!("{args:?}") };
			if let Some(index) = interfaces.iter().position(|&item| item==interface) {
				let id = server.new(&interface);
				server.request(registry, Registry::bind as u16, [UInt(name), String(interface.into()), UInt(version), UInt(id)]);
				globals[index] = id;
			}
		}
		globals
	}

	let [ compositor, shm, output, wm_base, seat] = globals(server, registry, ["wl_compositor", "wl_shm", "wl_output", "xdg_wm_base", "wl_seat"]);

	//output: ; geometry(x, y, w_mm, h_mm, subpixel, make: string, model: string, transform), mode(flags, width, height, refresh), done, scale(factor)
	const geometry : u16 = 0; const mode : u16 = 1; const done : u16 = 2; const scale : u16 = 3;

	let mut scale_factor = 3;
	let configure_bounds = xy{x: 3840, y: 2400};
	let size = widget.size(configure_bounds);
	let file = rustix::fs::memfd_create("buffer", rustix::fs::MemfdFlags::empty()).unwrap();
	let length = (size.y*size.x*4) as usize;
	rustix::fs::ftruncate(&file, length as u64).unwrap();
	enum Shm { create_pool } // shm: create_pool(shm_pool, fd, size); format(uint)
	const format : u16 = 0;
	let pool = server.new("pool");
	use std::os::unix::io::AsRawFd;
	server.sendmsg(shm, Shm::create_pool as u16, [UInt(pool), UInt(length as u32)], Some(file.as_raw_fd()));

	// buffer: ; release
	mod buffer { pub const release : u16 = 0; }

	// shm_pool: create_buffer(buffer, offset, width, height, stride, shm.format)
	enum ShmPool { create_buffer }
	enum ShmFormat { argb8888, xrgb8888 }

	let buffer = server.new("buffer");
	server.request(pool, ShmPool::create_buffer as u16, [UInt(buffer), UInt(0), UInt(size.x), UInt(size.y), UInt(size.x*4), UInt(ShmFormat::xrgb8888 as u32)]);

	// surface: attach(buffer, x, y), set_buffer_scale(factor); enter(output)
	enum Surface { destroy, attach, damage, frame, set_opaque_region, set_input_region, commit, set_buffer_transform, set_buffer_scale }
	mod surface { pub const enter : u16 = 0; }

	enum Compositor { create_surface }

	let create_surface = #[track_caller] |server: &mut Server, compositor| {
		let id = server.new("surface");
		server.request(compositor, Compositor::create_surface as u16, [UInt(id)]);
		id
	};

	let surface = create_surface(server, compositor);
	server.request(surface, Surface::set_buffer_scale as u16, [UInt(scale_factor)]);
	server.request(surface, Surface::attach as u16, [UInt(buffer),UInt(0),UInt(0)]);

	// wm_base: destroy, create_positioner, get_xdg_surface(xdg_surface, surface), pong(serial); ping(serial)
	enum WmBase { destroy, create_positioner, get_xdg_surface, pong }
	mod wm_base { pub const ping : u16 = 0; }

	let xdg_surface = server.new("xdg_surface");
	server.request(wm_base, WmBase::get_xdg_surface as u16, [UInt(xdg_surface), UInt(surface)]);

	// xdg_surface: destroy, get_toplevel(toplevel), get_popup, set_window_geometry, ack_configure(serial); configure(serial)
	enum XdgSurface { destroy, get_toplevel, get_popup, set_window_geometry, ack_configure }
	mod xdg_surface { pub const configure : u16 = 0; }

	let toplevel = server.new("toplevel");
	server.request(xdg_surface, XdgSurface::get_toplevel as u16, [UInt(toplevel)]);

	// toplevel: set_title(title: string); configure(width, height, states: array), close, configure_bounds(width, height), wm_capabilities
	enum TopLevel { destroy, set_parent, set_title }
	mod toplevel { pub const configure : u16 = 0; pub const configure_bounds : u16 = 2; }

	server.request(toplevel, TopLevel::set_title as u16, [String("App".into())]);

	let target = unsafe{std::slice::from_raw_parts_mut(
		rustix::mm::mmap(std::ptr::null_mut(), length, rustix::mm::ProtFlags::READ | rustix::mm::ProtFlags::WRITE, rustix::mm::MapFlags::SHARED, &file, 0).unwrap() as *mut u8,
		length)};
	let mut target = Image::new(size, bytemuck::cast_slice_mut(target));
	target.fill(bgra{b:0xFF, g:0xFF, r:0xFF, a:0xFF});

	widget.paint(&mut target, size, num::zero())?;

	server.request(surface, Surface::commit as u16, []);

	// seat: get_pointer(pointer), get_keyboard(keyboard); capabilities(capabilities), name(name: string)
	enum Seat { get_pointer, get_keyboard }
	mod seat { pub const capabilities : u16 = 0; pub const name : u16 = 1; }

	// keyboard: ; keymap(format, fd, size), enter(serial, surface, keys: array), leave(serial, surface), key(serial, time, key, state), modifiers(serial, depressed, latched, locked, group), repeat_info(rate, delay)
	mod keyboard { pub const keymap : u16 = 0; pub const enter : u16 = 1; pub const leave : u16 = 2; pub const key : u16 = 3; pub const modifiers : u16 = 4; pub const repeat_info : u16 = 5; }

	let keyboard = server.new("keyboard");
	server.request(seat, Seat::get_keyboard as u16, [UInt(keyboard)]);

	loop {
		let Message{id, opcode, ..} = message(server);
		//println!("{} {opcode}", server.names[id as usize]);
		/**/ if id == display && opcode == error {
			println!("{:?}", args(server, {use Type::*; [UInt, UInt, String]}));
		}
		else if id == registry && opcode == global {
			args(server, {use Type::*; [UInt, String, UInt]});
		}
		else if id == shm && opcode == format {
			args(server, {use Type::*; [UInt]});
		}
		else if id == output && opcode == geometry {
			args(server, {use Type::*; [UInt, UInt, UInt, UInt, UInt, String, String, UInt]});
		}
		else if id == output && opcode == mode {
			let [_, UInt(x), UInt(y), _] = args(server, {use Type::*; [UInt, UInt, UInt, UInt]}) else {panic!()};
			let _configure_bounds = xy{x,y};
		}
		else if id == output && opcode == done {
		}
		else if id == output && opcode == scale {
			let [UInt(factor)] = args(server, {use Type::*; [UInt]}) else {panic!()};
			#[allow(unused_assignments)] scale_factor = factor;
		}
		else if id == seat && opcode == seat::capabilities {
			args(server, {use Type::*; [UInt]});
		}
		else if id == seat && opcode == seat::name {
			args(server, {use Type::*; [String]});
		}
		else if id == toplevel && opcode == toplevel::configure_bounds {
			args(server, {use Type::*; [UInt,UInt]});
			//configure_bounds=xy{x:width as u32, y:height as u32};
		}
		else if id == toplevel && opcode == toplevel::configure {
			args(server, {use Type::*; [UInt,UInt,Array]});
			// unscaled_size=xy{x:width as u32, y:height as u32};
		}
		else if id == xdg_surface && opcode == xdg_surface::configure {
			let [UInt(serial)] = args(server, {use Type::*; [UInt]}) else {panic!()};
			server.request(xdg_surface, XdgSurface::ack_configure as u16, [UInt(serial)]);
		}
		else if id == surface && opcode == surface::enter {
			let [UInt(_output)] = args(server, {use Type::*; [UInt]}) else {panic!()};
		}
		else if id == buffer && opcode == buffer::release {
		}
		else if id == keyboard && opcode == keyboard::keymap {
			args(server, {use Type::*; [UInt,UInt]});
		}
		else if id == keyboard && opcode == keyboard::repeat_info {
			args(server, {use Type::*; [UInt,UInt]});
		}
		else if id == keyboard && opcode == keyboard::modifiers {
			args(server, {use Type::*; [UInt,UInt,UInt,UInt,UInt]});
		}
		else if id == keyboard && opcode == keyboard::enter {
			args(server, {use Type::*; [UInt,UInt,Array]});
		}
		else if id == keyboard && opcode == keyboard::leave {
			args(server, {use Type::*; [UInt,UInt]});
		}
		else if id == wm_base && opcode == wm_base::ping {
			let [UInt(serial)] = args(server, {use Type::*; [UInt]}) else {panic!()};
			server.request(wm_base, WmBase::pong as u16, [UInt(serial)]);
		}
		else if id == keyboard && opcode == keyboard::key {
			break;
		}
		/*else if id == toplevel && opcode == toplevel::close {
			// xdg_surface = None,
		}*/
		else { panic!("{:?} {opcode:?}", &server.names[id as usize]); }
	}
}
