#[path="wayland.rs"] mod wayland;
use {num::{zero,IsZero}, vector::{xy, size, int2}, image::bgra, crate::{prelude::*, widget::{Target, Widget, EventContext, ModifiersState, Event}}, wayland::*};

struct Pool<'t> {
	file: rustix::io::OwnedFd,
	shm_pool: ShmPool<'t>,
	buffer: Buffer<'t>,
	target: Target<'t>,
}

pub struct Cursor<'t> {
	name: &'static str,
	pointer: &'t Pointer<'t>,
	shm: &'t Shm<'t>,
	pool: Option<Pool<'t>>,
	compositor: &'t Compositor<'t>,
	surface: Option<Surface<'t>>,
	serial: u32,
}

impl Cursor<'_> {
	pub fn set(&mut self, name: &'static str) {
		if self.name == name { return; }
		#[cfg(feature="xcursor")] {
		let pool = self.pool.get_or_insert_with(|| {
			let size = xy{x: 64, y: 64};
			let length = (size.y*size.x*4) as usize;
			assert!(length%4096==0);
			let file = rustix::fs::memfd_create("cursor", rustix::fs::MemfdFlags::empty()).unwrap();
			rustix::fs::ftruncate(&file, length as u64).unwrap();
			let shm_pool : ShmPool = self.shm.server.new();
			self.shm.create_pool(&shm_pool, &file, length as u32);
			let buffer = shm_pool.server.new();
			shm_pool.create_buffer(&buffer, 0, size.x, size.y, size.x*4, shm_pool::Format::argb8888);
			let mmap = unsafe{
				use rustix::mm;
				std::slice::from_raw_parts_mut(
						mm::mmap(std::ptr::null_mut(), length, mm::ProtFlags::READ | mm::ProtFlags::WRITE, mm::MapFlags::SHARED, &file, 0).unwrap() as *mut u8,
						length
				)
			};
			let target = Target::new(size, bytemuck::cast_slice_mut(mmap));
			Pool{file, shm_pool, buffer, target}
		});
		let images = xcursor::parser::parse_xcursor(&std::fs::read(xcursor::CursorTheme::load("default").load_icon(name).unwrap()).unwrap()).unwrap();
		let image = images.iter().min_by_key(|image| (pool.target.size.x as i32 - image.size as i32).abs()).unwrap();
		let hot = xy{x: image.xhot, y: image.yhot};
		let image = image::Image::cast_slice(&image.pixels_argb, xy{x: image.width, y: image.height});
		assert_eq!(pool.target.size, image.size);
		pool.target.data.copy_from_slice(&image);
		let scale_factor = 3;
		let ref surface = self.surface.get_or_insert_with(|| {
			let surface: Surface = self.compositor.server.new();
			self.compositor.create_surface(&surface);
			surface.set_buffer_scale(scale_factor);
			surface
		});
		surface.attach(&pool.buffer,0,0);
        surface.commit();
		self.pointer.set_cursor(self.serial, surface, hot.x/scale_factor, hot.y/scale_factor);
	}
	#[cfg(not(feature="xcursor"))] unreachable!()
	}
}

