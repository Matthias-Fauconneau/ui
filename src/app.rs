pub fn default<T: Default>() -> T { Default::default() }
use {vector::{num::zero, xy}, crate::{Result, Event, EventContext, Widget}};

#[path="wayland.rs"] mod wayland;
use wayland::*;

use {std::sync::Arc, vulkano::{VulkanLibrary, Validated, VulkanError, instance::{Instance, InstanceCreateInfo, InstanceExtensions},
	device::{Device, DeviceCreateInfo, DeviceFeatures, DeviceExtensions, physical::PhysicalDeviceType, QueueCreateInfo, QueueFlags},
	memory::allocator::StandardMemoryAllocator,
	command_buffer::{allocator::StandardCommandBufferAllocator, CommandBufferLevel, CommandBufferBeginInfo, CommandBufferUsage},
	descriptor_set::allocator::StandardDescriptorSetAllocator,
	swapchain::{Swapchain, SwapchainCreateInfo, Surface, SwapchainPresentInfo, acquire_next_image},
	command_buffer::RecordingCommandBuffer,
	image::{ImageUsage, view::ImageView}, format::Format,
	sync::{future::{GpuFuture, FenceSignalFuture}, now}}};
/*#[path="vulkan.rs"] mod vulkan;
use vulkan::Context;*/

