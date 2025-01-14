use {vector::{num::{zero, IsZero}, xy}, crate::{Event, EventContext, Widget}};
#[path="wayland.rs"] mod wayland; use wayland::*;
mod drm {
	pub struct DRM(std::fs::File);
	impl DRM { pub fn new(path: &str) -> Self { Self(std::fs::OpenOptions::new().read(true).write(true).open(path).unwrap()) } }
	impl std::os::fd::AsFd for DRM { fn as_fd(&self) -> std::os::fd::BorrowedFd { self.0.as_fd() } }
	impl std::os::fd::AsRawFd for DRM { fn as_raw_fd(&self) -> std::os::fd::RawFd { self.0.as_raw_fd() } }
	impl ::drm::Device for DRM {}
	impl ::drm::control::Device for DRM {}
}
use self::drm::DRM;

pub fn run<T:Widget>(title: &str, widget: &mut T) {
	let ref server = Server::connect();
	let display = Display{server, id: 1};
	let ref registry = server.new("registry");
	display.get_registry(registry);
	let ([compositor, wm_base, seat, dmabuf, lease_device,output], []) = server.globals(registry, ["wl_compositor","xdg_wm_base","wl_seat","zwp_linux_dmabuf_v1","wp_drm_lease_device_v1","wl_output"], []);
	let ref compositor = Compositor{server, id: compositor};
	let ref wm_base = WmBase{server, id: wm_base};
	let ref seat = Seat{server, id: seat};
	let ref dmabuf = DMABuf{server, id: dmabuf};
	let ref lease_device = LeaseDevice{server, id: lease_device};
	let ref output = Output{server, id: output};

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
	let mut window = Surface::new(server, compositor, wm_base, title, None/*Some(output)*/);
	//let mut window = Surface::new(server, compositor, wm_base, title, Some(output));

	let drm = DRM::new(if std::path::Path::new("/dev/dri/card2").exists() { "/dev/dri/card2" } else { "/dev/dri/card1"});

	let ref params : dmabuf::Params = server.new("params");
	let ref buffer_ref : Buffer = server.new("buffer_ref");
	let mut buffer = [None; 3];
	let mut scale_factor = 0;
	let mut configure_bounds = zero();
	let mut size = zero();
	let modifiers_state = Default::default();

	loop {
		let mut need_paint = widget.event(size, &mut EventContext{modifiers_state}, &Event::Idle).unwrap(); // determines whether to wait for events
		// ^ could also trigger eventfd instead
		loop {
			let events = {
				use rustix::event::{PollFd,PollFlags};
				let server = &*server.server.borrow();
				let mut fds = [PollFd::new(server, PollFlags::IN)];
				rustix::event::poll(&mut fds, if window.can_paint && window.done && need_paint {0} else {-1}).unwrap();
				let events = fds.map(|fd| fd.revents().contains(PollFlags::IN));
				events
			};
			if events[0] {
				if let Some((Message{id, opcode, ..}, _any_fd)) = message(&*server.server.borrow()) {
					use Arg::*;
					/**/ if id == registry.id && opcode == registry::global {
						server.args({use Type::*; [UInt, String, UInt]});
					} else if id == display.id && opcode == display::error {
						let [UInt(id),UInt(code),String(message)] = server.args({use Type::*; [UInt, UInt, String]}) else {unreachable!()};
						panic!("{id} {code} {message} {:?}", server.names.lock()/*.iter().find(|(e,_)| *e==id).map(|(_,name)| name)*/);
					}
					else if id == display.id && opcode == display::delete_id {
						let [UInt(id)] = server.args({use Type::*; [UInt]}) else {unreachable!()};
						if window.callback.as_ref().is_some_and(|callback| id == callback.id) {
							window.done = true; // O_o
							window.callback = None;
							if server.last_id.load(std::sync::atomic::Ordering::SeqCst) == id+1 { server.last_id.store(id, std::sync::atomic::Ordering::SeqCst); }
						}
						else { // Reused immediately
							assert!(id == params.id || id == buffer_ref.id, "{id}");
						}
					}
					else if id == dmabuf.id && opcode == dmabuf::format {
						let [UInt(_)] = server.args({use Type::*; [UInt]}) else {unreachable!()};
					}
					else if id == dmabuf.id && opcode == dmabuf::modifier {
						let [UInt(_)] = server.args({use Type::*; [UInt]}) else {unreachable!()};
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
						window.surface.set_buffer_scale(scale_factor);
					}
					else if id == output.id && opcode == output::name {
						server.args({use Type::*; [String]});
					}
					else if id == output.id && opcode == output::description {
						server.args({use Type::*; [String]});
					}
					else if id == output.id && opcode == output::done {
					}
					else if id == window.toplevel.id && opcode == toplevel::wm_capabilities {
						let [Array(_)] = server.args({use Type::*; [Array]}) else {unreachable!()};
						println!("top_level::configure_bounds");
					}
					else if id == window.toplevel.id && opcode == toplevel::configure_bounds {
						let [UInt(_width),UInt(_height)] = server.args({use Type::*; [UInt,UInt]}) else {unreachable!()};
						println!("top_level::configure_bounds");
					}
					else if id == window.toplevel.id && opcode == toplevel::configure {
						let [UInt(x),UInt(y),Array(_)] = server.args({use Type::*; [UInt,UInt,Array]}) else {unreachable!()};
						size = xy{x: x*scale_factor, y: y*scale_factor};
						if size.is_zero() {
							assert!(configure_bounds.x > 0 && configure_bounds.y > 0);
							size = widget.size(configure_bounds).map(|x| x.next_multiple_of(scale_factor));
						}
						assert!(size.x > 0 && size.y > 0, "{:?}", xy{x: x*scale_factor, y: y*scale_factor});
						println!("top_level::configure");
					}
					else if id == window.xdg_surface.id && opcode == xdg_surface::configure {
						let [UInt(serial)] = server.args({use Type::*; [UInt]}) else {unreachable!()};
						window.xdg_surface.ack_configure(serial);
						window.can_paint = true;
						need_paint = true;
						println!("xdg_surface::configure");
					}
					else if id == window.surface.id && opcode == surface::enter {
						let [UInt(_output)] = server.args({use Type::*; [UInt]}) else {unreachable!()};
					}
					else if id == window.surface.id && opcode == surface::leave {
						let [UInt(_output)] = server.args({use Type::*; [UInt]}) else {unreachable!()};
					}
					else if id == buffer_ref.id && opcode == buffer::release {}
					else if id == pointer.id && opcode == pointer::enter {
						let [UInt(_),_,_,_] = server.args({use Type::*; [UInt,UInt,UInt,UInt]}) else {unreachable!()};
					}
					else if id == pointer.id && opcode == pointer::leave {
						server.args({use Type::*; [UInt,UInt]});
					}
					else if id == pointer.id && opcode == pointer::motion {
						let [_,Int(_),Int(_)] = server.args({use Type::*; [UInt,Int,Int]}) else {unreachable!()};
					}
					else if id == pointer.id && opcode == pointer::button {
						let [_,_,UInt(_),UInt(_)] = server.args({use Type::*; [UInt,UInt,UInt,UInt]}) else {unreachable!()};
					}
					else if id == pointer.id && opcode == pointer::axis {
						let [_,UInt(_),Int(_)] = server.args({use Type::*; [UInt,UInt,Int]}) else {unreachable!()};
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
						let [_,UInt(_),_,_,_] = server.args({use Type::*; [UInt,UInt,UInt,UInt,UInt]}) else {unreachable!()};
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
						let [_serial,UInt(_key_time),UInt(key),UInt(_state)] = server.args({use Type::*; [UInt,UInt,UInt,UInt]}) else {unreachable!()};
						if key == 1 { return; }
					}
					else if window.callback.as_ref().is_some_and(|callback| id == callback.id) && opcode == callback::done {
						let [UInt(_timestamp_ms)] = server.args({use Type::*; [UInt]}) else {unreachable!()};
						window.done = true;
					}
					else if id == window.surface.id && opcode == surface::enter {
						let [UInt(_output)] = server.args({use Type::*; [UInt]}) else {unreachable!()};
					}
					else if id == window.surface.id && opcode == surface::leave {
						let [UInt(_output)] = server.args({use Type::*; [UInt]}) else {unreachable!()};
					}
					else if id == window.surface.id && opcode == toplevel::close {
						return;
					}
					else if id == lease_device.id && opcode == drm_lease_device::drm_fd {
					}
					else if id == lease_device.id && opcode == drm_lease_device::done {
					}
					else if id == lease_device.id && opcode == drm_lease_device::released {
					}
					else { println!("{:?} {opcode:?} {:?} {:?}", id, [registry.id, keyboard.id, pointer.id, seat.id, display.id], server.names); }
				} else { println!("No messages :("); }
			} else {
				break;
			}
		} // event loop
		println!("{need_paint} {size}");
		if need_paint && size.x > 0 && size.y > 0 {
			use ::drm::{control::Device as _, buffer::Buffer as _};
			buffer.rotate_left(1);
			let ref mut buffer = buffer[0];
			if buffer.is_some_and(|buffer: ::drm::control::dumbbuffer::DumbBuffer| {let (x, y) = buffer.size(); xy{x, y} != size}) { *buffer = None; }
			let mut buffer = buffer.get_or_insert_with(|| {
				widget.event(size, &mut EventContext{modifiers_state}, &Event::Stale).unwrap();
				let mut buffer = drm.create_dumb_buffer(size.into(), if false { ::drm::buffer::DrmFourcc::Xrgb8888 } else { ::drm::buffer::DrmFourcc::Xrgb2101010 }, 32).unwrap();
				let stride = {assert_eq!(buffer.pitch()%4, 0); buffer.pitch()/4};
				if false {
					let mut map = drm.map_dumb_buffer(&mut buffer).unwrap();
					image::fill(&mut image::Image::<& mut [u32]>::cast_slice_mut(map.as_mut(), size, stride), image::bgr8::from(crate::background()).into());
				}
				buffer
			});
			{
				let stride = {assert_eq!(buffer.pitch()%4, 0); buffer.pitch()/4};
				let mut map = drm.map_dumb_buffer(&mut buffer).unwrap();
				assert!(stride * size.y <= map.as_mut().len() as u32, "{} {}", stride * size.y, map.as_mut().len());
				let mut target = image::Image::cast_slice_mut(map.as_mut(), size, stride);
				widget.paint(&mut target, size, zero()).unwrap();
			}
			dmabuf.create_params(params);
			let modifiers = 0u64;
			params.add(drm.buffer_to_prime_fd(buffer.handle(), 0).unwrap(), 0, 0, buffer.pitch(), (modifiers>>32) as u32, modifiers as u32);
			params.create_immed(buffer_ref, buffer.size().0, buffer.size().1, buffer.format() as u32, 0);
			params.destroy();
			window.surface.attach(&buffer_ref,0,0);
			buffer_ref.destroy();
			window.surface.damage_buffer(0, 0, buffer.size().0, buffer.size().1);
			window.done = false;
			let callback = window.callback.get_or_insert_with(|| server.new("callback"));
			window.surface.frame(&callback);
			window.surface.commit();
			println!("commit");
		}
	} // {idle; event; draw;} loop
}
