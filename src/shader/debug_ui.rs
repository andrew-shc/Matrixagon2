use std::mem;
use std::rc::Rc;
use ash::{Device, vk};
// use crate::{vertex_input};
use crate::shader::{ColorBlendKind, standard_graphics_pipeline, StandardGraphicsPipelineInfo, VBOFS};
use matrixagon_util::{IndexedBuffer, Vertex, VulkanVertexState};


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

    pub(crate) ui_ivbo: IndexedBuffer,
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

            ui_ivbo: IndexedBuffer::new(device.clone()), scissor: None,
        }
    }

    pub(crate) unsafe fn draw_pipeline(&self, cmd_buf: vk::CommandBuffer) {
        self.device.cmd_next_subpass(cmd_buf, vk::SubpassContents::INLINE);

        if let Some(scissor) = self.scissor {
            let scissors = [scissor];
            self.device.cmd_set_scissor(cmd_buf, 0, &scissors);
        }

        if let Some((ui_vbo, ui_ibo, ibo_len)) = self.ui_ivbo.obtain_indexed_vbo() {
            self.device.cmd_bind_pipeline(cmd_buf, vk::PipelineBindPoint::GRAPHICS, self.gfxs_pipeline);
            self.device.cmd_bind_vertex_buffers(cmd_buf, 0, &ui_vbo, &VBOFS);
            self.device.cmd_bind_index_buffer(cmd_buf, ui_ibo, 0, vk::IndexType::UINT32);
            self.device.cmd_draw_indexed(cmd_buf, ibo_len, 1, 0, 0, 0);
        }
    }

    pub(crate) unsafe fn destroy(&self) {
        self.ui_ivbo.destroy();

        self.device.destroy_pipeline(self.gfxs_pipeline, None);
    }
}