pub fn run<T:Widget>(title: &str, widget: &mut T) -> Result {
	let vulkan = VulkanLibrary::new().unwrap();
	let enabled_extensions = InstanceExtensions{ext_debug_utils: true, ..default()};
	let enabled_layers = if false { vec!["VK_LAYER_KHRONOS_validation".to_owned()] } else { vec![] };
	let instance = Instance::new(vulkan, InstanceCreateInfo{enabled_extensions, enabled_layers, ..default()})?;
	let enabled_extensions = DeviceExtensions{khr_external_memory_fd: true, ext_external_memory_dma_buf: true, ext_queue_family_foreign: true, ext_image_drm_format_modifier: true, ..default()};
	// FIXME: select from wayland dmabuf feedback
	let (physical_device, queue_family_index) = instance.enumerate_physical_devices()?.find_map(|p| {
		((p.properties().device_type == PhysicalDeviceType::DiscreteGpu || p.properties().device_type == PhysicalDeviceType::IntegratedGpu) && p.supported_extensions().contains(&enabled_extensions))
			.then_some(())?;
		let (i, _) = p.queue_family_properties().iter().enumerate()
			.find(|&(i, q)| q.queue_flags.intersects(QueueFlags::GRAPHICS) /*&& p.surface_support(i as u32, &surface).unwrap_or(false)*/)?;
		Some((p, i as u32))
	}).unwrap();
	let (device, mut queues) = Device::new(physical_device, DeviceCreateInfo{
		enabled_extensions,
		queue_create_infos: vec![QueueCreateInfo{queue_family_index, ..default()}],
		enabled_features: DeviceFeatures{dynamic_rendering: true, dynamic_rendering_unused_attachments: true, ..default()},
		..default()
	})?;
	let queue = queues.next().unwrap();
	let format = Format::B8G8R8_SRGB; //B8G8R8A8_SRGB;
 
	use {std::sync::Arc, vulkano::{device::{Device, Queue}, memory::allocator::StandardMemoryAllocator, command_buffer::allocator::StandardCommandBufferAllocator,  format::Format, descriptor_set::allocator::StandardDescriptorSetAllocator}};
    #[derive(Clone)] pub struct Context {
		pub device: Arc<Device>,
		pub queue: Arc<Queue>,
		pub memory_allocator: Arc<StandardMemoryAllocator>,
		pub command_buffer_allocator: Arc<StandardCommandBufferAllocator>,
		pub descriptor_set_allocator: Arc<StandardDescriptorSetAllocator>,
		pub format: Format,
    }
    let ref mut context = Context{
        memory_allocator: Arc::new(StandardMemoryAllocator::new_default(device.clone())),
        command_buffer_allocator: Arc::new(StandardCommandBufferAllocator::new(device.clone(), default())),
        descriptor_set_allocator: Arc::new(StandardDescriptorSetAllocator::new(device.clone(), default())),
        device, queue, format,
    };
    let mut commands = RecordingCommandBuffer::new(context.command_buffer_allocator.clone(), context.queue.queue_family_index(), CommandBufferLevel::Primary, CommandBufferBeginInfo{usage: CommandBufferUsage::OneTimeSubmit, ..default()})?;
    
	let ref server = Server::connect();
	let display = Display{server, id: 1};
	let ref registry = server.new("registry");
	display.get_registry(registry);
	let ([compositor, wm_base, seat, dmabuf, lease_device, output], []) = server.globals(registry, ["wl_compositor","xdg_wm_base","wl_seat","zwp_linux_dmabuf_v1","wp_drm_lease_device_v1","wl_output"], []);
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
	
	let ref feedback : dmabuf::Feedback = server.new("feedback");
	dmabuf.get_surface_feedback(feedback, &window.surface);
	
	let ref params : dmabuf::Params = server.new("params");
	let ref buffer_ref : Buffer = server.new("buffer_ref");
	let mut framebuffer = None;
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
							let app_size = widget.size(configure_bounds).map(|x| x.next_multiple_of(scale_factor));
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
						if key == 1 { return Ok(()); }
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
		if need_paint && size.x > 0 && size.y > 0 {
			use vulkano::memory::{DeviceMemory, MemoryAllocateInfo, ExternalMemoryHandleType, ExternalMemoryHandleTypes, MemoryMapInfo};
			let format = drm_fourcc::DrmFourcc::Xrgb8888;
			let pitch = size.x*4;
			let image_size = (size.y*pitch) as u64;
			let allocation_size = (4*image_size) as u64;
			if framebuffer.as_ref().is_some_and(|(memory,_,_):&(DeviceMemory,_,_)| memory.allocation_size() != allocation_size) { framebuffer = None; }
			let &mut (ref mut memory, ref mut fd, ref mut framebuffers) = framebuffer.get_or_insert_with(|| {
				widget.event(size, &mut EventContext{modifiers_state}, &Event::Stale).unwrap();
				let memory_type_index = 1;
				let mut memory = DeviceMemory::allocate(context.device.clone(), MemoryAllocateInfo{allocation_size, memory_type_index,
					export_handle_types: ExternalMemoryHandleTypes::DMA_BUF, ..default()}).unwrap();
				memory.map(MemoryMapInfo{offset: 0, size: allocation_size, ..default()}).unwrap();
				let fd = memory.export_fd(ExternalMemoryHandleType::DmaBuf).unwrap();
				(memory, fd, [(0*image_size, image_size), (1*image_size, image_size), (2*image_size, image_size), (3*image_size, image_size)])
			});
			
			let (offset, len) = framebuffers[0];
			framebuffers.rotate_left(1);
			{
				let stride = {assert_eq!(pitch%4, 0); pitch/4};
				let map = memory.mapping_state().unwrap();
				let mut map = map.slice(offset..offset+len).unwrap();
				let mut target = image::Image::cast_slice_mut(unsafe{map.as_mut()}, size, stride);
				widget.paint(&mut target, size, zero()).unwrap();
			}
			
			dmabuf.create_params(params);
			let modifiers = 0u64;
			params.add(fd, 0, offset as u32, pitch, (modifiers>>32) as u32, modifiers as u32);
			params.create_immed(buffer_ref, size.x, size.y, format as u32, 0);
			params.destroy();
			window.surface.attach(&buffer_ref,0,0);
			buffer_ref.destroy();
			window.surface.damage_buffer(0, 0, size.x, size.y);
			window.done = false;
			let callback = window.callback.get_or_insert_with(|| server.new("callback"));
			window.surface.frame(&callback);
			window.surface.commit();
		}
	} // {idle; event; draw;} loop
}
