pub mod camera;
pub mod terrain;
pub mod texture;
pub mod debug_ui;
pub mod tick;
pub mod flags;

use ash::vk;
use crate::util::CmdBufContext;
use crate::world::{WorldEvent};


// can be modified for new render purposes beyond simple camera and terrain
#[derive(Copy, Clone, Debug, Hash, PartialEq, Eq)]
pub enum RenderDataPurpose {
    CameraViewProjection,
    BlockTextures,
    TerrainOpaque,
    TerrainTransparent,
    TerrainTranslucent,
    PresentationInpAttachment,
    DebugUI,
    DebugUIInpAttachment,
    Time,
}


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
    unsafe fn load_descriptors(&mut self, _: CmdBufContext) -> Vec<RenderData> {Vec::new()}
    unsafe fn destroy(&mut self) {}
}