#[throws] pub fn run(widget: &mut dyn Widget/*, idle: &mut dyn FnMut(&mut dyn Widget)->Result<bool>*/) {
	let run_start_time = std::time::Instant::now();
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
	let ref shm = Shm{server, id: shm};
	let ref compositor = Compositor{server, id: compositor};
	let ref wm_base = WmBase{server, id: wm_base};
	let ref seat = Seat{server, id: seat};
	let ref output = Output{server, id: output};

	let ref surface = server.new();
	compositor.create_surface(surface);
	let ref xdg_surface = server.new();
	wm_base.get_xdg_surface(xdg_surface, surface);
	let ref toplevel = server.new();
	xdg_surface.get_toplevel(toplevel);
	toplevel.set_title("App");
	surface.commit();
	let mut scale_factor = 0;

	let ref pointer = server.new();
	seat.get_pointer(pointer);
	let ref keyboard = server.new();
	seat.get_keyboard(keyboard);

	let file = rustix::fs::memfd_create("target", rustix::fs::MemfdFlags::empty()).unwrap();
	let shm_pool = server.new();
	rustix::fs::ftruncate(&file, 4096).unwrap();
	shm.create_pool(&shm_pool, &file, 4096);
	let ref mut pool = Pool{file, shm_pool, buffer: server.new(), target: Target::new(zero(), &mut [])};

	#[throws] fn paint(pool: &mut Pool, size: size, widget: &mut dyn Widget, surface: &Surface) {
		assert!(size.x > 0 && size.y > 0);
		if pool.target.size != size {
			let old_length = (pool.target.size.y*pool.target.size.x*4) as usize;
			let length = (size.y*size.x*4) as usize;
			if length > old_length {
				rustix::fs::ftruncate(&pool.file, length as u64).unwrap(); // It is the client's responsibility to ensure that the file is at least as big as the new pool size
				pool.shm_pool.resize(length as u32); //  This request can only be used to make the pool bigger
			}
			if !pool.target.size.is_zero() { pool.buffer.destroy(); }
			pool.shm_pool.create_buffer(&pool.buffer, 0, size.x, size.y, size.x*4, shm_pool::Format::xrgb8888);
			//println!("create_buffer {size:?}");

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

	let mut configure_bounds = zero();
	let mut size = zero();
	let mut modifiers_state = ModifiersState::default();
	let mut pointer_position = int2::default();
	let mut mouse_buttons = 0;
	let ref mut cursor = Cursor{name: "", pointer, serial: 0, shm, pool: None, compositor, surface: None};
	let mut repeat : Option<(u64, char)> = None;
	let timerfd = rustix::time::timerfd_create(rustix::time::TimerfdClockId::Realtime, rustix::time::TimerfdFlags::empty())?;

	loop {
		let events = {
			let fd = server.server.borrow();
			let ref mut fds = vec![rustix::io::PollFd::new(&*fd, rustix::io::PollFlags::IN)];
			if let Some((msec, _)) = repeat {
				rustix::time::timerfd_settime(&timerfd, rustix::time::TimerfdTimerFlags::ABSTIME,
					&rustix::time::Itimerspec{it_interval:linux_raw_sys::general::__kernel_timespec{tv_sec:0,tv_nsec:0},it_value: linux_raw_sys::general::__kernel_timespec{tv_sec:(msec/1000) as i64,tv_nsec:((msec%1000)*1000000) as i64}}
				)?;
				fds.push(rustix::io::PollFd::new(&timerfd, rustix::io::PollFlags::IN));
			}
			rustix::io::poll(fds, -1)?;
			fds.iter().map(|fd| fd.revents().contains(rustix::io::PollFlags::IN)).collect::<Box<_>>()
		};
		if events[0] {
			let Message{id, opcode, ..} = message(&mut*server.server.borrow_mut());
			use Arg::*;
			/**/ if id == display.id && opcode == display::error {
				panic!("{:?}", server.args({use Type::*; [UInt, UInt, String]}));
			}
			else if id == display.id && opcode == display::delete_id {
				let [UInt(id)] = server.args({use Type::*; [UInt]}) else {panic!()};
				assert!(id == pool.buffer.id); // Reused immediately
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
				configure_bounds = xy{x,y};
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
			}

			else if id == toplevel.id && opcode == toplevel::configure_bounds {
				let [UInt(_width),UInt(_height)] = server.args({use Type::*; [UInt,UInt]}) else {panic!()};
				//configure_bounds = xy{x:width as u32*scale_factor, y:height as u32*scale_factor};
				//panic!("{configure_bounds:?}");
			}
			else if id == toplevel.id && opcode == toplevel::configure {
				let [UInt(x),UInt(y),_] = server.args({use Type::*; [UInt,UInt,Array]}) else {panic!()};
				size = xy{x: x*scale_factor, y: y*scale_factor};
				//println!("configure {size:?}");
				if size.is_zero() { assert!(configure_bounds.x > 0 && configure_bounds.y > 0); size = widget.size(configure_bounds); }
				assert!(size.x > 0 && size.y > 0, "{:?}", xy{x: x*scale_factor, y: y*scale_factor});
				//vector::component_wise_min(size, widget.size(size));
				// unscaled_size=xy{x:width as u32, y:height as u32};
			}
			else if id == xdg_surface.id && opcode == xdg_surface::configure {
				let [UInt(serial)] = server.args({use Type::*; [UInt]}) else {panic!()};
				xdg_surface.ack_configure(serial);
				paint(pool, size, widget, surface)?;
				//eprintln!("paint {:?}", (std::time::Instant::now()-run_start_time));
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
				if widget.event(size, &mut EventContext{modifiers_state, cursor}, &Event::Motion{position: pointer_position, mouse_buttons})? {
					paint(pool, size, widget, surface)?;
				}
			}
			else if id == pointer.id && opcode == pointer::button {
				let [_,_,UInt(button),UInt(state)] = server.args({use Type::*; [UInt,UInt,UInt,UInt]}) else {panic!()};
				#[allow(non_upper_case_globals)] const usb_hid_buttons: [u32; 2] = [272, 111];
				let button = usb_hid_buttons.iter().position(|&b| b == button).unwrap_or_else(|| panic!("{:x}", button)) as u8;
				if state>0 { mouse_buttons |= 1<<button; } else { mouse_buttons &= !(1<<button); }
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
				let [_serial,UInt(_key_time),UInt(key),UInt(state)] = server.args({use Type::*; [UInt,UInt,UInt,UInt]}) else {panic!()};
				let key = [
					'\0','⎋','1','2','3','4','5','6','7','8',
					'9','0','-','=','⌫','\t','q','w','e','r',
					't','y','u','i','o','p','{','}','\n','⌃',
					'a','s','d','f','g','h','j','k','l',
					';','\'','`','⇧','\\','z','x','c','v','b',
					'n','m',',','.','/','⇧','�','⎇',' ','⇪',
					'\u{F701}','\u{F702}','\u{F703}','\u{F704}','\u{F705}','\u{F706}','\u{F707}','\u{F708}','\u{F709}','\u{F70A}',
					'�','⇳','7','8','9','-','4','5','6','+',
					'1','2','3','0','.','�','�','≷','\u{F70B}','\u{F70C}','\u{F70D}',
					'�','�','�','�','�',',','\n','⌃'/*\x1B⎈*/,'/','⎙',
					'⎇','\n','⇤','↑','⇞','←','→','⇥','↓','⇟',
					'⎀','⌦','�','🔇','🕩','🕪','⏻','=','±','⏯',
					'�',',','�','�','¥','◆','◆','⎄'][key as usize];
				if state > 0 {
					if key == '⎋' { break; }
					if widget.event(size, &mut EventContext{modifiers_state, cursor}, &Event::Key(key))? {
						paint(pool, size, widget, surface)?;
					}
					let linux_raw_sys::general::__kernel_timespec{tv_sec,tv_nsec} = rustix::time::clock_gettime(rustix::time::ClockId::Realtime);
					let base = tv_sec as u64*1000+tv_nsec as u64/1000000;
					//let time = base&0xFFFFFFFF_00000000 + key_time as u64;
					repeat = Some((base+150, key));
				} else { repeat = None; }
			}
			else if let Some(pool) = &cursor.pool && id == pool.buffer.id && opcode == buffer::release {
			}
			else if let Some(surface) = &cursor.surface && id == surface.id && opcode == surface::enter {
				let [UInt(_output)] = server.args({use Type::*; [UInt]}) else {panic!()};
			}
			else if let Some(surface) = &cursor.surface && id == surface.id && opcode == surface::leave {
				let [UInt(_output)] = server.args({use Type::*; [UInt]}) else {panic!()};
			}
			else if id == toplevel.id && opcode == toplevel::close {
				//println!("close");
				break;
			}
			else { panic!("{:?} {opcode:?}", id); }
		}
		if events.len() > 1 && events[1] && let Some((msec, key)) = repeat {
			if widget.event(size, &mut EventContext{modifiers_state, cursor}, &Event::Key(key))? {
				paint(pool, size, widget, surface)?;
			}
			repeat = Some((msec+33, key));
		}
	}
}
