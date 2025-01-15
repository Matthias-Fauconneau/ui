pub fn default<T: Default>() -> T { Default::default() }
use {vector::{num::zero, xy}, crate::{Result, Event, EventContext, Widget}};

#[path="wayland.rs"] mod wayland;
use wayland::*;

use {std::sync::Arc, vulkano::{VulkanLibrary, Validated, VulkanError, instance::{Instance, InstanceCreateInfo, InstanceExtensions},
	device::{Device, DeviceCreateInfo, DeviceFeatures, DeviceExtensions, physical::PhysicalDeviceType, QueueCreateInfo, QueueFlags},
	memory::{DeviceMemory, MemoryAllocateInfo, ExternalMemoryHandleType, ExternalMemoryHandleTypes, MemoryMapInfo, allocator::{GenericMemoryAllocatorCreateInfo, StandardMemoryAllocator}},
	command_buffer::{allocator::StandardCommandBufferAllocator, CommandBufferLevel, CommandBufferBeginInfo, CommandBufferUsage},
	descriptor_set::allocator::StandardDescriptorSetAllocator,
	command_buffer::RecordingCommandBuffer,
	image::{ImageFormatInfo, ImageDrmFormatModifierInfo, Image, ImageCreateInfo, ImageUsage, ImageMemory, ImageTiling, view::ImageView}, format::Format,
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
	let format = Format::B8G8R8A8_SRGB; // B8G8R8_SRGB is not compatible as dmabuf color attachment
	/*println!("{:?}", physical_device.image_format_properties(ImageFormatInfo{
		format, 
		usage: ImageUsage::COLOR_ATTACHMENT, 
		external_memory_handle_type: Some(ExternalMemoryHandleType::DmaBuf),
		tiling: ImageTiling::DrmFormatModifier,
		drm_format_modifier_info: Some(ImageDrmFormatModifierInfo{drm_format_modifier: 0, ..default()}),
		..default()
	})?.unwrap());*/
	let (device, mut queues) = Device::new(physical_device.clone(), DeviceCreateInfo{
		enabled_extensions,
		queue_create_infos: vec![QueueCreateInfo{queue_family_index, ..default()}],
		enabled_features: DeviceFeatures{dynamic_rendering: true, dynamic_rendering_unused_attachments: true, ..default()},
		..default()
	})?;
	let queue = queues.next().unwrap();
	
	let ref memory_types = physical_device.memory_properties().memory_types;
    let ref export_handle_types = vec![ExternalMemoryHandleTypes::DMA_BUF; memory_types.len()];
    let ref block_sizes = vec![256 * 1024 * 1024; memory_types.len()];
    let dmabuf_memory_allocator = Arc::new(StandardMemoryAllocator::new(device.clone(), GenericMemoryAllocatorCreateInfo{block_sizes, export_handle_types, ..default()}));
    
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
	let mut framebuffer = [None, None, None];
	let mut scale_factor = 0;
	let mut configure_bounds = zero();
	let mut size = zero();
	let modifiers_state = Default::default();
	let mut previous_frame_end = Some(now(context.device.clone()).boxed());

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
			let pitch = size.x*4;
			/*let image_size = (size.y*pitch) as u64;
			let allocation_size = (4*image_size) as u64;*/
			/*if framebuffer.as_ref().is_some_and(|(memory,_,_):&(DeviceMemory,_,_)| memory.allocation_size() != allocation_size) { framebuffer = None; }
			let &mut (ref mut memory, ref mut fd, ref mut framebuffers) = framebuffer.get_or_insert_with(|| {
				widget.event(size, &mut EventContext{modifiers_state}, &Event::Stale).unwrap();
				let memory_type_index = 1;
				let mut memory = DeviceMemory::allocate(context.device.clone(), MemoryAllocateInfo{allocation_size, memory_type_index,
					export_handle_types: ExternalMemoryHandleTypes::DMA_BUF, ..default()}).unwrap();
				memory.map(MemoryMapInfo{offset: 0, size: allocation_size, ..default()}).unwrap();
				let fd = memory.export_fd(ExternalMemoryHandleType::DmaBuf).unwrap();
				(memory, fd, [(0*image_size, image_size), (1*image_size, image_size), (2*image_size, image_size), (3*image_size, image_size)])
			});*/
			framebuffer.rotate_left(1);
			let ref mut framebuffer = framebuffer[0];
			let ref mut framebuffer = framebuffer.get_or_insert_with(|| {
				println!("{size}");
				let image = Image::new(dmabuf_memory_allocator.clone(), ImageCreateInfo{format, extent: [size.x, size.y, 1], usage: ImageUsage::COLOR_ATTACHMENT, 
					tiling: ImageTiling::DrmFormatModifier, drm_format_modifiers: vec![0], external_memory_handle_types: ExternalMemoryHandleTypes::DMA_BUF, ..default()}, default()).unwrap();
				println!("OK");
				image
			});
			//let (offset, len) = framebuffer;
			
			{
				/*let stride = {assert_eq!(pitch%4, 0); pitch/4};
				let map = memory.mapping_state().unwrap();
				let mut map = map.slice(offset..offset+len).unwrap();
				let mut target = image::Image::cast_slice_mut(unsafe{map.as_mut()}, size, stride);
				widget.paint(&mut target, size, zero()).unwrap();*/
    #[derive(BufferContents, Vertex)]
        #[repr(C)]
        struct MyVertex {
            #[format(R32G32_SFLOAT)]
            position: [f32; 2],
        }
        	use vulkano::pipeline::graphics::vertex_input::{Vertex, VertexDefinition};

			 	mod vs {
            vulkano_shaders::shader! {
                ty: "vertex",
                src: r"
                    #version 450

                    layout(location = 0) in vec2 position;

                    void main() {
                        gl_Position = vec4(position, 0.0, 1.0);
                    }
                ",
            }
        }

        mod fs {
            vulkano_shaders::shader! {
                ty: "fragment",
                src: r"
                    #version 450

                    layout(location = 0) out vec4 f_color;

                    void main() {
                        f_color = vec4(1.0, 0.0, 0.0, 1.0);
                    }
                ",
            }
        }

        // Before we draw, we have to create what is called a **pipeline**. A pipeline describes
        // how a GPU operation is to be performed. It is similar to an OpenGL program, but it also
        // contains many settings for customization, all baked into a single object. For drawing,
        // we create a **graphics** pipeline, but there are also other types of pipeline.
        let pipeline = {
            // First, we load the shaders that the pipeline will use: the vertex shader and the
            // fragment shader.
            //
            // A Vulkan shader can in theory contain multiple entry points, so we have to specify
            // which one.
            let vs = vs::load(context.device.clone())
                .unwrap()
                .entry_point("main")
                .unwrap();
            let fs = fs::load(context.device.clone())
                .unwrap()
                .entry_point("main")
                .unwrap();

            // Automatically generate a vertex input state from the vertex shader's input
                        // interface, that takes a single vertex buffer containing `Vertex` structs.
                        let vertex_input_state = MyVertex::per_vertex().definition(&vs).unwrap();
                        
            // Make a list of the shader stages that the pipeline will have.
            let stages = [
                PipelineShaderStageCreateInfo::new(vs),
                PipelineShaderStageCreateInfo::new(fs),
            ];

            // We must now create a **pipeline layout** object, which describes the locations and
            // types of descriptor sets and push constants used by the shaders in the pipeline.
            //
            // Multiple pipelines can share a common layout object, which is more efficient. The
            // shaders in a pipeline must use a subset of the resources described in its pipeline
            // layout, but the pipeline layout is allowed to contain resources that are not present
            // in the shaders; they can be used by shaders in other pipelines that share the same
            // layout. Thus, it is a good idea to design shaders so that many pipelines have common
            // resource locations, which allows them to share pipeline layouts.
            let layout = PipelineLayout::new(
                context.device.clone(),
                // Since we only have one pipeline in this example, and thus one pipeline layout,
                // we automatically generate the creation info for it from the resources used in
                // the shaders. In a real application, you would specify this information manually
                // so that you can re-use one layout in multiple pipelines.
                PipelineDescriptorSetLayoutCreateInfo::from_stages(&stages)
                    .into_pipeline_layout_create_info(context.device.clone())
                    .unwrap(),
            )
            .unwrap();

            // We describe the formats of attachment images where the colors, depth and/or stencil
            // information will be written. The pipeline will only be usable with this particular
            // configuration of the attachment images.
            let subpass = PipelineRenderingCreateInfo {
                // We specify a single color attachment that will be rendered to. When we begin
                // rendering, we will specify a swapchain image to be used as this attachment, so
                // here we set its format to be the same format as the swapchain.
                color_attachment_formats: vec![Some(framebuffer.format())],
                ..Default::default()
            };

         
                        
            // Finally, create the pipeline.
            GraphicsPipeline::new(
                context.device.clone(),
                None,
                GraphicsPipelineCreateInfo {
                    stages: stages.into_iter().collect(),
                    // How vertex data is read from the vertex buffers into the vertex shader.
                    vertex_input_state: Some(vertex_input_state),
                    // How vertices are arranged into primitive shapes. The default primitive shape
                    // is a triangle.
                    input_assembly_state: Some(default()),
                    // How primitives are transformed and clipped to fit the framebuffer. We use a
                    // resizable viewport, set to draw over the entire window.
                    viewport_state: Some(default()),
                    // How polygons are culled and converted into a raster of pixels. The default
                    // value does not perform any culling.
                    rasterization_state: Some(default()),
                    // How multiple fragment shader samples are converted to a single pixel value.
                    // The default value does not perform any multisampling.
                    multisample_state: Some(default()),
                    // How pixel values are combined with the values already present in the
                    // framebuffer. The default value overwrites the old value with the new one,
                    // without any blending.
                    color_blend_state: Some(ColorBlendState::with_attachment_states(
                        subpass.color_attachment_formats.len() as u32,
                        ColorBlendAttachmentState::default(),
                    )),
                    // Dynamic states allows us to specify parts of the pipeline settings when
                    // recording the command buffer, before we perform drawing. Here, we specify
                    // that the viewport should be dynamic.
                    dynamic_state: [DynamicState::Viewport].into_iter().collect(),
                    subpass: Some(subpass.into()),
                    ..GraphicsPipelineCreateInfo::layout(layout)
                },
            )
            .unwrap()
        };
				let vertices = [MyVertex{position:[-0.5, -0.25]},MyVertex{position:[0.0, 0.5]},MyVertex{position:[0.25, -0.1]}];
				use vulkano::{memory::allocator::{AllocationCreateInfo, MemoryTypeFilter}, buffer::{Buffer, BufferCreateInfo, BufferUsage, Subbuffer, subbuffer::BufferContents}};
				use vulkano::{
					device::Device,	shader::ShaderModule,
					command_buffer::{AutoCommandBufferBuilder, RecordingCommandBuffer, RenderingInfo, RenderingAttachmentInfo},
					render_pass::{AttachmentStoreOp,AttachmentLoadOp},
					image::view::ImageView, format::Format,
					descriptor_set::{DescriptorSet, WriteDescriptorSet},
					pipeline::{Pipeline, PipelineShaderStageCreateInfo, PipelineLayout, PipelineBindPoint, layout::PipelineDescriptorSetLayoutCreateInfo, GraphicsPipeline, DynamicState,
						graphics::{GraphicsPipelineCreateInfo, subpass::PipelineRenderingCreateInfo, viewport::Viewport,
							rasterization::{RasterizationState, CullMode},
							depth_stencil::{DepthStencilState, DepthState, CompareOp},
							color_blend::{ColorBlendState, ColorBlendAttachmentState, AttachmentBlend}
						}
					},
				};
		        let vertex_buffer = Buffer::from_iter(
					context.memory_allocator.clone(), 
					BufferCreateInfo{usage: BufferUsage::VERTEX_BUFFER, ..default()},
            		AllocationCreateInfo{memory_type_filter: MemoryTypeFilter::PREFER_DEVICE|MemoryTypeFilter::HOST_SEQUENTIAL_WRITE, ..default()}, 
              		vertices
				).unwrap();
				let framebuffer = ImageView::new_default(framebuffer.clone())?;
				let mut builder = AutoCommandBufferBuilder::primary(context.command_buffer_allocator.clone(), context.queue.queue_family_index(),
					CommandBufferUsage::OneTimeSubmit)?;
				let [extent@..,_] = framebuffer.image().extent().map(|u32| u32 as f32);
				let len = vertex_buffer.len();
				builder.begin_rendering(RenderingInfo{color_attachments: vec![Some(RenderingAttachmentInfo{
					load_op: AttachmentLoadOp::Clear,
					store_op: AttachmentStoreOp::Store,
					clear_value: Some([0.0, 0.0, 1.0, 1.0].into()),
					..RenderingAttachmentInfo::image_view(framebuffer)
				})], ..default()})?
				.set_viewport(0, [Viewport{extent, ..default()}].into_iter().collect())?
				.bind_pipeline_graphics(pipeline.clone())?
				.bind_vertex_buffers(0, vertex_buffer)?;
				unsafe{builder.draw(len as u32, 1, 0, 0) }?;
				builder.end_rendering()?;
				let command_buffer = builder.build().unwrap();
				let future = previous_frame_end.take().unwrap()
					//.join(acquire_future)
					.then_execute(context.queue.clone(), command_buffer)?
				/*// The color output is now expected to contain our triangle. But in order to
					// show it on the screen, we have to *present* the image by calling
					// `then_swapchain_present`.
					//
					// This function does not actually present the image immediately. Instead it
					// submits a present command at the end of the queue. This means that it will
					// only be presented once the GPU has finished executing the command buffer
					// that draws the triangle.
					.then_swapchain_present(
						self.queue.clone(),
						SwapchainPresentInfo::swapchain_image_index(
							rcx.swapchain.clone(),
							image_index,
						),
					)*/
					.then_signal_fence_and_flush();

				match future.map_err(Validated::unwrap) {
					Ok(future) => {
						previous_frame_end = Some(future.boxed());
					}
					Err(VulkanError::OutOfDate) => {
						previous_frame_end = Some(now(context.device.clone()).boxed());
					}
					Err(e) => {
						println!("failed to flush future: {e}");
						previous_frame_end = Some(now(context.device.clone()).boxed());
					}
				}
			}
			
			dmabuf.create_params(params);
			let ImageMemory::Normal(resource_memory) = framebuffer.memory() else {unreachable!()};
			let ref resource_memory = resource_memory[0];
			let device_memory = resource_memory.device_memory();
			let fd = device_memory.export_fd(ExternalMemoryHandleType::DmaBuf).unwrap(); // FIXME: reuse
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
			window.surface.commit();
		}
	} // {idle; event; draw;} loop
}
