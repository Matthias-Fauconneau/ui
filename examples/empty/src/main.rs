use {std::sync::Arc, ui::{default, vulkan::{Context, Commands, ImageView}}};
struct Empty; impl ui::Widget for Empty { fn paint(&mut self, context: &Context, commands: &mut Commands, target: Arc<ImageView>, _: ui::uint2, _: ui::int2) -> ui::Result<()> {
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
                color_attachment_formats: vec![Some(target.format())],
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
    let [extent@..,_] = target.image().extent().map(|u32| u32 as f32);
	let len = vertex_buffer.len();
	commands.begin_rendering(RenderingInfo{color_attachments: vec![Some(RenderingAttachmentInfo{
		load_op: AttachmentLoadOp::Clear,
		store_op: AttachmentStoreOp::Store,
		clear_value: Some([0.0, 0.0, 1.0, 1.0].into()),
		..RenderingAttachmentInfo::image_view(target)
	})], ..default()})?
	.set_viewport(0, [Viewport{extent, ..default()}].into_iter().collect())?
	.bind_pipeline_graphics(pipeline.clone())?
	.bind_vertex_buffers(0, vertex_buffer)?;
	unsafe{commands.draw(len as u32, 1, 0, 0) }?;
	commands.end_rendering()?;
	Ok(())
} }
fn main() -> ui::Result { ui::run("empty", &mut Empty) }
