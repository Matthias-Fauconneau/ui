use std::sync::Arc;
use vulkano::{device::{Device, Queue}, memory::allocator::StandardMemoryAllocator, command_buffer::allocator::StandardCommandBufferAllocator,  format::Format, descriptor_set::allocator::StandardDescriptorSetAllocator};
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

pub use vulkano::image::view::ImageView;

use crate::{default, Error, throws};
pub use vulkano::buffer::subbuffer::BufferContents;
pub use vulkano::pipeline::graphics::vertex_input::Vertex;
use vulkano::{
	shader::ShaderModule,
	command_buffer::{RenderingInfo, RenderingAttachmentInfo},
	render_pass::{AttachmentStoreOp,AttachmentLoadOp},
	descriptor_set::{DescriptorSet, WriteDescriptorSet},
	buffer::allocator::{SubbufferAllocator, SubbufferAllocatorCreateInfo},
	pipeline::{Pipeline, PipelineShaderStageCreateInfo, PipelineLayout, PipelineBindPoint, layout::PipelineDescriptorSetLayoutCreateInfo, GraphicsPipeline, DynamicState,
		graphics::{GraphicsPipelineCreateInfo, subpass::PipelineRenderingCreateInfo, viewport::Viewport,
			vertex_input::VertexDefinition,
			rasterization::{RasterizationState, CullMode},
			//depth_stencil::{DepthStencilState, DepthState, CompareOp},
			color_blend::ColorBlendState
		}
	},
};

pub trait Shader {
	type Uniforms: BufferContents+Copy;
	type Vertex: Vertex;
	const NAME: &'static str;
	fn load(device: Arc<Device>) -> Result< Arc<ShaderModule>, vulkano::Validated<vulkano::VulkanError> >;
}

pub struct Pass<S> {
	pub pipeline: Arc<GraphicsPipeline>,
	uniform_buffer: SubbufferAllocator,
	_marker: std::marker::PhantomData<S>
}

impl<S:Shader> Pass<S> {
	pub type Uniforms = S::Uniforms;
	
	#[throws] pub fn new(Context{device, format, memory_allocator, ..}: &Context) -> Self {
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
			//depth_stencil_state: Some(DepthStencilState{depth: Some(DepthState{compare_op: CompareOp::LessOrEqual, ..DepthState::simple()}), ..default()}),
			multisample_state: Some(default()),
			color_blend_state: Some(ColorBlendState::with_attachment_states(1, default())),
			//color_blend_state: Some(ColorBlendState::with_attachment_states(1, ColorBlendAttachmentState{blend: Some(AttachmentBlend::alpha()), ..default()})),
			dynamic_state: [DynamicState::Viewport].into_iter().collect(),
   			subpass: Some(PipelineRenderingCreateInfo{
				color_attachment_formats: vec![Some(*format)],
				//depth_attachment_format: Some(Format::/*D16_UNORM*/D32_SFLOAT/*FIXME*/),
				..default()
			}.into()),
			..GraphicsPipelineCreateInfo::layout(layout)
		})?;
		let uniform_buffer = SubbufferAllocator::new(memory_allocator.clone(), SubbufferAllocatorCreateInfo{
			buffer_usage: BufferUsage::UNIFORM_BUFFER, memory_type_filter: MemoryTypeFilter::HOST_SEQUENTIAL_WRITE, ..default()});
		Self{pipeline, uniform_buffer, _marker: default()}
	}

	#[throws] pub fn begin_rendering(&self, Context{descriptor_set_allocator,..}: &Context, commands: &mut Commands, target: Arc<ImageView>, uniforms: &S::Uniforms) {
		let [extent@..,_] = target.image().extent().map(|u32| u32 as f32);
		commands.begin_rendering(RenderingInfo{
			color_attachments: vec![Some(RenderingAttachmentInfo{
				load_op: AttachmentLoadOp::Clear,
				store_op: AttachmentStoreOp::Store,
				clear_value: Some([0.,0.,0.,0.].into()),
				..RenderingAttachmentInfo::image_view(target)
			})],
			..default()
		}).unwrap()
		.set_viewport(0, [Viewport{extent, ..default()}].into_iter().collect()).unwrap()
		.bind_pipeline_graphics(self.pipeline.clone())?;
		let layout = self.pipeline.layout().set_layouts().get(0).unwrap();
		if layout.descriptor_counts().len() > 0 {
			commands.bind_descriptor_sets(PipelineBindPoint::Graphics, self.pipeline.layout().clone(), 0,
				DescriptorSet::new(descriptor_set_allocator.clone(), layout.clone(),
					[WriteDescriptorSet::buffer(0, {let buffer = self.uniform_buffer.allocate_sized().unwrap(); *buffer.write()? = *uniforms; buffer})].into_iter(), [])?)?;
		}
	}
}

