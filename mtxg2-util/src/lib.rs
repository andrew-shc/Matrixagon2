use ash::vk;

pub use matrixagon_derive::Vertex;


pub trait VulkanVertexState<const A: usize> {
    const BINDING_DESCRIPTION: vk::VertexInputBindingDescription;
    const ATTRIBUTE_DESCRIPTION: [vk::VertexInputAttributeDescription; A];
    const VERTEX_INPUT_STATE: vk::PipelineVertexInputStateCreateInfo;
}
