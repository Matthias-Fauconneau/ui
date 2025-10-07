use {vector::{num::zero, xy}, crate::{Result, Event, EventContext, Widget}};

#[path="wayland.rs"] mod wayland;
use wayland::*;

use {std::sync::Arc, vulkano::{VulkanLibrary, instance::{Instance, InstanceCreateInfo, InstanceExtensions},
	device::{Device, DeviceCreateInfo, DeviceFeatures, DeviceExtensions, physical::PhysicalDeviceType, QueueCreateInfo, QueueFlags},
	memory::{ExternalMemoryHandleType, ExternalMemoryHandleTypes, allocator::{GenericMemoryAllocatorCreateInfo, StandardMemoryAllocator}},
	command_buffer::{allocator::StandardCommandBufferAllocator, AutoCommandBufferBuilder, CommandBufferUsage, PrimaryCommandBufferAbstract},
	descriptor_set::allocator::StandardDescriptorSetAllocator,
	image::{Image, ImageCreateInfo, ImageUsage, ImageMemory, ImageTiling, view::ImageView}, format::Format,
	sync::future::GpuFuture
}};

use crate::vulkan::{Context, Commands, default};

pub fn new_trigger() -> rustix::io::Result<std::os::fd::OwnedFd> { rustix::event::eventfd(0, rustix::event::EventfdFlags::empty()) }
pub fn trigger(fd: impl std::os::fd::AsFd) -> rustix::io::Result<()> { if rustix::io::write(fd, &1u64.to_ne_bytes())? != 8 { Err(rustix::io::Errno::XFULL) } else { Ok(()) } }