pub use bytemuck;
#[macro_export] macro_rules! shader {
	{$name:ident, $vertex:ty, $Name:ident} => {
		mod $name {
			use vulkano::{Validated, VulkanError, device::Device, shader::{ShaderModule, ShaderModuleCreateInfo}};
			vulkano_spirv::shader!{$name}
			pub struct Shader;
			use super::*;
			impl $crate::vulkan::Shader for Shader {
				type Uniforms = self::Uniforms;
				type Vertex = $vertex;
				const NAME: &'static str = stringify!($name);
				fn load(device: Arc<Device>)->Result<Arc<ShaderModule>,Validated<VulkanError>> {
					unsafe extern "C" {
						#[link_name=concat!(concat!("_binary_", stringify!($name)), "_spv_start")] static start: [u8; 1];
						#[link_name=concat!(concat!("_binary_", stringify!($name)), "_spv_end")] static end: [u8; 1];
					}
					unsafe{ShaderModule::new(device,
						ShaderModuleCreateInfo::new(&$crate::vulkan::bytemuck::allocation::pod_collect_to_vec(std::slice::from_ptr_range(&start..&end))))}
				}
			}
			pub type Pass = $crate::vulkan::Pass<Shader>;
		}
		pub use $name::Pass as $Name;
	}
}

pub use vulkano::buffer::{Subbuffer, BufferUsage};
use vulkano::{memory::allocator::{AllocationCreateInfo, MemoryTypeFilter}, buffer::{Buffer, BufferCreateInfo}};
use vulkano::{Validated, buffer::AllocateBufferError};

pub fn buffer<T: BufferContents>(Context{memory_allocator, ..}: &Context, usage: BufferUsage, len: usize) -> Result<Subbuffer<[T]>, Validated<AllocateBufferError>> {
	Buffer::new_slice(
		memory_allocator.clone(),
		BufferCreateInfo{usage, ..default()},
		AllocationCreateInfo{memory_type_filter: {type O = MemoryTypeFilter; O::PREFER_DEVICE|O::HOST_SEQUENTIAL_WRITE}, ..default()},
		len as u64
	)
}

/*pub fn from_data<T: BufferContents>(Context{memory_allocator, ..}: &Context, usage: BufferUsage, data: T) -> Result<Subbuffer<T>, Validated<AllocateBufferError>> {
	Buffer::from_data(
		memory_allocator.clone(),
		BufferCreateInfo{usage, ..default()},
		AllocationCreateInfo{memory_type_filter: {type O = MemoryTypeFilter; O::PREFER_DEVICE|O::HOST_SEQUENTIAL_WRITE}, ..default()},
		data,
	)
}*/

pub fn from_iter<T: BufferContents, I: IntoIterator<Item=T>>(Context{memory_allocator, ..}: &Context, usage: BufferUsage, iter: I) -> Result<Subbuffer<[T]>, Validated<AllocateBufferError>>
where  I::IntoIter: ExactSizeIterator {
	Buffer::from_iter(
		memory_allocator.clone(),
		BufferCreateInfo{usage, ..default()},
		AllocationCreateInfo{memory_type_filter: {type O = MemoryTypeFilter; O::PREFER_DEVICE|O::HOST_SEQUENTIAL_WRITE}, ..default()},
		iter
	)
}
