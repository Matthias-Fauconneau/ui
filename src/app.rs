#[path="wayland.rs"] mod wayland;
use {num::{zero,IsZero}, vector::{xy, int2}, /*image::bgra,*/ crate::{prelude::*, widget::{/*Target,*/ Widget, EventContext, ModifiersState, Event}}, wayland::*};

pub struct Device(std::fs::File);
impl Device { pub fn new(path: &str) -> Self { Self(std::fs::OpenOptions::new().read(true).write(true).open(path).unwrap()) } }
impl std::os::fd::AsFd for Device { fn as_fd(&self) -> std::os::fd::BorrowedFd { self.0.as_fd() } }
impl std::os::fd::AsRawFd for Device { fn as_raw_fd(&self) -> std::os::fd::RawFd { self.0.as_raw_fd() } }
impl ::drm::Device for Device {}

pub struct Cursor<'t> {
	name: &'static str,
	#[allow(dead_code)] pointer: &'t Pointer<'t>,
	#[allow(dead_code)] dmabuf: &'t DMABuf<'t>,
	#[allow(dead_code)] compositor: &'t Compositor<'t>,
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

pub fn run(widget: &mut dyn Widget, idle: &mut dyn FnMut(&mut dyn Widget)->Result<bool>) -> Result<()> {
	let server = std::os::unix::net::UnixStream::connect({
		let mut path = std::path::PathBuf::from(std::env::var_os("XDG_RUNTIME_DIR").unwrap());
		path.push(std::env::var_os("WAYLAND_DISPLAY").unwrap());
		path
	})?;
	let ref server = Server::from(server);
	let display = Display{server, id: 1};

	let ref registry = server.new();
	display.get_registry(registry);

	let [dmabuf, compositor, wm_base, seat, output] = server.globals(registry, ["zwp_linux_dmabuf_v1", "wl_compositor",  "xdg_wm_base", "wl_seat", "wl_output"]);
	let ref dmabuf = DMABuf{server, id: dmabuf};
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

	let ref params : dmabuf::Params = server.new();
	let ref buffer : Buffer = server.new();
	let mut configure_bounds = zero();
	let mut size = zero();
	let mut can_paint = false;
	let mut modifiers_state = ModifiersState::default();
	let mut pointer_position = int2::default();
	let mut mouse_buttons = 0;
	let ref mut cursor = Cursor{name: "", pointer, serial: 0, dmabuf, compositor, surface: None};
	let mut repeat : Option<(u64, char)> = None;
	let timerfd = rustix::time::timerfd_create(rustix::time::TimerfdClockId::Realtime, rustix::time::TimerfdFlags::empty())?;

	loop {
		let mut paint = idle(widget).unwrap();
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
				rustix::io::poll(fds, if paint {0} else {-1})?;
				fds.iter().map(|fd| fd.revents().contains(rustix::io::PollFlags::IN)).collect::<Box<_>>()
			};
			if events[0] {
				let Message{id, opcode, ..} = message(&mut*server.server.borrow_mut());
				use Arg::*;
				/**/ if id == display.id && opcode == display::error {
					panic!("{:?}", server.args({use Type::*; [UInt, UInt, String]}));
				}
				else if id == display.id && opcode == display::delete_id {
					let [UInt(id)] = server.args({use Type::*; [UInt]}) else {unreachable!()};
					assert!(id == params.id || id == buffer.id); // Reused immediately
				}
				else if id == dmabuf.id && opcode == dmabuf::format {
					let [UInt(format)] = server.args({use Type::*; [UInt]}) else {unreachable!()};
					println!("f {format:x}");
				}
				else if id == dmabuf.id && opcode == dmabuf::modifier {
					let [UInt(modifier)] = server.args({use Type::*; [UInt]}) else {unreachable!()};
					println!("m {modifier:x}");
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
					let [_, UInt(x), UInt(y), _] = server.args({use Type::*; [UInt, UInt, UInt, UInt]}) else {unreachable!()};
					configure_bounds = xy{x,y};
				}
				else if id == output.id && opcode == output::scale {
					let [UInt(factor)] = server.args({use Type::*; [UInt]}) else {unreachable!()};
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
					let [UInt(_width),UInt(_height)] = server.args({use Type::*; [UInt,UInt]}) else {unreachable!()};
				}
				else if id == toplevel.id && opcode == toplevel::configure {
					let [UInt(x),UInt(y),_] = server.args({use Type::*; [UInt,UInt,Array]}) else {unreachable!()};
					size = xy{x: x*scale_factor, y: y*scale_factor};
					if size.is_zero() { assert!(configure_bounds.x > 0 && configure_bounds.y > 0); size = widget.size(configure_bounds); }
					assert!(size.x > 0 && size.y > 0, "{:?}", xy{x: x*scale_factor, y: y*scale_factor});
				}
				else if id == xdg_surface.id && opcode == xdg_surface::configure {
					let [UInt(serial)] = server.args({use Type::*; [UInt]}) else {unreachable!()};
					xdg_surface.ack_configure(serial);
					can_paint = true;
					paint = true;
				}
				else if id == surface.id && opcode == surface::enter {
					let [UInt(_output)] = server.args({use Type::*; [UInt]}) else {unreachable!()};
				}
				else if id == buffer.id && opcode == buffer::release {
				}
				else if id == pointer.id && opcode == pointer::enter {
					let [UInt(serial),_,_,_] = server.args({use Type::*; [UInt,UInt,UInt,UInt]}) else {unreachable!()};
					cursor.serial = serial;
				}
				else if id == pointer.id && opcode == pointer::leave {
					server.args({use Type::*; [UInt,UInt]});
				}
				else if id == pointer.id && opcode == pointer::motion {
					let [_,Int(x),Int(y)] = server.args({use Type::*; [UInt,Int,Int]}) else {unreachable!()};
					pointer_position = xy{x: x*scale_factor as i32/256,y: y*scale_factor as i32/256};
					if widget.event(size, &mut EventContext{modifiers_state, cursor}, &Event::Motion{position: pointer_position, mouse_buttons})? { paint=true }
				}
				else if id == pointer.id && opcode == pointer::button {
					let [_,_,UInt(button),UInt(state)] = server.args({use Type::*; [UInt,UInt,UInt,UInt]}) else {unreachable!()};
					#[allow(non_upper_case_globals)] const usb_hid_buttons: [u32; 2] = [272, 111];
					let button = usb_hid_buttons.iter().position(|&b| b == button).unwrap_or_else(|| panic!("{:x}", button)) as u8;
					if state>0 { mouse_buttons |= 1<<button; } else { mouse_buttons &= !(1<<button); }
					if widget.event(size, &mut EventContext{modifiers_state, cursor}, &Event::Button{position: pointer_position, button: button as u8, state: state as u8})? { paint=true; }
				}
				else if id == pointer.id && opcode == pointer::axis {
					let [_,UInt(axis),Int(value)] = server.args({use Type::*; [UInt,UInt,Int]}) else {unreachable!()};
					if axis != 0 { continue; }
					if widget.event(size, &mut EventContext{modifiers_state, cursor}, &Event::Scroll(value*scale_factor as i32/256))? { paint=true; }
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
					let [_,UInt(depressed),_,_,_] = server.args({use Type::*; [UInt,UInt,UInt,UInt,UInt]}) else {unreachable!()};
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
					let [UInt(serial)] = server.args({use Type::*; [UInt]}) else {unreachable!()};
					wm_base.pong(serial);
				}
				else if id == keyboard.id && opcode == keyboard::key {
					let [_serial,UInt(_key_time),UInt(key),UInt(state)] = server.args({use Type::*; [UInt,UInt,UInt,UInt]}) else {unreachable!()};
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
						if key == '⎋' { return Ok(()); }
						if widget.event(size, &mut EventContext{modifiers_state, cursor}, &Event::Key(key))? { paint=true; }
						let linux_raw_sys::general::__kernel_timespec{tv_sec,tv_nsec} = rustix::time::clock_gettime(rustix::time::ClockId::Realtime);
						let base = tv_sec as u64*1000+tv_nsec as u64/1000000;
						//let time = base&0xFFFFFFFF_00000000 + key_time as u64;
						repeat = Some((base+150, key));
					} else { repeat = None; }
				}
				/*else if let Some(pool) = &cursor.pool && id == pool.buffer.id && opcode == buffer::release {
				}*/
				else if let Some(surface) = &cursor.surface && id == surface.id && opcode == surface::enter {
					let [UInt(_output)] = server.args({use Type::*; [UInt]}) else {unreachable!()};
				}
				else if let Some(surface) = &cursor.surface && id == surface.id && opcode == surface::leave {
					let [UInt(_output)] = server.args({use Type::*; [UInt]}) else {unreachable!()};
				}
				else if id == toplevel.id && opcode == toplevel::close {
					//println!("close");
					return Ok(());
				}
				else { panic!("{:?} {opcode:?}", id); }
			}
			else if events.len() > 1 && events[1] && let Some((msec, key)) = repeat {
				if widget.event(size, &mut EventContext{modifiers_state, cursor}, &Event::Key(key))? { paint=true; }
				repeat = Some((msec+33, key));
			} else {
				break;
			}
		}
		if paint && can_paint {
			assert!(size.x > 0 && size.y > 0);
			let mut target = None;
			widget.paint(&mut target, size, zero())?;
			let Some(target) = target.take() else {unreachable!()};
			dmabuf.create_params(params);
			params.add(&target.fd, 0, 0, target.size.x*4, (target.modifiers>>32) as u32, target.modifiers as u32);
			params.create_immed(buffer, target.size.x, target.size.y, target.format, 0);
			params.destroy();
			surface.attach(&buffer,0,0);
			buffer.destroy();
			surface.damage_buffer(0, 0, target.size.x, target.size.y);
			surface.commit();
		}
	}
}
