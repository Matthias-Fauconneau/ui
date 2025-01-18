// FIXME: split into own crate

pub use std::sync::Arc;
use vulkano::{
	device::{Device, Queue},
	memory::allocator::StandardMemoryAllocator,
	command_buffer::allocator::StandardCommandBufferAllocator,
	descriptor_set::allocator::StandardDescriptorSetAllocator,
};
pub use vulkano::format::Format;

#[derive(Clone)] pub struct Context {
	pub device: Arc<Device>,
	pub queue: Arc<Queue>,
	pub memory_allocator: Arc<StandardMemoryAllocator>,
	pub command_buffer_allocator: Arc<StandardCommandBufferAllocator>,
	pub descriptor_set_allocator: Arc<StandardDescriptorSetAllocator>,
	pub format: Format,
}

use vulkano::command_buffer::{PrimaryAutoCommandBuffer, AutoCommandBufferBuilder};
pub type Commands = AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>;

pub use vulkano::image::{Image, ImageUsage, ImageCreateInfo, view::ImageView, ImageType, sampler::{Sampler, SamplerCreateInfo, Filter}};

pub use vulkano::buffer::subbuffer::BufferContents;
pub use vulkano::pipeline::graphics::vertex_input::Vertex;
pub use vulkano::{Validated, VulkanError};
pub type Result<T=(), E=Box<dyn std::error::Error>/*Validated<VulkanError>*/> = std::result::Result<T,E>;
pub fn default<T: Default>() -> T { Default::default() }
pub use vulkano::descriptor_set::{WriteDescriptorSet, layout::DescriptorType};
use vulkano::{
	shader::ShaderModule,
	command_buffer::{RenderingInfo, RenderingAttachmentInfo},
	render_pass::{AttachmentStoreOp,AttachmentLoadOp},
	buffer::allocator::{SubbufferAllocator, SubbufferAllocatorCreateInfo},
	descriptor_set::DescriptorSet,
	pipeline::{PipelineShaderStageCreateInfo, PipelineLayout, layout::PipelineDescriptorSetLayoutCreateInfo, Pipeline/*:trait*/, PipelineBindPoint, GraphicsPipeline,
		DynamicState,
		graphics::{GraphicsPipelineCreateInfo, subpass::PipelineRenderingCreateInfo, viewport::Viewport,
			vertex_input::VertexDefinition,
			rasterization::{RasterizationState, CullMode},
			depth_stencil::{DepthStencilState, DepthState, CompareOp},
			color_blend::ColorBlendState
		}
	},
};

pub trait Shader {
	type Uniforms: BufferContents+Copy;
	type Vertex: Vertex;
	const NAME: &'static str;
	fn load(device: Arc<Device>) -> Result<Arc<ShaderModule>, Validated<VulkanError>>;
}

pub struct Pass<S> {
	pub pipeline: Arc<GraphicsPipeline>,
	uniform_buffer: SubbufferAllocator,
	_marker: std::marker::PhantomData<S>
}

impl<S:Shader> Pass<S> {
	pub type Uniforms = S::Uniforms;
	
	pub fn new(Context{device, format, memory_allocator, ..}: &Context, depth: bool) -> Result<Self, Box<dyn std::error::Error>> {
		let shader = S::load(device.clone())?;
		let [vertex, fragment] = ["vertex","fragment"].map(|name| PipelineShaderStageCreateInfo::new(shader.entry_point(name).unwrap()));
		let vertex_input_state = (!S::Vertex::per_vertex().members.is_empty()).then_some(S::Vertex::per_vertex().definition(&vertex.entry_point)?);
		let layout = PipelineLayout::new(device.clone(), 
			PipelineDescriptorSetLayoutCreateInfo::from_stages([&vertex, &fragment]).into_pipeline_layout_create_info(device.clone())?)?;
		let pipeline = GraphicsPipeline::new(device.clone(), None, GraphicsPipelineCreateInfo{
			stages: [vertex, fragment].into_iter().collect(),
			vertex_input_state: vertex_input_state.or(Some(default())),
			input_assembly_state: Some(default()),
			viewport_state: Some(default()),
			rasterization_state: Some(RasterizationState{cull_mode: CullMode::Back, ..default()}),
			depth_stencil_state: depth.then_some(DepthStencilState{depth: Some(DepthState{compare_op: CompareOp::LessOrEqual, ..DepthState::simple()}), ..default()}),
			multisample_state: Some(default()),
			color_blend_state: Some(ColorBlendState::with_attachment_states(1, default())),
			dynamic_state: [DynamicState::Viewport].into_iter().collect(),
			subpass: Some(PipelineRenderingCreateInfo{
				color_attachment_formats: vec![Some(*format)],
				depth_attachment_format: depth.then_some(Format::D16_UNORM),
				..default()
			}.into()),
			..GraphicsPipelineCreateInfo::layout(layout)
		})?;
		let uniform_buffer = SubbufferAllocator::new(memory_allocator.clone(), SubbufferAllocatorCreateInfo{
			buffer_usage: BufferUsage::UNIFORM_BUFFER, memory_type_filter: MemoryTypeFilter::HOST_SEQUENTIAL_WRITE, ..default()});
		Ok(Self{pipeline, uniform_buffer, _marker: default()})
	}

