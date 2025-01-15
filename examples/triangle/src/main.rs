#![feature(slice_from_ptr_range)] // shader
/*use vulkano::{
	memory::allocator::{AllocationCreateInfo, MemoryTypeFilter}, 
	buffer::{Buffer, BufferCreateInfo, BufferUsage, subbuffer::BufferContents},
	command_buffer::{RenderingInfo, RenderingAttachmentInfo},
	render_pass::{AttachmentStoreOp,AttachmentLoadOp},
	pipeline::{PipelineShaderStageCreateInfo, PipelineLayout, layout::PipelineDescriptorSetLayoutCreateInfo, GraphicsPipeline, DynamicState,
		graphics::{GraphicsPipelineCreateInfo, subpass::PipelineRenderingCreateInfo, viewport::Viewport, 
			vertex_input::{Vertex, VertexDefinition}, color_blend::ColorBlendState}
	},
};*/

use {std::sync::Arc, ui::vulkan::{Context, Commands, ImageView}};

ui::shader!{triangle, Triangle}

struct App;
impl ui::Widget for App { 
fn paint(&mut self, context/*@Context{device, memory_allocator, ..}*/: &Context, commands: &mut Commands, target: Arc<ImageView>, _: ui::uint2, _: ui::int2) -> ui::Result<()> {
	let triangle = Triangle::new(context)?;
	triangle.begin_rendering(context, commands, target.clone(), &[])?;
	unsafe{commands.draw(4, 1, 0, 0)}?;
	commands.end_rendering()?;
	//#[derive(BufferContents, Vertex)] #[repr(C)] struct MyVertex { #[format(R32G32_SFLOAT)] position: [f32; 2] }
	/*mod vs { vulkano_shaders::shader!{ty: "vertex", src: r"#version 450
		layout(location = 0) in vec2 position;
		void main() { gl_Position = vec4(position, 0.0, 1.0); }"}}
	mod fs { vulkano_shaders::shader!{ty: "fragment", src: r"#version 450
		layout(location = 0) out vec4 f_color;
		void main() { f_color = vec4(1.0, 0.0, 0.0, 1.0); }"}}
	let pipeline = {
		let vs = vs::load(device.clone())?.entry_point("main").unwrap();
		let fs = fs::load(device.clone())?.entry_point("main").unwrap();
		let vertex_input_state = MyVertex::per_vertex().definition(&vs).unwrap();
		let stages = [PipelineShaderStageCreateInfo::new(vs), PipelineShaderStageCreateInfo::new(fs)];
		let layout = PipelineLayout::new(device.clone(), PipelineDescriptorSetLayoutCreateInfo::from_stages(&stages).into_pipeline_layout_create_info(device.clone())?)?;
		let subpass = PipelineRenderingCreateInfo {color_attachment_formats: vec![Some(target.format())], ..default()};
		GraphicsPipeline::new(device.clone(), None, GraphicsPipelineCreateInfo{ 
			stages: stages.into_iter().collect(),
			vertex_input_state: Some(vertex_input_state),
			input_assembly_state: Some(default()),
			viewport_state: Some(default()),
			rasterization_state: Some(default()),
			multisample_state: Some(default()),
			color_blend_state: Some(ColorBlendState::with_attachment_states(subpass.color_attachment_formats.len() as u32, default())),
			dynamic_state: [DynamicState::Viewport].into_iter().collect(),
			subpass: Some(subpass.into()),
			..GraphicsPipelineCreateInfo::layout(layout)
		})?
	};
	let vertices = [MyVertex{position:[-0.5, -0.25]},MyVertex{position:[0.0, 0.5]},MyVertex{position:[0.25, -0.1]}];
	let vertex_buffer = Buffer::from_iter(
		memory_allocator.clone(), 
		BufferCreateInfo{usage: BufferUsage::VERTEX_BUFFER, ..default()},
		AllocationCreateInfo{memory_type_filter: MemoryTypeFilter::PREFER_DEVICE|MemoryTypeFilter::HOST_SEQUENTIAL_WRITE, ..default()}, 
		vertices
	)?;
	let [extent@..,_] = target.image().extent().map(|u32| u32 as f32);
	commands.begin_rendering(RenderingInfo{color_attachments: vec![Some(RenderingAttachmentInfo{
		load_op: AttachmentLoadOp::Clear,
		store_op: AttachmentStoreOp::Store,
		clear_value: Some([0.0, 0.0, 1.0, 1.0].into()),
		..RenderingAttachmentInfo::image_view(target)
	})], ..default()})?;
	commands.set_viewport(0, [Viewport{extent, ..default()}].into_iter().collect())?;
	commands.bind_pipeline_graphics(pipeline.clone())?;
	let len = vertex_buffer.len();
	commands.bind_vertex_buffers(0, vertex_buffer)?;
	unsafe{commands.draw(len as u32, 1, 0, 0) }?;
	commands.end_rendering()?;*/
	Ok(())
}
}

fn main() -> ui::Result {
	ui::run("triangle", &mut App)
}
