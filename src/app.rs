#[path="wayland.rs"] mod wayland;
use {num::{zero,IsZero}, vector::{xy, size, int2}, image::bgra, crate::{prelude::*, widget::{Target, Widget, EventContext, ModifiersState, Event}}, wayland::*};

pub struct Cursor<'t> {
	pointer: u32,
	surface: u32,
	buffer: u32,
	target: Target<'t>,
	serial: u32,
}
impl Cursor<'_> {
	pub fn set(&mut self, server: &mut Server, name: &str) {
		let image = xcursor::parser::parse_xcursor(&std::fs::read(xcursor::CursorTheme::load("default").load_icon(name).unwrap()).unwrap()).unwrap()[0];
		let hot = xy{x: image.xhot, y: image.yhot};
		let image = image::Image::cast_slice(&image.pixels_argb, xy{x: image.width, y: image.height});
	    assert!(self.target.size == image.size);
		self.target.data.copy_from_slice(&image);

		server.request(self.surface, Surface::attach as u16, [UInt(self.buffer),UInt(0),UInt(0)]);
        server.request(self.surface, Surface::commit as u16, []);
		server.request(self.pointer, Pointer::set_cursor as u16, [UInt(self.serial),UInt(self.surface),UInt(hot.x),UInt(hot.y)]);
	}
}

