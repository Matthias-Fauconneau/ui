#![allow(non_upper_case_globals)]
#[cfg(feature="drm")] mod drm {
	pub struct DRM(std::fs::File);
	impl DRM { pub fn new(path: &str) -> Self { Self(std::fs::OpenOptions::new().read(true).write(true).open(path).unwrap()) } }
	impl std::os::fd::AsFd for DRM { fn as_fd(&self) -> std::os::fd::BorrowedFd { self.0.as_fd() } }
	impl std::os::fd::AsRawFd for DRM { fn as_raw_fd(&self) -> std::os::fd::RawFd { self.0.as_raw_fd() } }
	impl ::drm::Device for DRM {}
	impl ::drm::control::Device for DRM {}
}
#[cfg(feature="wayland")] #[path="wayland.rs"] pub mod wayland;
#[cfg(feature="wayland")] use wayland::*;
use {num::zero, vector::xy, crate::{prelude::*, Event}};
#[cfg(feature="drm")] use self::drm::DRM;
use {num::IsZero, vector::int2, crate::{EventContext, ModifiersState}};

#[cfg(feature="wayland")] pub struct Cursor<'t> {
	name: &'static str,
	#[allow(dead_code)] pointer: &'t Pointer<'t>,
	//#[allow(dead_code)] dmabuf: &'t DMABuf<'t>,
	#[allow(dead_code)] compositor: &'t Compositor<'t>,
	#[allow(dead_code)] surface: Option<Surface<'t>>,
	serial: u32,
}

#[cfg(feature="wayland")]impl Cursor<'_> {
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

#[cfg(feature="wayland")] pub struct App(#[cfg(feature="rustix")]rustix::fd::OwnedFd);

#[cfg(feature="softbuffer")] pub struct App {
	window: std::rc::Rc<winit::window::Window>,
	context: softbuffer::Context,
	surface: softbuffer::Surface,
}
#[cfg(feature="softbuffer")] impl winit::application::ApplicationHandler for Option<App> {
	fn resumed(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
		let window = std::rc::Rc::new(event_loop.create_window(Default::default()).unwrap());
		let context = softbuffer::Context::new(&window.clone()).unwrap();
		*self = Some(App{surface: softbuffer::Surface::new(&context, &window.clone()).unwrap(), window});
	}
	fn suspended(&mut self, _: &winit::event_loop::ActiveEventLoop) {}
	fn window_event(&mut self, _: &winit::event_loop::ActiveEventLoop, _: winit::window::WindowId, _: winit::event::WindowEvent) {}
	fn about_to_wait(&mut self, _: &winit::event_loop::ActiveEventLoop) {}
}
impl App {
	#[cfg(feature="wayland")] pub fn new() -> Result<Self> { Ok(Self(#[cfg(feature="rustix")] rustix::event::eventfd(0, rustix::event::EventfdFlags::empty())?)) }
	#[cfg(feature="rustix")] pub fn trigger(&self) -> rustix::io::Result<()> { Ok(assert!(rustix::io::write(&self.0, &1u64.to_ne_bytes())? == 8)) }
	#[cfg(feature="softbuffer")] pub fn run<T:Widget>(&self, title: &str, widget: &mut T) -> Result {
  		let event_loop = winit::event_loop::EventLoop::new().unwrap();
    	
