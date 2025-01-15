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