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
use {num::zero, vector::xy, crate::{prelude::*, widget::Widget, Event, EventContext}};
#[cfg(feature="drm")] use self::drm::DRM;
#[cfg(feature="wayland")] use {num::IsZero, vector::int2, wayland::*, crate::{background, ModifiersState}};

#[cfg(feature="wayland")] pub struct Cursor<'t> {
	name: &'static str,
	#[allow(dead_code)] pointer: &'t Pointer<'t>,
	//#[allow(dead_code)] dmabuf: &'t DMABuf<'t>,
	#[allow(dead_code)] compositor: &'t Compositor<'t>,
	#[allow(dead_code)] surface: Option<Surface<'t>>,
	serial: u32,
}

#[cfg(feature="wayland")] impl Cursor<'_> {
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

pub struct App(#[cfg(feature="rustix")] rustix::fd::OwnedFd);
impl App {
	pub fn new() -> Result<Self> { Ok(Self(#[cfg(feature="rustix")] rustix::event::eventfd(0, rustix::event::EventfdFlags::empty())?)) }
	#[cfg(feature="rustix")] pub fn trigger(&self) -> rustix::io::Result<()> { Ok(assert!(rustix::io::write(&self.0, &1u64.to_ne_bytes())? == 8)) }
	#[cfg(feature="wayland")] pub fn run<T:Widget>(&self, title: &str, widget: &mut T) -> Result {
		let ref server = Server::connect();
		let display = Display{server, id: 1};
		let ref registry = server.new("registry");
		display.get_registry(registry);

		let [compositor, wm_base, seat, output, dmabuf/*shm*/] = server.globals(registry, ["wl_compositor","xdg_wm_base","wl_seat","wl_output","zwp_linux_dmabuf_v1"/*"wl_shm"*/]);
		let ref compositor = Compositor{server, id: compositor};
		let ref wm_base = WmBase{server, id: wm_base};
		let ref seat = Seat{server, id: seat};
		let ref output = Output{server, id: output};
		let ref dmabuf = DMABuf{server, id: dmabuf};
		/*let ref shm = Shm{server, id: shm};
		struct Pool<'t> {
			file: rustix::fd::OwnedFd,
			shm_pool: ShmPool<'t>,
			buffer: Buffer<'t>,
			target: Target<'t>,
		}
		let file = rustix::fs::memfd_create("target", rustix::fs::MemfdFlags::empty()).unwrap();
		let shm_pool = server.new();
		rustix::fs::ftruncate(&file, 4096).unwrap();
		shm.create_pool(&shm_pool, &file, 4096);
		let ref mut pool = Pool{file, shm_pool, buffer: server.new(), target: Target::new(zero(), &mut [])};*/

		let ref pointer = server.new("pointer");
		seat.get_pointer(pointer);
		let ref keyboard = server.new("keyboard");
		seat.get_keyboard(keyboard);

		let ref surface = server.new("surface");
		compositor.create_surface(surface);
		let ref xdg_surface = server.new("xdg_surface");
		wm_base.get_xdg_surface(xdg_surface, surface);
		let ref toplevel = server.new("toplevel");
		xdg_surface.get_toplevel(toplevel);
		toplevel.set_title(title);
		surface.commit();

		let drm = DRM::new(if std::path::Path::new("/dev/dri/card0").exists() { "/dev/dri/card0" } else { "/dev/dri/card1"}); // or use Vulkan ext acquire drm display

		let ref params : dmabuf::Params = server.new("params");
		let ref buffer_ref : Buffer = server.new("buffer_ref");
		let timerfd = rustix::time::timerfd_create(rustix::time::TimerfdClockId::Realtime, rustix::time::TimerfdFlags::empty())?;

		let mut buffer = None;
		let mut scale_factor = 0;
		let mut configure_bounds = zero();
		let mut size = zero();
		let mut can_paint = false;
		let mut modifiers_state = ModifiersState::default();
		let mut pointer_position = int2::default();
		let mut mouse_buttons = 0;
		let ref mut cursor = Cursor{name: "", pointer, serial: 0, /*dmabuf,*/ compositor, surface: None};
		let mut repeat : Option<(u64, char)> = None;
		let mut callback : Option<Callback> = None;
		let mut done = true;
		let _start = std::time::Instant::now();
		let mut idle = std::time::Duration::ZERO;
		let mut _last_done_timestamp = 0;

		/*use {std::default::default, ash::{vk, extensions::ext::DebugUtils}};
		let entry = ash::Entry::linked();
		let ref name = std::ffi::CString::new(title)?;
		let instance = unsafe{entry.create_instance(&vk::InstanceCreateInfo::builder()
			.application_info(&vk::ApplicationInfo::builder().api_version(vk::make_api_version(0, 1, 5, 0)).application_name(name).application_version(0).engine_name(name))
			.enabled_layer_names(&[std::ffi::CStr::from_bytes_with_nul(b"VK_LAYER_KHRONOS_validation\0")?.as_ptr()])
			.enabled_extension_names(&[DebugUtils::name().as_ptr()])
			.push_next(&mut vk::ValidationFeaturesEXT::builder().enabled_validation_features(&[vk::ValidationFeatureEnableEXT::DEBUG_PRINTF]))
			, None)}?;
		let debug_utils = DebugUtils::new(&entry, &instance);
		unsafe extern "system" fn vulkan_debug_callback(severity: vk::DebugUtilsMessageSeverityFlagsEXT, r#type: vk::DebugUtilsMessageTypeFlagsEXT, data: *const vk::DebugUtilsMessengerCallbackDataEXT, _user_data: *mut std::os::raw::c_void) -> vk::Bool32 {
			let data = *data;
			if severity == vk::DebugUtilsMessageSeverityFlagsEXT::INFO &&
					r#type == vk::DebugUtilsMessageTypeFlagsEXT::VALIDATION &&
					std::ffi::CStr::from_ptr(data.p_message_id_name).to_str().unwrap() == "UNASSIGNED-DEBUG-PRINTF"
			{
				let message = std::ffi::CStr::from_ptr(data.p_message).to_str().unwrap();
				let [_, _, message]:[&str;3] = *{let t:Box<_> = message.split(" | ").collect::<Box<_>>().try_into().unwrap(); t};
				if let Some(message) = message.strip_suffix("nan") { panic!("{message}"); }
				println!("{message}");
			} else /*if severity != vk::DebugUtilsMessageSeverityFlagsEXT::VERBOSE && severity != vk::DebugUtilsMessageSeverityFlagsEXT::INFO*/ {
				println!("{:?} {:?} [{:?}] : {:?}", severity, r#type, Some(data.p_message_id_name).filter(|p| !p.is_null()).map(|p| std::ffi::CStr::from_ptr(p)).unwrap_or_default(), Some(data.p_message).filter(|p| !p.is_null()).map(|p| std::ffi::CStr::from_ptr(p)).unwrap_or_default() );
			}
			vk::FALSE
		}
		let _debug_utils_messenger = unsafe{debug_utils.create_debug_utils_messenger(&vk::DebugUtilsMessengerCreateInfoEXT{
			message_severity: vk::DebugUtilsMessageSeverityFlagsEXT::ERROR|vk::DebugUtilsMessageSeverityFlagsEXT::WARNING|vk::DebugUtilsMessageSeverityFlagsEXT::INFO|vk::DebugUtilsMessageSeverityFlagsEXT::VERBOSE,
			message_type: vk::DebugUtilsMessageTypeFlagsEXT::GENERAL|vk::DebugUtilsMessageTypeFlagsEXT::VALIDATION|vk::DebugUtilsMessageTypeFlagsEXT::PERFORMANCE, pfn_user_callback: Some(vulkan_debug_callback), ..default()}, None)?};*/

		/*let device = unsafe{instance.enumerate_physical_devices()?.into_iter().find(|device| instance.get_physical_device_properties(*device).device_type == vk::PhysicalDeviceType::INTEGRATED_GPU)}.unwrap();
		let queue_family_index = unsafe{instance.get_physical_device_queue_family_properties(device)}.iter().position(|p| p.queue_flags.contains(vk::QueueFlags::GRAPHICS)).unwrap() as u32;*/
		/*let device = unsafe{instance.create_device(device, &vk::DeviceCreateInfo::builder()
			.queue_create_infos(&[vk::DeviceQueueCreateInfo::builder().queue_family_index(queue_family_index).queue_priorities(&[1.]).build()])
			/*.enabled_extension_names(&[Swapchain::name().as_ptr()])*/, None)}?;*/
		/*let queue = unsafe{device.get_device_queue(queue_family_index, 0)};
		let command_pool = unsafe{device.create_command_pool(&vk::CommandPoolCreateInfo{flags: vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER, queue_family_index, ..default()}, None)}?;*/

		/*let swapchain = Swapchain::new(&instance, &device);
		let swapchain_create_info = vk::SwapchainCreateInfoKHR::default()
			.surface(surface)
			.min_image_count(desired_image_count)
			.image_color_space(surface_format.color_space)
			.image_format(surface_format.format)
			.image_extent(surface_resolution)
			.image_usage(vk::ImageUsageFlags::COLOR_ATTACHMENT)
			.image_sharing_mode(vk::SharingMode::EXCLUSIVE)
			.pre_transform(pre_transform)
			.composite_alpha(vk::CompositeAlphaFlagsKHR::OPAQUE)
			.present_mode(present_mode)
			.clipped(true)
			.image_array_layers(1);

		let swapchain = swapchain_loader
			.create_swapchain(&swapchain_create_info, None)
			.unwrap();*/

		loop {
			let mut paint = widget.event(size, &mut Some(EventContext{toplevel, modifiers_state, cursor}), &Event::Idle).unwrap(); // determines whether to wait for events
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
					//println!("{:.0}%", (1.-idle.div_duration_f32(start.elapsed()))*100.);
					let time = std::time::Instant::now();
					rustix::event::poll(fds, if can_paint && done && paint {0} else {-1})?;
					idle += time.elapsed();
					fds.iter().map(|fd| fd.revents().contains(PollFlags::IN)).collect::<Box<_>>()
				};
				if events[0] {
					assert!({let mut buf = [0; 8]; assert!(rustix::io::read(&self.0, &mut buf)? == buf.len()); let trigger_count = u64::from_ne_bytes(buf); trigger_count == 1});
					paint = widget.event(size, &mut Some(EventContext{toplevel, modifiers_state, cursor}), &Event::Trigger).unwrap(); // determines whether to wait for events
				} else if events[1] {
					let Message{id, opcode, ..} = message(&*server.server.borrow());
					//println!("{id} {opcode} {:?}",[toplevel.id, surface.id, keyboard.id, pointer.id, output.id, seat.id, display.id, dmabuf/*shm*/.id]);
					use Arg::*;
					/**/ if id == registry.id && opcode == registry::global {
						server.args({use Type::*; [UInt, String, UInt]});
					} else if id == display.id && opcode == display::error {
						panic!("{:?}", server.args({use Type::*; [UInt, UInt, String]}));
					}
					else if id == display.id && opcode == display::delete_id {
						let [UInt(id)] = server.args({use Type::*; [UInt]}) else {unreachable!()};
						if callback.as_ref().is_some_and(|callback| id == callback.id) { /*println!("delete_id callback {}", callback.unwrap().id);*/ callback = None; } // Cannot reuse same id... :(
						else { // Reused immediately
							assert!(id == params.id || id == buffer_ref.id, "{id}");
							//assert!(id == pool.buffer.id); // Reused immediately
						}
						//println!("delete_id {}", id);
					}
					else if id == dmabuf.id && opcode == dmabuf::format {
						let [UInt(format)] = server.args({use Type::*; [UInt]}) else {unreachable!()};
						println!("f {format:x}");
					}
					else if id == dmabuf.id && opcode == dmabuf::modifier {
						let [UInt(modifier)] = server.args({use Type::*; [UInt]}) else {unreachable!()};
						println!("m {modifier:x}");
					}
					/*else if id == shm.id && opcode == shm::format {
						server.args({use Type::*; [UInt]});
					}*/ else if id == seat.id && opcode == seat::capabilities {
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
						//buffer = None;
						size = xy{x: x*scale_factor, y: y*scale_factor};
						if size.is_zero() {
							assert!(configure_bounds.x > 0 && configure_bounds.y > 0);
							size = widget.size(configure_bounds);
							#[cfg(feature="int_roundings")] { size = size.map(|x| x.next_multiple_of(3)); }
							#[cfg(not(feature="int_roundings"))] { size = size.map(|x| x+2/3 ); }
							assert!(size.x % scale_factor == 0 && size.y % scale_factor == 0);
						}
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
					else if id == surface.id && opcode == surface::leave {
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
						if widget.event(size, &mut Some(EventContext{toplevel, modifiers_state, cursor}), &Event::Motion{position: pointer_position, mouse_buttons})? { paint=true }
					}
					else if id == pointer.id && opcode == pointer::button {
						let [_,_,UInt(button),UInt(state)] = server.args({use Type::*; [UInt,UInt,UInt,UInt]}) else {unreachable!()};
						#[allow(non_upper_case_globals)] const usb_hid_buttons: [u32; 2] = [272, 111];
						let button = usb_hid_buttons.iter().position(|&b| b == button).unwrap_or_else(|| panic!("{:x}", button)) as u8;
						if state>0 { mouse_buttons |= 1<<button; } else { mouse_buttons &= !(1<<button); }
						if widget.event(size, &mut Some(EventContext{toplevel, modifiers_state, cursor}), &Event::Button{position: pointer_position, button: button as u8, state: state as u8})? {
							paint=true;
						}
					}
					else if id == pointer.id && opcode == pointer::axis {
						let [_,UInt(axis),Int(value)] = server.args({use Type::*; [UInt,UInt,Int]}) else {unreachable!()};
						if axis != 0 { continue; }
						if widget.event(size, &mut Some(EventContext{toplevel, modifiers_state, cursor}), &Event::Scroll(value*scale_factor as i32/256))? { paint=true; }
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
								if widget.event(size, &mut Some(EventContext{toplevel, modifiers_state, cursor}), &Event::Key(key))? { paint=true; }
								let rustix::time::Timespec{tv_sec,tv_nsec} = rustix::time::clock_gettime(rustix::time::ClockId::Realtime);
								let base = tv_sec as u64*1000+tv_nsec as u64/1000000;
								//let time = base&0xFFFFFFFF_00000000 + key_time as u64;
								repeat = Some((base+150, key));
							} else { repeat = None; }
						}
					}
					else if callback.as_ref().is_some_and(|callback| id == callback.id) && opcode == callback::done {
						let [UInt(_timestamp_ms)] = server.args({use Type::*; [UInt]}) else {unreachable!()};
						//println!("{}", _timestamp_ms-last_done_timestamp);
						_last_done_timestamp = _timestamp_ms;
						done = true;
						//println!("done {}", callback.id);
						println!("done");
					}
					/*else if let Some(pool) = &cursor.pool && id == pool.buffer.id && opcode == buffer::release {
					}*/
					else if id == surface.id && opcode == surface::enter {
						let [UInt(_output)] = server.args({use Type::*; [UInt]}) else {unreachable!()};
					}
					else if id == surface.id && opcode == surface::leave {
						let [UInt(_output)] = server.args({use Type::*; [UInt]}) else {unreachable!()};
					}
					else if id == toplevel.id && opcode == toplevel::close {
						//println!("close");
						return Ok(());
					}
					else { panic!("{:?} {opcode:?} {:?} {:?}", id, [registry.id, toplevel.id, surface.id, keyboard.id, pointer.id, output.id, seat.id, display.id/*, dmabuf.id*/], server.names); }
				}
				else if events.len() > 2 && events[2] {
					let (msec, key) = repeat.unwrap();
					if widget.event(size, &mut Some(EventContext{toplevel, modifiers_state, cursor}), &Event::Key(key))? { paint=true; }
					repeat = Some((msec+33, key));
				}
				else { break; }
			} // event loop
			println!("{paint} {can_paint} {done}");
			if paint && can_paint && done {
				//let time = std::time::Instant::now();
				assert!(size.x > 0 && size.y > 0);
				use ::drm::{control::Device as _, buffer::Buffer as _};
				if buffer.is_some_and(|buffer: ::drm::control::dumbbuffer::DumbBuffer| {let (x, y) = buffer.size(); xy{x, y} != size}) { buffer = None; }
				let mut buffer = buffer.get_or_insert_with(|| {
					widget.event(size, &mut Some(EventContext{toplevel, modifiers_state, cursor}), &Event::Stale).unwrap();
					let mut buffer = drm.create_dumb_buffer(size.into(), ::drm::buffer::DrmFourcc::Xrgb8888 /*drm::buffer::DrmFourcc::Xrgb2101010*/, 32).unwrap();
					{
						let stride = {assert_eq!(buffer.pitch()%4, 0); buffer.pitch()/4};
						let mut map = drm.map_dumb_buffer(&mut buffer).unwrap();
						image::Image::<& mut [u32]>::cast_slice_mut(map.as_mut(), size, stride).fill(image::bgr8::from(background()).into());
					}
					buffer
				});
				/*if pool.target.size != size {
					let old_length = (pool.target.size.y*pool.target.size.x*4) as usize;
					let length = (size.y*size.x*4) as usize;
					if length > old_length {
							rustix::fs::ftruncate(&pool.file, length as u64).unwrap();
							pool.shm_pool.resize(length as u32); //  This request can only be used to make the pool bigger
					}
					if !pool.target.size.is_zero() { pool.buffer.destroy(); }
					pool.shm_pool.create_buffer(&pool.buffer, 0, size.x, size.y, size.x*4, shm_pool::Format::xrgb8888);
					//println!("create_buffer {size:?}");
					use rustix::mm;
					let mmap = unsafe{std::slice::from_raw_parts_mut(
							mm::mmap(std::ptr::null_mut(), length, mm::ProtFlags::READ | mm::ProtFlags::WRITE, mm::MapFlags::SHARED, &pool.file, 0).unwrap() as *mut u8,
							length)};
					pool.target = Target::new(size, bytemuck::cast_slice_mut(mmap));
				}*/
				/*{
					use vk::*;
					//VkImageDrmFormatModifierExplicitCreateInfoEXT and VkExternalMemoryImageCreateInfo onto VkImageCreateInfo
					/*VkImage vulkan_import_dmabuf(wlr_dmabuf_attributes *attribs) -> VkDeviceMemory mems[n_mems]:
					vulkan_format_props_from_drm(renderer->dev, attribs->format);
					uint32_t plane_count = attribs->n_planes;
					assert(plane_count < WLR_DMABUF_MAX_PLANES);
					const struct wlr_vk_format_modifier_props *mod = vulkan_format_props_find_modifier(fmt, attribs->modifier, for_render);
					VkExternalMemoryHandleTypeFlagBits htype = VK_EXTERNAL_MEMORY_HANDLE_TYPE_DMA_BUF_BIT_EXT;*/
					let image = unsafe{device.create_image(&ImageCreateInfo{
						image_type: ImageType::TYPE_2D,
						format: Format::A2R10G10B10_UNORM_PACK32,
						mip_levels: 1, array_layers: 1, samples: SampleCountFlags::TYPE_1,
						sharing_mode: SharingMode::EXCLUSIVE,
						initial_layout: ImageLayout::UNDEFINED,
						extent: Extent3D{width: size.x, height: size.y, depth: 1},
						usage: ImageUsageFlags::COLOR_ATTACHMENT|ImageUsageFlags::TRANSFER_SRC|ImageUsageFlags::SAMPLED,
						p_next: &ExternalMemoryImageCreateInfo{handle_types: ExternalMemoryHandleTypeFlags::DMA_BUF_EXT,
							p_next: &ImageDrmFormatModifierExplicitCreateInfoEXT{
								drm_format_modifier_plane_count: 1, drm_format_modifier: 0, p_plane_layouts: &SubresourceLayout{offset: 0, row_pitch: buffer.pitch() as u64,
								size: 0, ..default()} as *const _, ..default()} as *const ImageDrmFormatModifierExplicitCreateInfoEXT as *const _, ..default()}
								 as *const ExternalMemoryImageCreateInfo as *const _, ..default()}, None)}?;
					panic!("{:?}", image);
					/*unsigned mem_count = disjoint ? plane_count : 1u;
					VkBindImageMemoryInfo bindi[WLR_DMABUF_MAX_PLANES] = {0};
					VkBindImagePlaneMemoryInfo planei[WLR_DMABUF_MAX_PLANES] = {0};

					for (unsigned i = 0u; i < mem_count; ++i) {
						VkMemoryFdPropertiesKHR fdp = {
							.sType = VK_STRUCTURE_TYPE_MEMORY_FD_PROPERTIES_KHR,
						};
						res = renderer->dev->api.getMemoryFdPropertiesKHR(dev, htype,
							attribs->fd[i], &fdp);
						if (res != VK_SUCCESS) {
							wlr_vk_error("getMemoryFdPropertiesKHR", res);
							goto error_image;
						}

						VkImageMemoryRequirementsInfo2 memri = {
							.image = image,
							.sType = VK_STRUCTURE_TYPE_IMAGE_MEMORY_REQUIREMENTS_INFO_2,
						};

						VkImagePlaneMemoryRequirementsInfo planeri;
						if (disjoint) {
							planeri = (VkImagePlaneMemoryRequirementsInfo){
								.sType = VK_STRUCTURE_TYPE_IMAGE_PLANE_MEMORY_REQUIREMENTS_INFO,
								.planeAspect = mem_plane_aspect(i),
							};
							memri.pNext = &planeri;
						}

						VkMemoryRequirements2 memr = {
							.sType = VK_STRUCTURE_TYPE_MEMORY_REQUIREMENTS_2,
						};

						vkGetImageMemoryRequirements2(dev, &memri, &memr);
						int mem = vulkan_find_mem_type(renderer->dev, 0,
							memr.memoryRequirements.memoryTypeBits & fdp.memoryTypeBits);
						if (mem < 0) {
							wlr_log(WLR_ERROR, "no valid memory type index");
							goto error_image;
						}

						// Since importing transfers ownership of the FD to Vulkan, we have
						// to duplicate it since this operation does not transfer ownership
						// of the attribs to this texture. Will be closed by Vulkan on
						// vkFreeMemory.
						int dfd = fcntl(attribs->fd[i], F_DUPFD_CLOEXEC, 0);
						if (dfd < 0) {
							wlr_log_errno(WLR_ERROR, "fcntl(F_DUPFD_CLOEXEC) failed");
							goto error_image;
						}

						VkMemoryAllocateInfo memi = {
							.sType = VK_STRUCTURE_TYPE_MEMORY_ALLOCATE_INFO,
							.allocationSize = memr.memoryRequirements.size,
							.memoryTypeIndex = mem,
						};

						VkImportMemoryFdInfoKHR importi = {
							.sType = VK_STRUCTURE_TYPE_IMPORT_MEMORY_FD_INFO_KHR,
							.fd = dfd,
							.handleType = htype,
						};
						memi.pNext = &importi;

						VkMemoryDedicatedAllocateInfo dedi = {
							.sType = VK_STRUCTURE_TYPE_MEMORY_DEDICATED_ALLOCATE_INFO,
							.image = image,
						};
						importi.pNext = &dedi;

						res = vkAllocateMemory(dev, &memi, NULL, &mems[i]);
						if (res != VK_SUCCESS) {
							close(dfd);
							wlr_vk_error("vkAllocateMemory failed", res);
							goto error_image;
						}

						++(*n_mems);

						// fill bind info
						bindi[i].image = image;
						bindi[i].memory = mems[i];
						bindi[i].memoryOffset = 0;
						bindi[i].sType = VK_STRUCTURE_TYPE_BIND_IMAGE_MEMORY_INFO;

						if (disjoint) {
							planei[i].sType = VK_STRUCTURE_TYPE_BIND_IMAGE_PLANE_MEMORY_INFO;
							planei[i].planeAspect = planeri.planeAspect;
							bindi[i].pNext = &planei[i];
						}
					}
					res = vkBindImageMemory2(dev, mem_count, bindi);*/
				}*/
				{
					let stride = {assert_eq!(buffer.pitch()%4, 0); buffer.pitch()/4};
					let mut map = drm.map_dumb_buffer(&mut buffer)?;
					assert!(stride * size.y <= map.as_mut().len() as u32, "{} {}", stride * size.y, map.as_mut().len());
					let mut target = image::Image::cast_slice_mut(map.as_mut(), size, stride);
					println!("paint");
					widget.paint(&mut target, size, zero())?;
					println!("present");
					/*// unimplemented wayland PQ. use vulkan ext hdr metadata ?
					if !crate::dark {
						pub const PQ10_to_linear10 : std::sync::LazyLock<[u16; 0x400]> = std::sync::LazyLock::new(|| image::PQ10_EOTF.map(|pq| (pq*0x3FF as f32) as u16));
						#[allow(non_snake_case)] let ref_PQ10_to_linear10 = &PQ10_to_linear10;
						for pq in target.iter_mut() { *pq = u32::from(image::bgr::<u16>::from(*pq).map(|pq| ref_PQ10_to_linear10[pq as usize])) }
					}*/
				}
				dmabuf.create_params(params);
				let modifiers = 0u64;
				use rustix::fd::BorrowedFd;
				params.add(unsafe{BorrowedFd::borrow_raw(drm.buffer_to_prime_fd(buffer.handle(), 0)?)}, 0, 0, buffer.pitch(), (modifiers>>32) as u32, modifiers as u32);
				params.create_immed(buffer_ref, buffer.size().0, buffer.size().1, buffer.format() as u32, 0);
				params.destroy();
				surface.attach(&buffer_ref,0,0);
				buffer_ref.destroy();
				surface.damage_buffer(0, 0, buffer.size().0, buffer.size().1);
				/*let size = pool.target.size;
				widget.paint(&mut pool.target, size, zero())?;
				surface.attach(&pool.buffer,0,0);
				surface.damage_buffer(0, 0, pool.target.size.x, pool.target.size.y);*/
				assert!(done == true);
				done = false;
				assert!(callback.is_none());
				callback = {let callback : Callback = server.new("callback");
					//println!("frame {}", callback.id);
					surface.frame(&callback);
					Some(callback)};
				surface.commit();
				println!("commit {:?}", buffer.size());
				//eprintln!("{:?}ms", time.elapsed().as_millis());*/
			}
		} // idle-event loop
	}
	#[cfg(feature="softbuffer")] pub fn run<T:Widget>(&self, _title: &str, widget: &mut T) -> Result {
		use winit::{event::{self, Event::*, WindowEvent::*, VirtualKeyCode, ElementState}, event_loop::{ControlFlow, EventLoop}, window::WindowBuilder};
		let mut event_loop = EventLoop::new();
		let mut window = WindowBuilder::new().with_inner_size(winit::dpi::PhysicalSize::<u32>::from(<(_,_)>::from(widget.size(xy{x: 3840, y: 2160})))).build(&event_loop)?;
		let context = unsafe{softbuffer::Context::new(&window)}.unwrap();
		let mut surface = unsafe{softbuffer::Surface::new(&context, &window)}.unwrap();
		use winit::platform::run_return::EventLoopExtRunReturn;
		event_loop.run_return(move |event, _, control_flow| match event {
				RedrawRequested(window_id) if window_id == window.id() => {
					let size = {let size = window.inner_size(); xy{x: size.width, y: size.height}};
					widget.event(size, &mut window, &Event::Stale).unwrap();
					surface.resize(std::num::NonZeroU32::new(size.x).unwrap(), std::num::NonZeroU32::new(size.y).unwrap()).unwrap();
					let mut buffer = surface.buffer_mut().unwrap();
					let mut target = image::Image::new::<u32>(size, &mut *buffer);
					target.fill(image::bgr8::from(crate::background()).into());
					widget.paint(&mut target, size, zero()).unwrap();
					buffer.present().unwrap();
				}
				WindowEvent{event: CloseRequested, window_id} if window_id == window.id() => *control_flow = ControlFlow::Exit,
				WindowEvent{event:KeyboardInput{input:event::KeyboardInput{virtual_keycode:Some(VirtualKeyCode::Escape), ..},..},..} => *control_flow = ControlFlow::Exit,
				MainEventsCleared => if widget.event({let size = window.inner_size(); xy{x: size.width, y: size.height}}, &mut window, &Event::Idle).unwrap() { window.request_redraw(); },
				WindowEvent{event:KeyboardInput{input:event::KeyboardInput{virtual_keycode:Some(key), state:ElementState::Pressed, ..},..},..} =>
					if widget.event({let size = window.inner_size(); xy{x: size.width, y: size.height}}, &mut window, &Event::Key(match key {
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
						_ => return if false {println!("{key:?}");} else {}
					})).unwrap() { window.request_redraw(); },
					//if widget.event({let size = window.inner_size(); xy{x: size.width, y: size.height}}, &mut Some(EventContext), &Event::Key('⎙')).unwrap() { window.request_redraw(); },
				_ => {}
		});
		Ok(())
}
}
impl Default for App { fn default() -> Self { Self::new().unwrap() } }
pub fn run<T:Widget>(title: &str, widget: &mut T) -> Result { App::new()?.run(title, widget) }
