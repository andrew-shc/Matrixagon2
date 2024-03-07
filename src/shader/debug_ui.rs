use std::mem;
use std::rc::Rc;
use ash::{Device, vk};
// use crate::{vertex_input};
use crate::shader::{ColorBlendKind, standard_graphics_pipeline, StandardGraphicsPipelineInfo, VBOFS};
use matrixagon_util::{Vertex, VulkanVertexState};


// emulating the structure of the EguiVertex
#[derive(Copy, Clone, Debug, Vertex)]
pub struct EguiVertex {
    pub(crate) pos: [f32; 2],
    pub(crate) uv: [f32; 2],
    pub(crate) color: [u8; 3],
}


pub struct DebugUISubShader {
    device: Rc<Device>,
    gfxs_pipeline: vk::Pipeline,

    pub(crate) ui_vbo: Option<([vk::Buffer; 1], vk::DeviceMemory)>,
    pub(crate) ui_ibo: Option<(vk::Buffer, vk::DeviceMemory, u32)>,
    pub(crate) scissor: Option<vk::Rect2D>,
}

impl DebugUISubShader {
    pub(crate) unsafe fn new(device: Rc<Device>, pipeline_layout: vk::PipelineLayout, renderpass: vk::RenderPass) -> Self {
        // GRAPHICS PIPELINE

        let graphics_pipelines = standard_graphics_pipeline(
            device.clone(),
            vec![
                StandardGraphicsPipelineInfo {
                    shaders: vec![
                        ("C:/Users/andrewshen/documents/matrixagon2/src/shader/debug_ui.vert", vk::ShaderStageFlags::VERTEX),
                        ("C:/Users/andrewshen/documents/matrixagon2/src/shader/debug_ui.frag", vk::ShaderStageFlags::FRAGMENT),
                    ],
                    vertex_input_state: EguiVertex::VERTEX_INPUT_STATE,
                    back_face_culling: false, depth_testing: false,
                    color_blend_attachment_state: vec![ColorBlendKind::transparent()],
                    subpass_index: 1,
                },
            ],
            pipeline_layout, renderpass,
        );

        Self {
            device: device.clone(),
            gfxs_pipeline: graphics_pipelines[0],

            ui_vbo: None, ui_ibo: None, scissor: None,
        }
    }

    pub(crate) unsafe fn draw_pipeline(&self, cmd_buf: vk::CommandBuffer) {
        self.device.cmd_next_subpass(cmd_buf, vk::SubpassContents::INLINE);

        if let (Some((ui_vbo, _)), Some(ui_ibo)) = (self.ui_vbo, self.ui_ibo) {
            self.device.cmd_bind_vertex_buffers(cmd_buf, 0, &ui_vbo, &VBOFS);
            self.device.cmd_bind_index_buffer(cmd_buf, ui_ibo.0, 0, vk::IndexType::UINT32);

            self.device.cmd_bind_pipeline(cmd_buf, vk::PipelineBindPoint::GRAPHICS, self.gfxs_pipeline);
            self.device.cmd_draw_indexed(cmd_buf, ui_ibo.2, 1, 0, 0, 0);
        }
    }

    pub(crate) unsafe fn destroy(&self) {
        if let Some((old_buf, old_mem)) = self.ui_vbo {
            self.device.destroy_buffer(old_buf[0], None);
            self.device.free_memory(old_mem, None);
        }
        if let Some((old_buf, old_mem, _)) = self.ui_ibo {
            self.device.destroy_buffer(old_buf, None);
            self.device.free_memory(old_mem, None);
        }

        self.device.destroy_pipeline(self.gfxs_pipeline, None);
    }
}
