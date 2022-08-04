#[path="wayland.rs"] mod wayland;
use {num::{zero,IsZero}, vector::{xy, size, int2}, image::bgra, crate::{prelude::*, widget::{Target, Widget, EventContext, ModifiersState, Event}}, wayland::*};

pub struct Cursor<'t> {
	pointer: &'t Pointer<'t>,
	surface: Surface<'t>,
	buffer: Buffer<'t>,
	target: Target<'t>,
	serial: u32,
}
impl Cursor<'_> {
	pub fn set(&mut self, name: &str) {
		let image = &xcursor::parser::parse_xcursor(&std::fs::read(xcursor::CursorTheme::load("default").load_icon(name).unwrap()).unwrap()).unwrap()[0];
		let hot = xy{x: image.xhot, y: image.yhot};
		let image = image::Image::cast_slice(&image.pixels_argb, xy{x: image.width, y: image.height});
	    assert!(self.target.size == image.size);
		self.target.data.copy_from_slice(&image);
		self.surface.attach(&self.buffer,0,0);
        self.surface.commit();
		self.pointer.set_cursor(self.serial, &self.surface, hot.x, hot.y);
	}
}

#[throws] pub fn run(widget: &mut dyn Widget/*, idle: &mut dyn FnMut(&mut dyn Widget)->Result<bool>*/) {
	let server = std::os::unix::net::UnixStream::connect({
		let mut path = std::path::PathBuf::from(std::env::var_os("XDG_RUNTIME_DIR").unwrap());
		path.push(std::env::var_os("WAYLAND_DISPLAY").unwrap());
		path
	})?;
	let ref server = Server::from(server);
	let display = Display{server, id: 1};

	let ref registry = server.new();
	display.get_registry(registry);

	let [shm, compositor, wm_base, seat, output] = server.globals(registry, ["wl_shm", "wl_compositor",  "xdg_wm_base", "wl_seat", "wl_output"]);
	let shm = Shm{server, id: shm};
	let compositor = Compositor{server, id: compositor};
	let wm_base = WmBase{server, id: wm_base};
	let seat = Seat{server, id: seat};
	let output = Output{server, id: output};

	let ref surface = server.new();
	compositor.create_surface(surface);
	let mut scale_factor = 0;

	loop {
		let Message{id, opcode, ..} = message(&mut*server.server.borrow_mut());
		use Arg::*;
		/**/ if id == display.id && opcode == display::error {
			panic!("{:?}", server.args({use Type::*; [UInt, UInt, String]}));
		}
		else if id == registry.id && opcode == registry::global {
			server.args({use Type::*; [UInt, String, UInt]});
		}
		else if id == shm.id && opcode == shm::format {
			server.args({use Type::*; [UInt]});
		}
		else if id == seat.id && opcode == seat::capabilities {
			server.args({use Type::*; [UInt]});
		}
		else if id == seat.id && opcode == seat::name {
			server.args({use Type::*; [String]});
		}
		else if id == output.id && opcode == output::geometry {
			server.args({use Type::*; [UInt, UInt, UInt, UInt, UInt, String, String, UInt]});
		}
		else if id == output.id && opcode == output::mode {
			let [_, UInt(x), UInt(y), _] = server.args({use Type::*; [UInt, UInt, UInt, UInt]}) else {panic!()};
			let _configure_bounds = xy{x,y};
		}
		else if id == output.id && opcode == output::scale {
			let [UInt(factor)] = server.args({use Type::*; [UInt]}) else {panic!()};
			scale_factor = factor;
			surface.set_buffer_scale(scale_factor);
		}
		else if id == output.id && opcode == output::name {
			server.args({use Type::*; [String]});
		}
		else if id == output.id && opcode == output::description {
			server.args({use Type::*; [String]});
		}
		else if id == output.id && opcode == output::done {
			break;
		}
		else { panic!("{:?} {opcode:?}", id); }
	}

	let ref pointer = server.new();
	seat.get_pointer(pointer);
	let ref keyboard = server.new();
	seat.get_keyboard(keyboard);

	let ref xdg_surface = server.new();
	wm_base.get_xdg_surface(xdg_surface, surface);
	let ref toplevel = server.new();
	xdg_surface.get_toplevel(toplevel);
	toplevel.set_title("App");
	surface.commit();

	let file = rustix::fs::memfd_create("target", rustix::fs::MemfdFlags::empty()).unwrap();

	let shm_pool = server.new();
	use std::os::unix::io::AsRawFd;
	rustix::fs::ftruncate(&file, 1).unwrap();
	shm.create_pool(&shm_pool, file.as_raw_fd(), 1);

	struct Pool<'t> {
		file: rustix::io::OwnedFd,
		shm_pool: ShmPool<'t>,
		buffer: Buffer<'t>,
		target: Target<'t>,
	}
	let ref mut pool = Pool{file, shm_pool, buffer: server.new(), target: Target::new(zero(), &mut [])};

	let cursor_file = rustix::fs::memfd_create("cursor", rustix::fs::MemfdFlags::empty()).unwrap(); // Needs to stay open until received
	let ref mut cursor = {
		let surface = server.new();
		//compositor.create_surface(&surface);
		let size = xy{x: 32*scale_factor, y: 32*scale_factor};
		let length = (size.y*size.x*4) as usize;
		assert!(length%4096==0);
		rustix::fs::ftruncate(&cursor_file, length as u64).unwrap();
		//let ref shm_pool = server.new();
		//shm.create_pool(shm_pool, cursor_file.as_raw_fd(), length as u32);
		let buffer = server.new();
		//shm_pool.create_buffer(&buffer, 0, size.x, size.y, size.x*4, shm_pool::Format::argb8888);
		let mmap = unsafe{
			use rustix::mm;
			std::slice::from_raw_parts_mut(
					mm::mmap(std::ptr::null_mut(), length, mm::ProtFlags::READ | mm::ProtFlags::WRITE, mm::MapFlags::SHARED, &cursor_file, 0).unwrap() as *mut u8,
					length
			)
		};
		let target = Target::new(size, bytemuck::cast_slice_mut(mmap));
		Cursor{pointer, surface, buffer, target, serial: 0}
	};

	#[throws] fn paint(pool: &mut Pool, size: size, widget: &mut dyn Widget, surface: &Surface) {
		if pool.target.size != size {
			let length = (size.y*size.x*4) as usize;
			rustix::fs::ftruncate(&pool.file, length as u64).unwrap();
			pool.shm_pool.resize(length as u32);
			if !pool.target.size.is_zero() { pool.buffer.destroy(); }
			pool.shm_pool.create_buffer(&pool.buffer, 0, size.x, size.y, size.x*4, shm_pool::Format::xrgb8888);

			let mmap = unsafe{std::slice::from_raw_parts_mut(
				rustix::mm::mmap(std::ptr::null_mut(), length, rustix::mm::ProtFlags::READ | rustix::mm::ProtFlags::WRITE, rustix::mm::MapFlags::SHARED, &pool.file, 0).unwrap() as *mut u8,
				length)};
			pool.target = Target::new(size, bytemuck::cast_slice_mut(mmap));
		}
		pool.target.fill(bgra{b:0, g:0, r:0, a:0xFF});
		//target.fill(bgra{b:0xFF, g:0xFF, r:0xFF, a:0xFF});
		let size = pool.target.size;
		widget.paint(&mut pool.target, size, zero())?;
		surface.attach(&pool.buffer,0,0);
		surface.damage_buffer(0, 0, pool.target.size.x, pool.target.size.y);
		surface.commit();
	}

	let mut size = zero();
	let mut modifiers_state = ModifiersState::default();
	let mut pointer_position = int2::default();

	loop {
		let Message{id, opcode, ..} = message(&mut*server.server.borrow_mut());
		println!("{id} {opcode}");
		use Arg::*;
		/**/ if id == display.id && opcode == display::error {
			panic!("{:?}", server.args({use Type::*; [UInt, UInt, String]}));
		}
		else if id == display.id && opcode == display::delete_id {
			let [UInt(id)] = server.args({use Type::*; [UInt]}) else {panic!()};
			assert!(id == pool.buffer.id); // Reused immediately
		}
		else if id == toplevel.id && opcode == toplevel::configure_bounds {
			server.args({use Type::*; [UInt,UInt]});
			//configure_bounds=xy{x:width as u32, y:height as u32};
			dbg!()
		}
		else if id == toplevel.id && opcode == toplevel::configure {
			let [UInt(x),UInt(y),_] = server.args({use Type::*; [UInt,UInt,Array]}) else {panic!()};
			size = xy{x: x*scale_factor, y: y*scale_factor};
			if size.is_zero() { size = widget.size(size); }
			//vector::component_wise_min(size, widget.size(size));
			// unscaled_size=xy{x:width as u32, y:height as u32};
		}
		else if id == xdg_surface.id && opcode == xdg_surface::configure {
			let [UInt(serial)] = server.args({use Type::*; [UInt]}) else {panic!()};
			xdg_surface.ack_configure(serial);
			paint(pool, size, widget, surface)?;
		}
		else if id == surface.id && opcode == surface::enter {
			let [UInt(_output)] = server.args({use Type::*; [UInt]}) else {panic!()};
		}
		else if id == pool.buffer.id && opcode == buffer::release {
		}
		else if id == pointer.id && opcode == pointer::enter {
			let [UInt(serial),_,_,_] = server.args({use Type::*; [UInt,UInt,UInt,UInt]}) else {panic!()};
			cursor.serial = serial;
		}
		else if id == pointer.id && opcode == pointer::leave {
			server.args({use Type::*; [UInt,UInt]});
		}
		else if id == pointer.id && opcode == pointer::motion {
			let [_,Int(x),Int(y)] = server.args({use Type::*; [UInt,Int,Int]}) else {panic!()};
			pointer_position = xy{x: x*scale_factor as i32/256,y: y*scale_factor as i32/256};
		}
		else if id == pointer.id && opcode == pointer::button {
			let [_,_,UInt(button),UInt(state)] = server.args({use Type::*; [UInt,UInt,UInt,UInt]}) else {panic!()};
			#[allow(non_upper_case_globals)] const usb_hid_buttons: [u32; 2] = [272, 111];
			let button = usb_hid_buttons.iter().position(|&b| b == button).unwrap_or_else(|| panic!("{:x}", button)) as u8;
			//if state>0 { *mouse_buttons |= 1<<button; } else { *mouse_buttons &= !(1<<button); }
			if widget.event(size, &mut EventContext{modifiers_state, cursor}, &Event::Button{position: pointer_position, button: button as u8, state: state as u8})? {
					paint(pool, size, widget, surface)?;
			}
		}
		else if id == pointer.id && opcode == pointer::axis {
			let [_,UInt(axis),Int(value)] = server.args({use Type::*; [UInt,UInt,Int]}) else {panic!()};
			if axis != 0 { continue; }
			if widget.event(size, &mut EventContext{modifiers_state, cursor}, &Event::Scroll(value*scale_factor as i32/256))? {
				paint(pool, size, widget, surface)?;
			}
		}
		else if id == pointer.id && opcode == pointer::frame {
			server.args([]);
		}
		else if id == pointer.id && opcode == pointer::axis_source {
			server.args({use Type::*; [UInt]});
		}
		else if id == pointer.id && opcode == pointer::axis_stop {
			server.args({use Type::*; [UInt,UInt]});
		}
		else if id == keyboard.id && opcode == keyboard::keymap {
			server.args({use Type::*; [UInt,UInt]});
		}
		else if id == keyboard.id && opcode == keyboard::repeat_info {
			server.args({use Type::*; [UInt,UInt]});
		}
		else if id == keyboard.id && opcode == keyboard::modifiers {
			let [_,UInt(depressed),_,_,_] = server.args({use Type::*; [UInt,UInt,UInt,UInt,UInt]}) else {panic!()};
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
		else if id == keyboard.id && opcode == keyboard::enter {
			server.args({use Type::*; [UInt,UInt,Array]});
		}
		else if id == keyboard.id && opcode == keyboard::leave {
			server.args({use Type::*; [UInt,UInt]});
		}
		else if id == wm_base.id && opcode == wm_base::ping {
			let [UInt(serial)] = server.args({use Type::*; [UInt]}) else {panic!()};
			wm_base.pong(serial);
		}
		else if id == keyboard.id && opcode == keyboard::key {
			let [_serial,_time,UInt(key),UInt(state)] = server.args({use Type::*; [UInt,UInt,UInt,UInt]}) else {panic!()};
			#[allow(non_upper_case_globals)] static usb_hid_usage_table: std::sync::LazyLock<Vec<char>> = std::sync::LazyLock::new(|| [
				&['\0','⎋','1','2','3','4','5','6','7','8','9','0','-','=','⌫','\t','q','w','e','r','t','y','u','i','o','p','{','}','\n','⌃','a','s','d','f','g','h','j','k','l',';','\'','`','⇧','\\','z','x','c','v','b','n','m',',','.','/','⇧','\0','⎇',' ','⇪'],
				&(1..=10).map(|i| (0xF700u32+i).try_into().unwrap()).collect::<Vec<_>>()[..], &['\0'; 20], &['\u{F70B}','\u{F70C}'], &['\0'; 8],
				&['⎙','⎄',' ','⇤','↑','⇞','←','→','⇥','↓','⇟','⎀','⌦','\u{F701}','🔇','🕩','🕪','⏻','=','±','⏯','🔎',',','\0','\0','¥','⌘']].concat());
			let key = usb_hid_usage_table.get(key as usize).unwrap();
			if state > 0 {
				if *key == '⎋' { break; }
				if widget.event(size, &mut EventContext{modifiers_state, cursor}, &Event::Key(*key))? {
					paint(pool, size, widget, surface)?;
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
		else if id == toplevel.id && opcode == toplevel::close {
			break;
		}
		else { panic!("{:?} {opcode:?}", id); }
	}
	let _ = cursor_file;
}