	pub fn begin_rendering(&self, Context{descriptor_set_allocator,..}: &Context, commands: &mut Commands, target: Arc<ImageView>, depth: Option<Arc<ImageView>>, 
		clear: bool, uniforms: &S::Uniforms, additional_descriptor_sets: &[WriteDescriptorSet]) -> Result {
		let [extent@..,_] = target.image().extent().map(|u32| u32 as f32);
		commands.begin_rendering(RenderingInfo{
			color_attachments: vec![Some(RenderingAttachmentInfo{
				load_op: if clear { AttachmentLoadOp::Clear } else { AttachmentLoadOp::Load },
				store_op: AttachmentStoreOp::Store,
				clear_value: clear.then_some([0.,0.,0.,0.].into()),
				..RenderingAttachmentInfo::image_view(target)
			})],
			depth_attachment: depth.map(|depth| RenderingAttachmentInfo{
				load_op: if clear { AttachmentLoadOp::Clear } else { AttachmentLoadOp::Load },
				store_op: AttachmentStoreOp::Store,
				clear_value: clear.then_some((1.).into()),
				..RenderingAttachmentInfo::image_view(depth)
			}),
			..default()
		})?
		.set_viewport(0, [Viewport{extent, ..default()}].into_iter().collect())?
		.bind_pipeline_graphics(self.pipeline.clone())?;
		let ref layout = self.pipeline.layout().set_layouts()[0];
		let uniform_buffers = *layout.descriptor_counts().get(&DescriptorType::UniformBuffer).unwrap_or(&0);
		if uniform_buffers > 0 || additional_descriptor_sets.len() > 0 {
			assert!(uniform_buffers <= 1);
			commands.bind_descriptor_sets(PipelineBindPoint::Graphics, self.pipeline.layout().clone(), 0, DescriptorSet::new(descriptor_set_allocator.clone(), layout.clone(),
				(uniform_buffers > 0).then(|| WriteDescriptorSet::buffer(0, {let buffer = self.uniform_buffer.allocate_sized().unwrap(); *buffer.write().unwrap() = *uniforms; buffer}))
				.into_iter().chain(additional_descriptor_sets.into_iter().cloned()), [])? )?;
		}
		Ok(())
	}
}

pub use bytemuck;
#[macro_export] macro_rules! shader {
	{$name:ident} => {
		mod $name {
			use {std::sync::Arc, vulkano::{Validated, VulkanError, device::Device, shader::{ShaderModule, ShaderModuleCreateInfo}}};
			pub use $crate::vulkan::bytemuck;
			vulkano_spirv::shader!{$name}
			pub struct Shader;
			impl $crate::vulkan::Shader for Shader {
				type Uniforms = self::Uniforms;
				type Vertex = self::Vertex;
				const NAME: &'static str = stringify!($name);
				fn load(device: Arc<Device>) -> Result<Arc<ShaderModule>, Validated<VulkanError>> {
					unsafe extern "C" {
						#[link_name=concat!(concat!("_binary_", stringify!($name)), "_spv_start")] static start: [u8; 1];
						#[link_name=concat!(concat!("_binary_", stringify!($name)), "_spv_end")] static end: [u8; 1];
					}
					unsafe{ShaderModule::new(device,
						ShaderModuleCreateInfo::new(&bytemuck::allocation::pod_collect_to_vec(std::slice::from_ptr_range(&start..&end))))}
				}
			}
			pub type Pass = $crate::vulkan::Pass<Shader>;
		}
	}
}

pub use vulkano::buffer::{Subbuffer, BufferUsage};
use vulkano::{memory::allocator::{AllocationCreateInfo, MemoryTypeFilter}, buffer::{Buffer, BufferCreateInfo}};
use vulkano::buffer::AllocateBufferError;

pub fn buffer<T: BufferContents>(Context{memory_allocator, ..}: &Context, usage: BufferUsage, len: usize) -> Result<Subbuffer<[T]>, Validated<AllocateBufferError>> {
	Buffer::new_slice(
		memory_allocator.clone(),
		BufferCreateInfo{usage, ..default()},
		AllocationCreateInfo{memory_type_filter: {type O = MemoryTypeFilter; O::PREFER_DEVICE|O::HOST_SEQUENTIAL_WRITE}, ..default()},
		len as u64
	)
}

pub fn from_iter<T: BufferContents, I: IntoIterator<Item=T>>(Context{memory_allocator, ..}: &Context, usage: BufferUsage, iter: I)
-> Result<Subbuffer<[T]>, Validated<AllocateBufferError>> where I::IntoIter: ExactSizeIterator {
	Buffer::from_iter(
		memory_allocator.clone(),
		BufferCreateInfo{usage, ..default()},
		AllocationCreateInfo{memory_type_filter: {type O = MemoryTypeFilter; O::PREFER_DEVICE|O::HOST_SEQUENTIAL_WRITE}, ..default()},
		iter
	)
}

pub use vulkano::command_buffer::CopyBufferToImageInfo;