pub fn run<'t>(title: &str, app: Box<dyn std::ops::FnOnce(&Context, &mut Commands) -> Result<Box<dyn Widget+'t>>+'t>) -> Result { run_with_trigger(new_trigger()?, title, app) }
pub fn run_with_trigger<'t>(ref trigger: impl std::os::fd::AsFd, title: &str, app: Box<dyn std::ops::FnOnce(&Context, &mut Commands) -> Result<Box<dyn Widget+'t>>+'t>) -> Result {
	let vulkan = VulkanLibrary::new().unwrap();
	let ref enabled_extensions = InstanceExtensions{ext_debug_utils: true, ..default()};
	let enabled_layers = if true { &["VK_LAYER_KHRONOS_validation"] as &[_] } else { &[] };
	let instance = Instance::new(&vulkan, &InstanceCreateInfo{enabled_extensions, enabled_layers, ..default()})?;
	let ref enabled_extensions = DeviceExtensions{ext_image_drm_format_modifier: true, ext_external_memory_dma_buf: true,
		ext_dynamic_rendering_unused_attachments: true, ..default()};
	// FIXME: select from wayland dmabuf feedback
	let (physical_device, queue_family_index) = instance.enumerate_physical_devices()?
		.filter(|p| [PhysicalDeviceType::DiscreteGpu,PhysicalDeviceType::IntegratedGpu].contains(&p.properties().device_type))
		.filter(|p| p.supported_extensions().contains(enabled_extensions))
		.find_map(|p| {
			let (i, _) = p.queue_family_properties().iter().enumerate().find(|&(_, q)| q.queue_flags.intersects(QueueFlags::GRAPHICS))?;
			Some((p, i as u32))
		}).unwrap();

	let format = Format::B8G8R8A8_SRGB; // B8G8R8_SRGB is not compatible as dmabuf color attachment
	let (device, mut queues) = Device::new(&physical_device, &DeviceCreateInfo{
		enabled_extensions,
		queue_create_infos: &[QueueCreateInfo{queue_family_index, ..default()}],
		enabled_features: &DeviceFeatures{dynamic_rendering: true, dynamic_rendering_unused_attachments: true, ..default()},
		..default()
	})?;
	let queue = queues.next().unwrap();

	let ref memory_types = physical_device.memory_properties().memory_types;
	let ref export_handle_types = vec![ExternalMemoryHandleTypes::DMA_BUF; memory_types.len()];
	let ref block_sizes = vec![256 * 1024 * 1024; memory_types.len()];
	let dmabuf_memory_allocator = Arc::new(StandardMemoryAllocator::new(&device,
		&GenericMemoryAllocatorCreateInfo{block_sizes, export_handle_types, ..default()}));
	let ref mut context = Context{
		memory_allocator: Arc::new(StandardMemoryAllocator::new(&device, &default())),
		command_buffer_allocator: Arc::new(StandardCommandBufferAllocator::new(&device, &default())),
		descriptor_set_allocator: Arc::new(StandardDescriptorSetAllocator::new(&device, &default())),
		device, queue, format,
	};

	let mut commands = AutoCommandBufferBuilder::primary(context.command_buffer_allocator.clone(), context.queue.queue_family_index(),
		CommandBufferUsage::OneTimeSubmit)?;
	let mut app = app(context, &mut commands)?;

	let ref server = Server::connect();
	let display = Display{server, id: 1};
	let ref registry = server.new("registry");
	display.get_registry(registry);
	let ([compositor, wm_base, seat, dmabuf, output], []) = server.globals(registry, ["wl_compositor","xdg_wm_base","wl_seat","zwp_linux_dmabuf_v1","wl_output"], []);
	let ref compositor = Compositor{server, id: compositor};
	let ref wm_base = WmBase{server, id: wm_base};
	let ref seat = Seat{server, id: seat};
	let ref dmabuf = DMABuf{server, id: dmabuf};
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
	let mut window = Surface::new(server, compositor, wm_base, title, if true { Some(output) } else { None });

	let ref feedback : dmabuf::Feedback = server.new("feedback");
	dmabuf.get_surface_feedback(feedback, &window.surface);

	let ref params : dmabuf::Params = server.new("params");
	let ref buffer_ref : Buffer = server.new("buffer_ref");
	let mut framebuffer = [None, None, None];
	let mut scale_factor = 0;
	let mut configure_bounds = zero();
	let mut size = zero();
	let modifiers_state = Default::default();
	let mut fence = commands.build()?.execute(context.queue.clone())?.then_signal_fence_and_flush()?.boxed();

	let timerfd = rustix::time::timerfd_create(rustix::time::TimerfdClockId::Realtime, rustix::time::TimerfdFlags::empty())?;
	let mut repeat : Vec<char> = Vec::new();
	let mut msec : u64 = 0;

	let ref evdev = rustix::fs::open("/dev/input/event17", rustix::fs::OFlags::RDONLY, rustix::fs::Mode::empty()).ok();

	loop {
		let mut commands = AutoCommandBufferBuilder::primary(context.command_buffer_allocator.clone(), context.queue.queue_family_index(),
			CommandBufferUsage::OneTimeSubmit)?;
		let mut need_paint = app.event(context, &mut commands, size, &mut EventContext{modifiers_state}, &Event::Idle).unwrap(); // determines whether to wait for events
		// ^ could also trigger eventfd instead
		loop {
			let events = {
				use rustix::event::{PollFd,PollFlags};
				let server = &*server.server.borrow();
				let mut fds = vec![PollFd::new(server, PollFlags::IN), PollFd::new(trigger, PollFlags::IN)];
				if let Some(evdev) = evdev.as_ref() { fds.push(PollFd::new(evdev, PollFlags::IN)); }
				if !repeat.is_empty() {
					use rustix::time::{timerfd_settime,TimerfdTimerFlags,Itimerspec,Timespec};
					timerfd_settime(&timerfd, TimerfdTimerFlags::ABSTIME,
						&Itimerspec{it_interval: Timespec{tv_sec:0, tv_nsec:0}, it_value: Timespec{tv_sec:(msec/1000) as i64,tv_nsec:((msec%1000)*1000000) as i64}}
					)?;
					fds.push(PollFd::new(&timerfd, PollFlags::IN));
				};
				let zero = default();
				rustix::event::poll(&mut fds, if window.can_paint && window.done && need_paint {Some(&zero)} else {None}).unwrap();
				fds.iter().map(|fd| fd.revents().contains(PollFlags::IN)).collect::<Box<_>>()
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
						server.args({use Type::*; [UInt]});
					}
					else if id == output.id && opcode == output::name {
						server.args({use Type::*; [String]});
					}
					else if id == output.id && opcode == output::description {
						server.args({use Type::*; [String]});
					}
					else if id == output.id && opcode == output::done {
					}
					else if id == feedback.id && opcode == dmabuf::feedback::done {}
					else if id == feedback.id && opcode == dmabuf::feedback::format_table {
						server.args({use Type::*; [UInt]});
						//assert!(_any_fd.is_some());
					}
					else if id == feedback.id && opcode == dmabuf::feedback::main_device {
						server.args({use Type::*; [Array]});
					}
					else if id == feedback.id && opcode == dmabuf::feedback::tranche_target_device {
						let [Array(_dev)] = server.args({use Type::*; [Array]}) else {unreachable!()};
						// FIXME: use to select Vulkan physical device
						//drm = Some(DRM::new(u64::from_ne_bytes(dev[..].try_into().unwrap())));
					}
					else if id == feedback.id && opcode == dmabuf::feedback::tranche_done {}
					else if id == feedback.id && opcode == dmabuf::feedback::tranche_formats {
						server.args({use Type::*; [Array]});
					}
					else if id == feedback.id && opcode == dmabuf::feedback::tranche_flags {
						server.args({use Type::*; [UInt]});
					}
					else if id == window.toplevel.id && opcode == toplevel::close {
						return Ok(());
					}
					else if id == window.toplevel.id && opcode == toplevel::wm_capabilities {
						let [Array(_)] = server.args({use Type::*; [Array]}) else {unreachable!()};
					}
					else if id == window.toplevel.id && opcode == toplevel::configure_bounds {
						let [UInt(_width),UInt(_height)] = server.args({use Type::*; [UInt,UInt]}) else {unreachable!()};
					}
					else if id == window.toplevel.id && opcode == toplevel::configure {
						let [UInt(x),UInt(y),Array(_)] = server.args({use Type::*; [UInt,UInt,Array]}) else {unreachable!()};
						size = xy{x: x*scale_factor, y: y*scale_factor};
						if size.x == 0 || size.y == 0 {
							assert!(configure_bounds.x > 0 && configure_bounds.y > 0);
							let app_size = app.size(configure_bounds).map(|x| x.next_multiple_of(scale_factor));
							size = xy{x: if size.x > 0 { size.x } else { app_size.x}, y: if size.y > 0 { size.y } else { app_size.y }};
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
					else if id == window.surface.id && opcode == surface::preferred_buffer_scale {
						let [UInt(factor)] = server.args({use Type::*; [UInt]}) else {unreachable!()};
						scale_factor = factor;
						window.surface.set_buffer_scale(scale_factor);
					}
					else if id == buffer_ref.id && opcode == buffer::release { println!("release"); }
					else if id == pointer.id && opcode == pointer::enter {
						let [UInt(_),_,_,_] = server.args({use Type::*; [UInt,UInt,UInt,UInt]}) else {unreachable!()};
					}
					else if id == pointer.id && opcode == pointer::leave {
						server.args({use Type::*; [UInt,UInt]});
					}
					else if id == pointer.id && opcode == pointer::motion {
						let [_,Int(x),Int(y)] = server.args({use Type::*; [UInt,Int,Int]}) else {unreachable!()};
						need_paint |= app.event(context, &mut commands, size, &mut EventContext{modifiers_state}, &Event::Motion{position: xy{x: (x*3)>>8,y: (y*3)>>8}, buttons: 0})?;
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
						server.args({use Type::*; [UInt, UInt]});
					}
					else if id == pointer.id && opcode == pointer::axis_value120 {
						server.args({use Type::*; [UInt, UInt]});
					}
					else if id == pointer.id && opcode == pointer::axis_relative_direction {
						server.args({use Type::*; [UInt, UInt]});
					}
					else if id == keyboard.id && opcode == keyboard::keymap {
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
						let [_serial,UInt(_key_time),UInt(key),UInt(state)] = server.args({use Type::*; [UInt,UInt,UInt,UInt]}) else {unreachable!()};
						if key == 1 { return Ok(()); }
						#[allow(non_upper_case_globals)] const usb_hid_usage_table: [char; 126] = ['\0','⎋','1','2','3','4','5','6','7','8','9','0','-','=','⌫','\t','q','w','e','r','t','y','u','i','o','p','{','}','\n','⌃','a','s','d','f','g','h','j','k','l',';','\'','`','⇧','\\','z','x','c','v','b','n','m',',','.','/','⇧','\0','⎇',' ','⇪','\u{F701}','\u{F702}','\u{F703}','\u{F704}','\u{F705}','\u{F706}','\u{F707}','\u{F708}','\u{F709}','\u{F70A}','\0','\0','\0','\0','\0','\0','\0','\0','\0','\0','\0','\0','\0','\0','\0','\0','\0','\0','\0','\0','\u{F70B}','\u{F70C}','\0','\0','\0','\0','\0','\0','\0','\0','⎙','⎄',' ','⇤','↑','⇞','←','→','⇥','↓','⇟','⎀','⌦','\u{F701}','🔇','🕩','🕪','⏻','=','±','⏯','🔎',',','\0','\0','¥','⌘'];
						let key = usb_hid_usage_table[key as usize];
						if state > 0 {
							need_paint |= app.event(context, &mut commands, size, &mut EventContext{modifiers_state}, &Event::Key(key))?;
							let rustix::time::Timespec{tv_sec,tv_nsec} = rustix::time::clock_gettime(rustix::time::ClockId::Realtime);
							msec = (tv_sec as u64*1000+tv_nsec as u64/1000000)+150;
							if !repeat.contains(&key) { repeat.push(key); }
						} else {
							repeat.retain(|&k| k != key);
						}
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
						return Ok(());
					}
					else {
						use itertools::Itertools;
						println!("! {id:?}:{opcode:?} {:?}", server.names.lock().unwrap().iter().format_with(" ", |(id,name),f| f(&format_args!("{id}:{name}"))));
						assert!(server.names.lock().unwrap().iter().any(|&(known,_)| known==id));
					}
					//print!("{id:?}:{opcode:?} ");
				} else { println!("No messages :("); }
			} else {
				if events[1] {
					let trigger_count = {let mut buf = [0; 8]; assert!(rustix::io::read(&trigger, &mut buf)? == buf.len()); u64::from_ne_bytes(buf)};
					assert!(trigger_count < 38);
					need_paint |= app.event(context, &mut commands, size, &mut EventContext{modifiers_state}, &Event::Trigger).unwrap();
					continue;
				}
				if let Some(evdev) = evdev.as_ref() && events[2] {
					#[repr(C)] #[derive(Clone, Copy, Debug)] struct timeval { sec: i64, usec: i64 }
					#[repr(C)] #[derive(Clone, Copy, Debug)] struct input_event { time: timeval, r#type: u16, code: u16, value: i32 }
					unsafe impl bytemuck::Zeroable for input_event {}
					unsafe impl bytemuck::Pod for input_event {}
					let mut buffer = [0; std::mem::size_of::<input_event>()];
					assert_eq!(rustix::io::read(&evdev, &mut buffer).unwrap(), buffer.len());
				 	let input_event{r#type, code, value, ..} = *bytemuck::from_bytes(&buffer);
					const SYN : u16 = 0; const KEY : u16 = 1; /*const REL : u16 = 2;*/ const ABS : u16 = 3;
					let key = match r#type {
						SYN => None,
						KEY =>  {
							const LB : u16 = 310; const RB : u16 = 311;
							match code {
								LB => Some(('l', value > 0)),
								RB => Some(('r', value > 0)),
								_ => {println!("{code}"); None}
							}
						},
						ABS => {
							const X : u16 = 0; const Y : u16 = 1; const Z: u16 = 2; const RZ : u16 = 5; const DX : u16 = 16; const DY : u16 = 17;
							let key = |threshold:i32|
								if value < -threshold { match code {X|DX => Some(('←', true)), Y|DY => Some(('↑', true)),_=>None} }
								else if value > threshold { match code {X|DX => Some(('→', true)), Y|DY => Some(('↓', true)),_=>None} }
								else { match code { X|DX => Some(('↔', false)), Y|DY => Some(('↕', false)), _ => None} };
							match code { 
								DX|DY => key(0), 
								X|Y => key(4096), 
								Z|RZ => {
									need_paint |= app.event(context, &mut commands, size, &mut EventContext{modifiers_state}, &match code {
										Z => Event::PadTrigger('←', value),
										RZ => Event::PadTrigger('→', value),
										_ => unreachable!()
									})?;
									None
								}, 
								_ => None
							}
						}
						_ => {println!("{type}"); None } //unreachable!("{type}")
					};
					let Some((key, pressed)) = key else { continue; };
					if pressed {
						if repeat.contains(&key) { continue; } // Prevent analog stick moves to repeat quickly, wait for timed repeat instead
						if !repeat.contains(&key) { repeat.push(key); }
						need_paint |= app.event(context, &mut commands, size, &mut EventContext{modifiers_state}, &Event::Key(key))?;
						let rustix::time::Timespec{tv_sec,tv_nsec} = rustix::time::clock_gettime(rustix::time::ClockId::Realtime);
						msec = (tv_sec as u64*1000+tv_nsec as u64/1000000)+150;
					} else {
						if key == '↔' { repeat.retain(|&key| key != '←' && key != '→'); }
						else if key == '↕' { repeat.retain(|&key| key != '↑' && key != '↓'); }
						else { repeat.retain(|&k| k != key); }
					}
					continue;
				}
				if events.len() > 3 && events[3] {
					for &key in &repeat {
						need_paint |= app.event(context, &mut commands, size, &mut EventContext{/*toplevel: &window.toplevel,*/ modifiers_state, /*cursor*/}, &Event::Key(key))?;
					}
					msec += 33;
					continue;
				}
				break;
			}
		} // event loop
		if need_paint && size.x > 0 && size.y > 0 {
			framebuffer.rotate_left(1);
			let ref mut framebuffer = framebuffer[0];
			let ref mut framebuffer = framebuffer.get_or_insert_with(|| Image::new(&dmabuf_memory_allocator, &ImageCreateInfo{
				format,
				extent: [size.x, size.y, 1],
				usage: ImageUsage::COLOR_ATTACHMENT,
				tiling: ImageTiling::DrmFormatModifier,
				drm_format_modifiers: &[0],
				external_memory_handle_types: ExternalMemoryHandleTypes::DMA_BUF, ..default()
			}, &default()).unwrap());

			let target = ImageView::new_default(&framebuffer)?;
			crate::time!(app.paint(&context, &mut commands, target, size, zero()))?;
			let next_fence = fence.then_execute(context.queue.clone(), commands.build()?)?.then_signal_fence_and_flush()?;

			dmabuf.create_params(params);
			let ImageMemory::Normal(resource_memory) = framebuffer.memory() else {unreachable!()};
			let ref resource_memory = resource_memory[0];
			let device_memory = resource_memory.device_memory();
			use rustix::fd::FromRawFd;
			let fd = unsafe {std::os::fd::OwnedFd::from_raw_fd(device_memory.export_fd(ExternalMemoryHandleType::DmaBuf).unwrap())}; // FIXME: reuse
			let pitch = size.x*4;
			let modifiers = 0u64;
			params.add(fd, 0, resource_memory.offset() as u32, pitch, (modifiers>>32) as u32, modifiers as u32);
			let format = drm_fourcc::DrmFourcc::Xrgb8888;
			params.create_immed(buffer_ref, size.x, size.y, format as u32, 0);
			params.destroy();
			window.surface.attach(&buffer_ref,0,0);
			buffer_ref.destroy();
			window.surface.damage_buffer(0, 0, size.x, size.y);
			window.done = false;
			let callback = window.callback.get_or_insert_with(|| server.new("callback"));
			window.surface.frame(&callback);

			next_fence.wait(None)?; // FIXME: use linux-drm-syncobj-v1 instead of waiting (but unsupported yet by niri: https://github.com/YaLTeR/niri/issues/785)
			fence = next_fence.boxed();

			window.surface.commit();
		}
	} // {idle; event; draw;} loop
}
