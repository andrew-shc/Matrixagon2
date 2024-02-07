pub mod camera;
pub mod terrain;
pub mod texture;
pub mod debug_ui;

use ash::vk;
use crate::world::{WorldEvent};


// can be modified for new render purposes beyond simple camera and terrain
#[derive(Copy, Clone, Debug)]
pub enum RenderDataPurpose {
    CameraViewProjection,
    BlockTextures,
    TerrainVertices,
    TransparentVertices,
    PresentationInpAttachment,
    DebugUI,
    DebugUIInpAttachment,
}

// TODO: initial render data & update render data
// pub enum RenderInputConfig {
//     VertexInputConfig,
// }


// this enum should rarely be modified for new buffer types
#[derive(Clone, Debug)]
pub enum RenderData {
    InitialDescriptorBuffer(Vec<vk::DescriptorBufferInfo>, RenderDataPurpose),
    InitialDescriptorImage(Vec<vk::DescriptorImageInfo>, RenderDataPurpose),
    RecreateVertexBuffer(vk::Buffer, vk::DeviceMemory, RenderDataPurpose),
    RecreateIndexBuffer(vk::Buffer, vk::DeviceMemory, u32, RenderDataPurpose),
    SetScissorDynamicState(vk::Rect2D, RenderDataPurpose),
}

// using a single master trait for components, since splitting the trait into related methods
// requires the World struct to upcast trait objects (experimental features) without using
// any mutable references on top of Box<dyn T>, so it additionally needs Rc<RefCell<dyn T>> to work
// (double indirection)
pub trait Component {
    // Renderable
    fn render(&self) -> Vec<RenderData>;
    // Interactable
    fn respond_event(&mut self, event: WorldEvent) -> Vec<WorldEvent>;  // emits new event(s)
    fn update(&mut self);
    // Descriptable
    unsafe fn load_descriptors(&mut self, _: vk::CommandPool, _: vk::Queue) -> Vec<RenderData> {Vec::new()}
    unsafe fn destroy(&mut self) {}
}

// TODO: remove
// optionally emit new events and whether the component should render again
#[derive(Default)]
pub struct ComponentEventResponse(pub(crate) Vec<WorldEvent>, pub(crate) bool);