#[throws] pub fn run(widget: &mut dyn Widget/*, idle: &mut dyn FnMut(&mut dyn Widget)->Result<bool>*/) {
	let server = std::os::unix::net::UnixStream::connect({
		let mut path = std::path::PathBuf::from(std::env::var_os("XDG_RUNTIME_DIR").unwrap());
		path.push(std::env::var_os("WAYLAND_DISPLAY").unwrap());
		path
	})?;

	let display = 1;
	let last_id = std::sync::atomic::AtomicU32::new(display+1);
	let ref mut server = Server{server, last_id, names: ["".into(),"display".into()].into()};

	use Arg::*;

	let registry = server.new("registry");
	server.request(display, Display::get_registry as u16, [UInt(registry)]);

	fn globals<const N: usize>(server: &mut Server, registry: u32, interfaces: [&str; N]) -> [u32; N] {
		let mut globals = [0; N];
		while globals.iter().any(|&item| item==0) {
			let Message{id, opcode, ..} = message(server);
			assert!(id == registry && opcode == registry::global);
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

	let [compositor, shm, output, wm_base, seat] = globals(server, registry, ["wl_compositor", "wl_shm", "wl_output", "xdg_wm_base", "wl_seat"]);


	let pointer = server.new("pointer");
	server.request(seat, Seat::get_pointer as u16, [UInt(pointer)]);

	// keyboard: ; keymap(format, fd, size), enter(serial, surface, keys: array), leave(serial, surface), key(serial, time, key, state), modifiers(serial, depressed, latched, locked, group), repeat_info(rate, delay)
	mod keyboard { pub const keymap: u16 = 0; pub const enter: u16 = 1; pub const leave: u16 = 2; pub const key: u16 = 3; pub const modifiers: u16 = 4; pub const repeat_info: u16 = 5; }

	let keyboard = server.new("keyboard");
	server.request(seat, Seat::get_keyboard as u16, [UInt(keyboard)]);

	let file = rustix::fs::memfd_create("target", rustix::fs::MemfdFlags::empty()).unwrap();

	// shm: create_pool(shm_pool, fd, size); format(uint)
	enum Shm { create_pool }
	mod shm { pub const format: u16 = 0; }

	// shm_pool: create_buffer(buffer, offset, width, height, stride, shm.format), resize(size)
	enum ShmPool { create_buffer, destroy, resize }
	enum ShmFormat { argb8888, xrgb8888 }

	// buffer: destroy; release
	enum Buffer { destroy }
	mod buffer { pub const release: u16 = 0; }

	let pool = server.new("pool");
	use std::os::unix::io::AsRawFd;
	rustix::fs::ftruncate(&file, 1).unwrap();
	server.sendmsg(shm, Shm::create_pool as u16, [UInt(pool), UInt(1)], Some(file.as_raw_fd()));

	let buffer = server.new("buffer");

	struct Pool<'t> {
		file: rustix::io::OwnedFd,
		id: u32,
		buffer: u32,
		target: Target<'t>,
	}
	let ref mut pool = Pool{file, id: pool, buffer, target: Target::new(zero(), &mut [])};

	let surface = server.new("surface");
	server.request(compositor, Compositor::create_surface as u16, [UInt(surface)]);

	// wm_base: destroy, create_positioner, get_xdg_surface(xdg_surface, surface), pong(serial); ping(serial)
	enum WmBase { destroy, create_positioner, get_xdg_surface, pong }
	mod wm_base { pub const ping: u16 = 0; }

	let xdg_surface = server.new("xdg_surface");
	server.request(wm_base, WmBase::get_xdg_surface as u16, [UInt(xdg_surface), UInt(surface)]);

	// xdg_surface: destroy, get_toplevel(toplevel), get_popup, set_window_geometry, ack_configure(serial); configure(serial)
	enum XdgSurface { destroy, get_toplevel, get_popup, set_window_geometry, ack_configure }
	mod xdg_surface { pub const configure: u16 = 0; }

	let toplevel = server.new("toplevel");
	server.request(xdg_surface, XdgSurface::get_toplevel as u16, [UInt(toplevel)]);

	// toplevel: set_title(title: string); configure(width, height, states: array), close, configure_bounds(width, height), wm_capabilities
	enum TopLevel { destroy, set_parent, set_title }
	mod toplevel { pub const configure: u16 = 0; pub const close: u16 = 1; pub const configure_bounds: u16 = 2; }

	server.request(toplevel, TopLevel::set_title as u16, [String("App".into())]);
	server.request(surface, Surface::commit as u16, []);

	let mut scale_factor = 3;

	let ref mut cursor = {
		let surface = server.new("surface");
		server.request(compositor, Compositor::create_surface as u16, [UInt(surface)]);
		let file = rustix::fs::memfd_create("cursor", rustix::fs::MemfdFlags::empty()).unwrap();
		let size = xy{x: 24*scale_factor, y: 24*scale_factor};
		let length = (size.y*size.x*4) as usize;
		rustix::fs::ftruncate(&file, length as u64).unwrap();
		let pool = server.new("pool");
		server.sendmsg(shm, Shm::create_pool as u16, [UInt(pool), UInt(length as u32)], Some(file.as_raw_fd()));
		let buffer = server.new("buffer");
		server.request(pool, ShmPool::create_buffer as u16, [UInt(buffer), UInt(0), UInt(size.x), UInt(size.y), UInt(size.x*4), UInt(ShmFormat::argb8888 as u32)]);
		let mmap = unsafe{std::slice::from_raw_parts_mut(
					rustix::mm::mmap(std::ptr::null_mut(), length, rustix::mm::ProtFlags::READ | rustix::mm::ProtFlags::WRITE, rustix::mm::MapFlags::SHARED, &file, 0).unwrap() as *mut u8,
					length)};
		let target = Target::new(size, bytemuck::cast_slice_mut(mmap));
		Cursor{pointer, surface, buffer, target, serial: 0}
	};


	#[throws] fn paint(pool: &mut Pool, size: size, widget: &mut dyn Widget, server: &mut Server, surface: u32) {
		if pool.target.size != size {
			let length = (size.y*size.x*4) as usize;
			rustix::fs::ftruncate(&pool.file, length as u64).unwrap();
			server.request(pool.id, ShmPool::resize as u16, [UInt(length as u32)]);

			if !pool.target.size.is_zero() { server.request(pool.buffer, Buffer::destroy as u16, []); }
			server.request(pool.id, ShmPool::create_buffer as u16, [UInt(pool.buffer), UInt(0), UInt(size.x), UInt(size.y), UInt(size.x*4), UInt(ShmFormat::xrgb8888 as u32)]);

			let mmap = unsafe{std::slice::from_raw_parts_mut(
				rustix::mm::mmap(std::ptr::null_mut(), length, rustix::mm::ProtFlags::READ | rustix::mm::ProtFlags::WRITE, rustix::mm::MapFlags::SHARED, &pool.file, 0).unwrap() as *mut u8,
				length)};
			pool.target = Target::new(size, bytemuck::cast_slice_mut(mmap));
		}
		pool.target.fill(bgra{b:0, g:0, r:0, a:0xFF});
		//target.fill(bgra{b:0xFF, g:0xFF, r:0xFF, a:0xFF});
		let size = pool.target.size;
		widget.paint(&mut pool.target, size, zero())?;
		server.request(surface, Surface::attach as u16, [UInt(pool.buffer),UInt(0),UInt(0)]);
		server.request(surface, Surface::damage_buffer as u16, [UInt(0),UInt(0),UInt(pool.target.size.x),UInt(pool.target.size.y)]);
		server.request(surface, Surface::commit as u16, []);
	}

	let mut size = zero();
	let mut modifiers_state = ModifiersState::default();
	let mut pointer_position = int2::default();

	loop {
		let Message{id, opcode, ..} = message(server);
		//println!("{} {opcode}", server.names[id as usize]);
		/**/ if id == display && opcode == display::error {
			println!("{:?}", args(server, {use Type::*; [UInt, UInt, String]}));
		}
		else if id == display && opcode == display::delete_id {
			let [UInt(id)] = args(server, {use Type::*; [UInt]}) else {panic!()};
			assert!(id == buffer); // Reused immediately
		}
		else if id == registry && opcode == registry::global {
			args(server, {use Type::*; [UInt, String, UInt]});
		}
		else if id == shm && opcode == shm::format {
			args(server, {use Type::*; [UInt]});
		}
		else if id == output && opcode == output::geometry {
			args(server, {use Type::*; [UInt, UInt, UInt, UInt, UInt, String, String, UInt]});
		}
		else if id == output && opcode == output::mode {
			let [_, UInt(x), UInt(y), _] = args(server, {use Type::*; [UInt, UInt, UInt, UInt]}) else {panic!()};
			let _configure_bounds = xy{x,y};
		}
		else if id == output && opcode == output::scale {
			let [UInt(factor)] = args(server, {use Type::*; [UInt]}) else {panic!()};
			scale_factor = factor;
			server.request(surface, Surface::set_buffer_scale as u16, [UInt(scale_factor)]);
		}
		else if id == output && opcode == output::name {
			args(server, {use Type::*; [String]});
		}
		else if id == output && opcode == output::description {
			args(server, {use Type::*; [String]});
		}
		else if id == output && opcode == output::done {
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
			dbg!()
		}
		else if id == toplevel && opcode == toplevel::configure {
			let [UInt(x),UInt(y),_] = args(server, {use Type::*; [UInt,UInt,Array]}) else {panic!()};
			size = xy{x: x*scale_factor, y: y*scale_factor};
			if size.is_zero() { size = widget.size(size); }
			//vector::component_wise_min(size, widget.size(size));
			// unscaled_size=xy{x:width as u32, y:height as u32};
		}
		else if id == xdg_surface && opcode == xdg_surface::configure {
			let [UInt(serial)] = args(server, {use Type::*; [UInt]}) else {panic!()};
			server.request(xdg_surface, XdgSurface::ack_configure as u16, [UInt(serial)]);
			paint(pool, size, widget, server, surface)?;
		}
		else if id == surface && opcode == surface::enter {
			let [UInt(_output)] = args(server, {use Type::*; [UInt]}) else {panic!()};
		}
		else if id == buffer && opcode == buffer::release {
		}
		else if id == pointer && opcode == pointer::enter {
			let [UInt(serial),_,_,_] = args(server, {use Type::*; [UInt,UInt,UInt,UInt]}) else {panic!()};
			cursor.serial = serial;
		}
		else if id == pointer && opcode == pointer::leave {
			args(server, {use Type::*; [UInt,UInt]});
		}
		else if id == pointer && opcode == pointer::motion {
			let [_,Int(x),Int(y)] = args(server, {use Type::*; [UInt,Int,Int]}) else {panic!()};
			pointer_position = xy{x: x*scale_factor as i32/256,y: y*scale_factor as i32/256};
		}
		else if id == pointer && opcode == pointer::button {
			let [_,_,UInt(button),UInt(state)] = args(server, {use Type::*; [UInt,UInt,UInt,UInt]}) else {panic!()};
			#[allow(non_upper_case_globals)] const usb_hid_buttons: [u32; 2] = [272, 111];
			let button = usb_hid_buttons.iter().position(|&b| b == button).unwrap_or_else(|| panic!("{:x}", button)) as u8;
			//if state>0 { *mouse_buttons |= 1<<button; } else { *mouse_buttons &= !(1<<button); }
			if widget.event(size, &mut EventContext{modifiers_state, server, cursor}, &Event::Button{position: pointer_position, button: button as u8, state: state as u8})? {
					paint(pool, size, widget, server, surface)?;
			}
		}
		else if id == pointer && opcode == pointer::axis {
			let [_,UInt(axis),Int(value)] = args(server, {use Type::*; [UInt,UInt,Int]}) else {panic!()};
			if axis != 0 { continue; }
			if widget.event(size, &mut EventContext{modifiers_state, server, cursor}, &Event::Scroll(value*scale_factor as i32/256))? {
				paint(pool, size, widget, server, surface)?;
			}
		}
		else if id == pointer && opcode == pointer::frame {
			args(server, []);
		}
		else if id == pointer && opcode == pointer::axis_source {
			args(server, {use Type::*; [UInt]});
		}
		else if id == pointer && opcode == pointer::axis_stop {
			args(server, {use Type::*; [UInt,UInt]});
		}
		else if id == keyboard && opcode == keyboard::keymap {
			args(server, {use Type::*; [UInt,UInt]});
		}
		else if id == keyboard && opcode == keyboard::repeat_info {
			args(server, {use Type::*; [UInt,UInt]});
		}
		else if id == keyboard && opcode == keyboard::modifiers {
			let [_,UInt(depressed),_,_,_] = args(server, {use Type::*; [UInt,UInt,UInt,UInt,UInt]}) else {panic!()};
			const SHIFT: u32 = 0b1;
			const CTRL: u32 = 0b100;
			const ALT: u32 = 0b1000;
			const LOGO: u32 = 0b1000000;
			modifiers_state = ModifiersState{
				shift: depressed&SHIFT != 0,
				ctrl: depressed&CTRL != 0,
				logo: depressed&LOGO != 0,
				alt: depressed&ALT != 0,
			};
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
			let [_serial,_time,UInt(key),UInt(state)] = args(server, {use Type::*; [UInt,UInt,UInt,UInt]}) else {panic!()};
			#[allow(non_upper_case_globals)] static usb_hid_usage_table: std::sync::LazyLock<Vec<char>> = std::sync::LazyLock::new(|| [
				&['\0','⎋','1','2','3','4','5','6','7','8','9','0','-','=','⌫','\t','q','w','e','r','t','y','u','i','o','p','{','}','\n','⌃','a','s','d','f','g','h','j','k','l',';','\'','`','⇧','\\','z','x','c','v','b','n','m',',','.','/','⇧','\0','⎇',' ','⇪'],
				&(1..=10).map(|i| (0xF700u32+i).try_into().unwrap()).collect::<Vec<_>>()[..], &['\0'; 20], &['\u{F70B}','\u{F70C}'], &['\0'; 8],
				&['⎙','⎄',' ','⇤','↑','⇞','←','→','⇥','↓','⇟','⎀','⌦','\u{F701}','🔇','🕩','🕪','⏻','=','±','⏯','🔎',',','\0','\0','¥','⌘']].concat());
			let key = usb_hid_usage_table.get(key as usize).unwrap();
			if state > 0 {
				if *key == '⎋' { break; }
				if widget.event(size, &mut EventContext{modifiers_state, server, cursor}, &Event::Key(*key))? {
					paint(pool, size, widget, server, surface)?;
				}
				/*repeat = {
					let repeat = std::rc::Rc::new(std::cell::Cell::new(key));
					let from_monotonic_millis = |t| {
						let now = {let rustix::time::Timespec{tv_sec, tv_nsec} = rustix::time::clock_gettime(rustix::time::ClockId::Monotonic); tv_sec * 1000 + tv_nsec / 1000_000};
						std::time::Instant::now() - std::time::Duration::from_millis((now - t as i64) as u64)
					};
					use futures_lite::StreamExt;
					state.streams.push(
						async_io::Timer::interval_at(from_monotonic_millis(time)+std::time::Duration::from_millis(150), std::time::Duration::from_millis(33))
						.filter_map({
							let repeat = std::rc::Rc::downgrade(&repeat);
							// stops and autodrops from streams when weak link fails to upgrade (repeat cell dropped)
							move |_| { repeat.upgrade().map(|x| {let key = x.get(); (box move |w| { w.key(key)?; w.draw() }) as Box::<dyn FnOnce(&mut App<'t,W>)->Result<()>>}) }
						})
						.fuse()
						.boxed_local()
					);
					Some(repeat)
				};*/
			} //else if repeat.as_ref().filter(|r| r.get()==key ).is_some() { repeat = None }
		}
		else if id == toplevel && opcode == toplevel::close {
			break;
		}
		else { panic!("{:?} {opcode:?}", &server.names[id as usize]); }
	}
}