        /*.with_event_handler(|_, event, event_loop| {
        	//event_loop.set_control_flow(ControlFlow::Wait);
        	match event {
            //Event::WindowEvent{event: WindowEvent::Resized(size: {width, height}), ..} => surface.resize(width, height).unwrap(),
            Event::WindowEvent{event: WindowEvent::RedrawRequested, ..} => {
                let size = window.inner_size();
                let mut buffer = surface.buffer_mut().unwrap();
                buffer.present().unwrap();
            }
            Event::WindowEvent{event: WindowEvent::CloseRequested|WindowEvent::KeyboardInput{event: KeyEvent{logical_key: Key::Named(NamedKey::Escape), ..}, ..}, ..} => event_loop.exit(),
            _ => {}
        }*/
		/*//use winit::{event::{self, Event::*, WindowEvent::*, ElementState}, event_loop::{ControlFlow, EventLoop}};
		let mut event_loop = EventLoop::new();
		//let mut window = WindowBuilder::new().with_inner_size(winit::dpi::PhysicalSize::<u32>::from(<(_,_)>::from(widget.size(xy{x: 3840, y: 2160})))).with_title(title).build(&event_loop)?;
		let near_eye = event_loop.available_monitors().find(|o| o.size()==[1920,1080].into()).map(|near_eye| WindowBuilder::new().with_fullscreen(Some(Fullscreen::Borderless(Some(near_eye)))).with_title(title).build(&event_loop).unwrap()); // HACK
		let mirror = Some(WindowBuilder::new().with_title(title).build(&event_loop).unwrap());
		let mut windows = [near_eye, mirror].into_iter().filter_map(|w| w).map(|window| {
			let context = unsafe{softbuffer::Context::new(&window)}.unwrap();
			let surface = unsafe{softbuffer::Surface::new(&context, &window)}.unwrap();
			(window, context, surface)
		}).collect::<Box<_>>();
		let mut mirror : Option<Box<[u32]>> = None;
		use winit::platform::run_return::EventLoopExtRunReturn;
		event_loop.run_return(move |event, _, control_flow| match event {
			WindowEvent {event: ScaleFactorChanged{..}, window_id} | RedrawRequested(window_id) => {
				let windows_len = windows.len();
				let (ref mut window, _, surface) = windows.iter_mut().find(|(window,_,_)| window_id == window.id()).unwrap();
				let size = {let size = window.inner_size(); xy{x: size.width, y: size.height}};
				if let Some(mirror) = mirror.as_mut() {
					surface.resize(std::num::NonZeroU32::new(size.x).unwrap(), std::num::NonZeroU32::new(size.y).unwrap()).unwrap();
					let mut buffer = surface.buffer_mut().unwrap();
					//assert_eq!(buffer.stride, mirror.stride);
					if buffer.len() == mirror.len() { buffer.copy_from_slice(&*mirror); } // WORKAROUND: winit starts with wrong scale factors / buffer sizes
					buffer.present().unwrap();
				}
				mirror = None; // FIXME: generalize to >2 windows
				widget.event(size, window, &Event::Stale).unwrap();
				surface.resize(std::num::NonZeroU32::new(size.x).unwrap(), std::num::NonZeroU32::new(size.y).unwrap()).unwrap();
				let mut buffer = surface.buffer_mut().unwrap();
				let mut target = image::Image::new::<u32>(size, &mut *buffer);
				target.fill(image::bgr8::from(crate::background()).into());
				widget.paint(&mut target, size, zero()).unwrap();
				if mirror.is_none() && windows_len>1 { mirror = Some(Box::<[u32]>::from(&*buffer)) }
				buffer.present().unwrap();
			}
			WindowEvent{event: CloseRequested, ..} => *control_flow = ControlFlow::Exit,
			WindowEvent{event:KeyboardInput{input:event::KeyboardInput{virtual_keycode:Some(VirtualKeyCode::Escape), ..},..},..} => *control_flow = ControlFlow::Exit,
			MainEventsCleared => if widget.event({let size = windows[0].0.inner_size(); xy{x: size.width, y: size.height}}, &mut windows[0].0, &Event::Idle).unwrap() {
				for (window,_,_) in windows.iter() { window.request_redraw(); }
			},
			WindowEvent{event:KeyboardInput{input:event::KeyboardInput{virtual_keycode:Some(key), state:ElementState::Pressed, ..},..},..} =>
				if widget.event({let size = windows[0].0.inner_size(); xy{x: size.width, y: size.height}}, &mut windows[0].0, &Event::Key(match key {
					VirtualKeyCode::Space => ' ',
					VirtualKeyCode::Return => '\n',
					VirtualKeyCode::A => 'a',
					VirtualKeyCode::B => 'b',
					VirtualKeyCode::C => 'c',
					VirtualKeyCode::D => 'd',
					VirtualKeyCode::E => 'e',
					VirtualKeyCode::F => 'f',
					VirtualKeyCode::G => 'g',
					VirtualKeyCode::H => 'h',
					VirtualKeyCode::I => 'i',
					VirtualKeyCode::J => 'j',
					VirtualKeyCode::K => 'k',
					VirtualKeyCode::L => 'l',
					VirtualKeyCode::M => 'm',
					VirtualKeyCode::N => 'n',
					VirtualKeyCode::O => 'o',
					VirtualKeyCode::P => 'p',
					VirtualKeyCode::Q => 'q',
					VirtualKeyCode::R => 'r',
					VirtualKeyCode::S => 's',
					VirtualKeyCode::T => 't',
					VirtualKeyCode::U => 'u',
					VirtualKeyCode::V => 'v',
					VirtualKeyCode::W => 'w',
					VirtualKeyCode::X => 'x',
					VirtualKeyCode::Y => 'y',
					VirtualKeyCode::Z => 'z',
					VirtualKeyCode::F12 => '\u{F70C}',
					VirtualKeyCode::Back => '⌫',
					_ => return if false {println!("{key:?}");} else {}
				})).unwrap() { for (window,_,_) in windows.iter() { window.request_redraw(); } },
			_ => {}
		});*/
		Ok(())
	}
	#[cfg(not(feature="softbuffer"))] pub fn run<T:Widget>(&self, title: &str, widget: &mut T) -> Result {
		let ref server = Server::connect();
		let display = Display{server, id: 1};
		let ref registry = server.new("registry");
		display.get_registry(registry);
		let ([compositor, wm_base, seat, dmabuf, lease_device], [outputs]) = server.globals(registry, ["wl_compositor","xdg_wm_base","wl_seat","zwp_linux_dmabuf_v1","wp_drm_lease_device_v1"], ["wl_output"]);
		let ref compositor = Compositor{server, id: compositor};
		let ref wm_base = WmBase{server, id: wm_base};
		let ref seat = Seat{server, id: seat};
		let outputs = outputs.iter().map(|&id| Output{server, id}).collect::<Box<_>>();
		let ref dmabuf = DMABuf{server, id: dmabuf};
		let ref lease_device = LeaseDevice{server, id: lease_device};

		let ref pointer = server.new("pointer");
		seat.get_pointer(pointer);
		let ref keyboard = server.new("keyboard");
		seat.get_keyboard(keyboard);

		struct Surface<'t> {
			surface: wayland::Surface<'t>,
			xdg_surface: XdgSurface<'t>,
			toplevel: Toplevel<'t>,
			can_paint: bool,
			callback : Option<Callback<'t>>,
			done: bool,
		}
		impl<'t> Surface<'t> {
			fn new(server: &'t wayland::Server, compositor: &Compositor, wm_base: &WmBase, title: &str, fullscreen: Option<&Output>) -> Self {
				let surface = server.new("surface");
				compositor.create_surface(&surface);
				let xdg_surface = server.new("xdg_surface");
				wm_base.get_xdg_surface(&xdg_surface, &surface);
				let toplevel = server.new("toplevel");
				xdg_surface.get_toplevel(&toplevel);
				toplevel.set_title(title);
				if let Some(output) = fullscreen { toplevel.set_fullscreen(Some(output)); }
				surface.commit();
				Self{surface, xdg_surface, toplevel, can_paint: false, callback: None, done: true}
			}
		}
		let mut window = Surface::new(server, compositor, wm_base, title, Some(&outputs.last().unwrap()));

		#[cfg(feature="drm")] let drm = DRM::new(if std::path::Path::new("/dev/dri/card0").exists() { "/dev/dri/card0" } else { "/dev/dri/card1"});

		let ref params : dmabuf::Params = server.new("params");
		let ref buffer_ref : Buffer = server.new("buffer_ref");
		let timerfd = rustix::time::timerfd_create(rustix::time::TimerfdClockId::Realtime, rustix::time::TimerfdFlags::empty())?;

		#[cfg(feature="drm")] let mut buffer = [None; 3];
		let mut scale_factor = 0;
		let mut configure_bounds = zero();
		let mut size = zero();
		let mut modifiers_state = ModifiersState::default();
		let mut pointer_position = int2::default();
		let mut mouse_buttons = 0;
		let ref mut cursor = Cursor{name: "", pointer, serial: 0, /*dmabuf,*/ compositor, surface: None};
		let mut repeat : Option<(u64, char)> = None;
		let _start = std::time::Instant::now();
		let mut idle = std::time::Duration::ZERO;
		let mut _last_done_timestamp = 0;
		//let ref lease_request : LeaseRequest = server.new("lease_request");

		loop {
			let mut need_paint = widget.event(size, &mut EventContext{toplevel: &window.toplevel, modifiers_state, cursor}, &Event::Idle).unwrap(); // determines whether to wait for events
			// ^ could also trigger eventfd instead
			loop {
				let events = {
					use rustix::{event::{PollFd,PollFlags},time::{timerfd_settime,TimerfdTimerFlags,Itimerspec,Timespec}};
					let server = &*server.server.borrow();
					let ref mut fds = Vec::from([PollFd::new(&self.0, PollFlags::IN), PollFd::new(server, PollFlags::IN)]);
					if let Some((msec, _)) = repeat {
						timerfd_settime(&timerfd, TimerfdTimerFlags::ABSTIME,
							&Itimerspec{it_interval: Timespec{tv_sec:0, tv_nsec:0}, it_value: Timespec{tv_sec:(msec/1000) as i64,tv_nsec:((msec%1000)*1000000) as i64}}
						)?;
						fds.push(PollFd::new(&timerfd, PollFlags::IN));
					}
					let time = std::time::Instant::now();
					rustix::event::poll(fds, if window.can_paint && window.done && need_paint {0} else {-1})?;
					idle += time.elapsed();
					fds.iter().map(|fd| fd.revents().contains(PollFlags::IN)).collect::<Box<_>>()
				};
				if events[0] {
					assert!({let mut buf = [0; 8]; assert!(rustix::io::read(&self.0, &mut buf)? == buf.len()); let trigger_count = u64::from_ne_bytes(buf); trigger_count == 1});
					need_paint = widget.event(size, &mut EventContext{toplevel: &window.toplevel, modifiers_state, cursor}, &Event::Trigger).unwrap(); // determines whether to wait for events
				} else if events[1] {
					//println!("events[1] {}", events[1]);
					if let Some((Message{id, opcode, ..}, _any_fd)) = message(&*server.server.borrow()) {
						//println!("id {id}");
						use Arg::*;
						/**/ if id == registry.id && opcode == registry::global {
							server.args({use Type::*; [UInt, String, UInt]});
						} else if id == display.id && opcode == display::error {
							let [UInt(id),UInt(code),String(message)] = server.args({use Type::*; [UInt, UInt, String]}) else {unreachable!()};
							panic!("{id} {code} {message} {:?}", server.names.lock().unwrap()/*.iter().find(|(e,_)| *e==id).map(|(_,name)| name)*/);
						}
						else if id == display.id && opcode == display::delete_id {
							let [UInt(id)] = server.args({use Type::*; [UInt]}) else {unreachable!()};
							//println!("delete_id {id}");
							if window.callback.as_ref().is_some_and(|callback| id == callback.id) {
								window.done = true; // O_o
								window.callback = None;
								//server.last_id.compare_exchange(id, id-1, std::sync::atomic::Ordering::Relaxed, std::sync::atomic::Ordering::Relaxed).unwrap();
								if server.last_id.load(std::sync::atomic::Ordering::SeqCst) == id+1 { server.last_id.store(id, std::sync::atomic::Ordering::SeqCst); }
							}
							else { // Reused immediately
								assert!(id == params.id || id == buffer_ref.id, "{id}");
							}
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
						else if outputs.iter().any(|o| o.id == id) && opcode == output::geometry {
							server.args({use Type::*; [UInt, UInt, UInt, UInt, UInt, String, String, UInt]});
						}
						else if outputs.iter().any(|o| o.id == id) && opcode == output::mode {
							let [_, UInt(x), UInt(y), _] = server.args({use Type::*; [UInt, UInt, UInt, UInt]}) else {unreachable!()};
							configure_bounds = xy{x,y};
						}
						else if outputs.iter().any(|o| o.id == id) && opcode == output::scale {
							let [UInt(factor)] = server.args({use Type::*; [UInt]}) else {unreachable!()};
							scale_factor = factor;
							window.surface.set_buffer_scale(scale_factor);
						}
						else if outputs.iter().any(|o| o.id == id) && opcode == output::name {
							server.args({use Type::*; [String]});
						}
						else if outputs.iter().any(|o| o.id == id) && opcode == output::description {
							server.args({use Type::*; [String]});
						}
						else if outputs.iter().any(|o| o.id == id) && opcode == output::done {
						}
						else if id == window.toplevel.id && opcode == toplevel::configure_bounds {
							let [UInt(_width),UInt(_height)] = server.args({use Type::*; [UInt,UInt]}) else {unreachable!()};
						}
						else if id == window.toplevel.id && opcode == toplevel::configure {
							let [UInt(x),UInt(y),_] = server.args({use Type::*; [UInt,UInt,Array]}) else {unreachable!()};
							//buffer = None;
							size = xy{x: x*scale_factor, y: y*scale_factor};
							if size.is_zero() {
								assert!(configure_bounds.x > 0 && configure_bounds.y > 0);
								size = widget.size(configure_bounds).map(|x| x.next_multiple_of(scale_factor));
							}
							assert!(size.x > 0 && size.y > 0, "{:?}", xy{x: x*scale_factor, y: y*scale_factor});
						}
						else if id == window.xdg_surface.id && opcode == xdg_surface::configure {
							let [UInt(serial)] = server.args({use Type::*; [UInt]}) else {unreachable!()};
							window.xdg_surface.ack_configure(serial);
							window.can_paint = true;
							need_paint = true;
						}
						else if id == window.surface.id && opcode == surface::enter {
							let [UInt(_output)] = server.args({use Type::*; [UInt]}) else {unreachable!()};
						}
						else if id == window.surface.id && opcode == surface::leave {
							let [UInt(_output)] = server.args({use Type::*; [UInt]}) else {unreachable!()};
						}
						else if id == buffer_ref.id && opcode == buffer::release {}
						//else if id == pool.buffer.id && opcode == buffer::release {}
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
							if widget.event(size, &mut EventContext{toplevel: &window.toplevel, modifiers_state, cursor}, &Event::Motion{position: pointer_position, mouse_buttons})? { need_paint=true }
						}
						else if id == pointer.id && opcode == pointer::button {
							let [_,_,UInt(button),UInt(state)] = server.args({use Type::*; [UInt,UInt,UInt,UInt]}) else {unreachable!()};
							#[allow(non_upper_case_globals)] const usb_hid_buttons: [u32; 2] = [0x110, 0x111];
							let button = usb_hid_buttons.iter().position(|&b| b == button).unwrap_or_else(||{ println!("{:x}", button); usb_hid_buttons.len()}) as u8;
							if state>0 { mouse_buttons |= 1<<button; } else { mouse_buttons &= !(1<<button); }
							if widget.event(size, &mut EventContext{toplevel: &window.toplevel, modifiers_state, cursor}, &Event::Button{position: pointer_position, button: button as u8, state: state as u8})? {
								need_paint=true;
							}
						}
						else if id == pointer.id && opcode == pointer::axis {
							let [_,UInt(axis),Int(value)] = server.args({use Type::*; [UInt,UInt,Int]}) else {unreachable!()};
							if axis != 0 { continue; }
							if widget.event(size, &mut EventContext{toplevel: &window.toplevel, modifiers_state, cursor}, &Event::Scroll(value*scale_factor as i32/256))? { need_paint=true; }
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
						else if id == pointer.id && opcode == pointer::axis_value120 {
							server.args({use Type::*; [UInt]});
						} else if id == keyboard.id && opcode == keyboard::keymap {
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
							//enum Key { prog3 = 202, unknown = 240 };
							const prog3 : u32 = 202; const unknown : u32 = 240;
							if let unknown|prog3 = key {} else {
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
									if widget.event(size, &mut EventContext{toplevel: &window.toplevel, modifiers_state, cursor}, &Event::Key(key))? { need_paint=true; }
									let rustix::time::Timespec{tv_sec,tv_nsec} = rustix::time::clock_gettime(rustix::time::ClockId::Realtime);
									let base = tv_sec as u64*1000+tv_nsec as u64/1000000;
									//let time = base&0xFFFFFFFF_00000000 + key_time as u64;
									repeat = Some((base+150, key));
								} else { repeat = None; }
							}
						}
						else if window.callback.as_ref().is_some_and(|callback| id == callback.id) && opcode == callback::done {
							let [UInt(_timestamp_ms)] = server.args({use Type::*; [UInt]}) else {unreachable!()};
							//println!("{}", _timestamp_ms-_last_done_timestamp);
							_last_done_timestamp = _timestamp_ms;
							window.done = true;
							//println!("done {}", window.callback.as_ref().unwrap().id);
							//println!("done");
						}
						/*else if let Some(pool) = &cursor.pool && id == pool.buffer.id && opcode == buffer::release {
						}*/
						else if id == window.surface.id && opcode == surface::enter {
							let [UInt(_output)] = server.args({use Type::*; [UInt]}) else {unreachable!()};
						}
						else if id == window.surface.id && opcode == surface::leave {
							let [UInt(_output)] = server.args({use Type::*; [UInt]}) else {unreachable!()};
						}
						else if id == window.surface.id && opcode == toplevel::close {
							//println!("close");
							return Ok(());
						}
						else if id == lease_device.id && opcode == drm_lease_device::drm_fd {
						}
						/*else if id == lease_device.id && opcode == drm_lease_device::connector {
							println!("connector");
							let [UInt(_connector)] = server.args({use Type::*; [UInt]}) else {unreachable!()};
							lease_device.create_lease_request(lease_request);
						}*/
						else if id == lease_device.id && opcode == drm_lease_device::done {
						}
						else if id == lease_device.id && opcode == drm_lease_device::released {
						}
						else { /*println!("{:?} {opcode:?} {:?} {:?}", id, [registry.id, keyboard.id, pointer.id, seat.id, display.id], server.names);*/ }
					} else { println!("No messages :("); }
				}
				else if events.len() > 2 && events[2] {
					let (msec, key) = repeat.unwrap();
					if widget.event(size, &mut EventContext{toplevel: &window.toplevel, modifiers_state, cursor}, &Event::Key(key))? { need_paint=true; }
					repeat = Some((msec+33, key));
				}
				else { break; }
			} // event loop
			#[cfg(feature="drm")] if need_paint && size.x > 0 && size.y > 0 {
				use ::drm::{control::Device as _, buffer::Buffer as _};
				buffer.rotate_left(1);
				let ref mut buffer = buffer[0];
				if buffer.is_some_and(|buffer: ::drm::control::dumbbuffer::DumbBuffer| {let (x, y) = buffer.size(); xy{x, y} != size}) { *buffer = None; }
				//buffer = None; // Force not reusing buffer to avoid partial updates being presented (when compositor scans out while app is drawing) // FIXME TODO: proper double buffering
				let mut buffer = buffer.get_or_insert_with(|| {
					widget.event(size, &mut EventContext{toplevel: &window.toplevel, modifiers_state, cursor}, &Event::Stale).unwrap();
					let mut buffer = drm.create_dumb_buffer(size.into(), ::drm::buffer::DrmFourcc::Xrgb8888 /*drm::buffer::DrmFourcc::Xrgb2101010*/, 32).unwrap();
					#[cfg(feature="background")] {
						let stride = {assert_eq!(buffer.pitch()%4, 0); buffer.pitch()/4};
						let mut map = drm.map_dumb_buffer(&mut buffer).unwrap();
						image::fill(&mut image::Image::<& mut [u32]>::cast_slice_mut(map.as_mut(), size, stride), image::bgr8::from(crate::background()).into());
					}
					buffer
				});
				{
					let stride = {assert_eq!(buffer.pitch()%4, 0); buffer.pitch()/4};
					let mut map = drm.map_dumb_buffer(&mut buffer)?;
					assert!(stride * size.y <= map.as_mut().len() as u32, "{} {}", stride * size.y, map.as_mut().len());
					let mut target = image::Image::cast_slice_mut(map.as_mut(), size, stride);
					widget.paint(&mut target, size, zero())?;
				}
				dmabuf.create_params(params);
				let modifiers = 0u64;
				params.add(drm.buffer_to_prime_fd(buffer.handle(), 0)?, 0, 0, buffer.pitch(), (modifiers>>32) as u32, modifiers as u32);
				params.create_immed(buffer_ref, buffer.size().0, buffer.size().1, buffer.format() as u32, 0);
				params.destroy();
				window.surface.attach(&buffer_ref,0,0);
				buffer_ref.destroy();
				window.surface.damage_buffer(0, 0, buffer.size().0, buffer.size().1);
				window.done = false;
				let callback = window.callback.get_or_insert_with(|| server.new("callback"));
				window.surface.frame(&callback);
				window.surface.commit();
			}
		} // idle-event loop
	}
}
impl Default for App { fn default() -> Self { Self::new().unwrap() } }
pub fn run<T:Widget>(title: &str, widget: &mut T) -> Result {  trace::timeout(1, || App::new()?.run(title, widget)) }
