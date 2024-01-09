use std::mem;
use std::rc::Rc;
use ash::{Device, vk};
use ash::vk::{CommandBuffer, Extent2D, Framebuffer, RenderPass};
use egui::epaint::Vertex;
use crate::component::RenderData;
use crate::offset_of;
use crate::shader::chunk::ChunkVertex;
use crate::shader::{get_vertex_inp, Shader};


// fn vertex_inp() -> vk::PipelineVertexInputStateCreateInfo {
//     get_vertex_inp::<Vertex>(vec![
//         (vk::Format::R32G32_SFLOAT, offset_of!(Vertex, pos) as u32),
//         (vk::Format::R32G32_SFLOAT, offset_of!(Vertex, uv) as u32),
//         (vk::Format::R8G8B8A8_UNORM, offset_of!(Vertex, color) as u32),
//     ])
// }

pub struct DebugUISubShader {
    device: Rc<Device>,
    extent: vk::Extent2D,
    gfxs_pipeline: vk::Pipeline,

    ui_vbo: Option<(vk::Buffer, vk::DeviceMemory)>,
    ui_ibo: Option<(vk::Buffer, vk::DeviceMemory, u32)>,
}

impl DebugUISubShader {
    // pub(crate) fn new() -> Self {
    //     Self {
    //
    //     }
    // }

    fn recreate_buffer(&mut self, render_data: RenderData) {
        todo!()
    }

    unsafe fn draw_pipeline(&self, cmd_buf: CommandBuffer) {
        todo!()
    }

    unsafe fn destroy(&self) {
        todo!()
    }
}
