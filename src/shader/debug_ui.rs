use std::mem;
use std::rc::Rc;
use ash::{Device, vk};
use egui::epaint::Vertex;
use crate::component::RenderData;
use crate::{get_vertex_inp, offset_of};
use crate::shader::chunk::ChunkVertex;
use crate::shader::{DescriptorManager, gen_shader_modules_info, get_vertex_inp, Shader};


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

    pub(crate) ui_vbo: Option<(vk::Buffer, vk::DeviceMemory)>,
    pub(crate) ui_ibo: Option<(vk::Buffer, vk::DeviceMemory, u32)>,
    pub(crate) scissor: Option<vk::Rect2D>,
}

impl DebugUISubShader {
    pub(crate) unsafe fn new(device: Rc<Device>, extent: vk::Extent2D, pipeline_layout: vk::PipelineLayout, renderpass: vk::RenderPass) -> Self {
        // GRAPHICS PIPELINE

        let (shader_stages, shader_modules) = gen_shader_modules_info(
            device.clone(), vec![
                ("C:/Users/andrewshen/documents/matrixagon2/src/shader/debug_ui.vert", vk::ShaderStageFlags::VERTEX),
                ("C:/Users/andrewshen/documents/matrixagon2/src/shader/debug_ui.frag", vk::ShaderStageFlags::FRAGMENT),
            ]);

        let vertex_inp_info = get_vertex_inp!(Vertex;
            (vk::Format::R32G32_SFLOAT, pos),
            (vk::Format::R32G32_SFLOAT, uv),
            (vk::Format::R8G8B8A8_UNORM, color)
        );

        let dynamic_states = vec![vk::DynamicState::VIEWPORT, vk::DynamicState::SCISSOR];
        let dynamic_state_info = vk::PipelineDynamicStateCreateInfo::builder()
            .dynamic_states(dynamic_states.as_slice())
            .build();

        let input_assembly_info = vk::PipelineInputAssemblyStateCreateInfo {
            topology: vk::PrimitiveTopology::TRIANGLE_LIST,
            primitive_restart_enable: vk::FALSE,
            ..Default::default()
        };

        let viewport_state_info = vk::PipelineViewportStateCreateInfo {
            viewport_count: 1,
            scissor_count: 1,
            ..Default::default()
        };

        let rasterizer_info = vk::PipelineRasterizationStateCreateInfo {
            depth_clamp_enable: vk::FALSE,
            rasterizer_discard_enable: vk::FALSE,
            polygon_mode: vk::PolygonMode::FILL,
            line_width: 1.0,
            cull_mode: vk::CullModeFlags::NONE,
            depth_bias_enable: vk::FALSE,
            ..Default::default()
        };

        let multisampling_info = vk::PipelineMultisampleStateCreateInfo {
            sample_shading_enable: vk::FALSE,
            rasterization_samples: vk::SampleCountFlags::TYPE_1,
            ..Default::default()
        };

        let color_blend_attachement = vk::PipelineColorBlendAttachmentState {
            color_write_mask: vk::ColorComponentFlags::R | vk::ColorComponentFlags::G |
                vk::ColorComponentFlags::B | vk::ColorComponentFlags::A,
            blend_enable: vk::TRUE,
            src_color_blend_factor: vk::BlendFactor::SRC_ALPHA,
            dst_color_blend_factor: vk::BlendFactor::ONE_MINUS_SRC_ALPHA,
            color_blend_op: vk::BlendOp::ADD,
            src_alpha_blend_factor: vk::BlendFactor::SRC_ALPHA,
            dst_alpha_blend_factor: vk::BlendFactor::ONE_MINUS_SRC_ALPHA,
            alpha_blend_op: vk::BlendOp::ADD,
            ..Default::default()
        };

        let color_blend_info = vk::PipelineColorBlendStateCreateInfo {
            logic_op_enable: vk::FALSE,
            attachment_count: 1,
            p_attachments: &color_blend_attachement,
            blend_constants: [0.0, 0.0, 0.0, 0.0],
            ..Default::default()
        };

        // PIPELINE CREATION

        let pipeline_info = vk::GraphicsPipelineCreateInfo {
            stage_count: 2,
            p_stages: shader_stages.as_ptr(),
            p_vertex_input_state: &vertex_inp_info,
            p_input_assembly_state: &input_assembly_info,
            p_viewport_state: &viewport_state_info,
            p_rasterization_state: &rasterizer_info,
            p_multisample_state: &multisampling_info,
            // p_depth_stencil_state: &depth_stencil,
            p_color_blend_state: &color_blend_info,
            p_dynamic_state: &dynamic_state_info,

            layout: pipeline_layout,
            render_pass: renderpass,
            subpass: 1,
            ..Default::default()
        };

        let gfxs_pipeline = device.create_graphics_pipelines(vk::PipelineCache::null(), &[pipeline_info], None).unwrap();

        for module in shader_modules {
            device.destroy_shader_module(module, None);
        }

        Self {
            device: device.clone(),
            extent,
            gfxs_pipeline: gfxs_pipeline[0],

            ui_vbo: None, ui_ibo: None, scissor: None,
        }
    }

    pub(crate) fn update_extent(&mut self, new_extent: vk::Extent2D) {
        self.extent = new_extent;
    }

    fn recreate_buffer(&mut self, render_data: RenderData) {
        // TODO: should use this method instead of directly integrating it into chunk.rs
    }

    pub(crate) unsafe fn draw_pipeline(&self, cmd_buf: vk::CommandBuffer, descriptor: &DescriptorManager) {
        self.device.cmd_next_subpass(cmd_buf, vk::SubpassContents::INLINE);

        self.device.cmd_bind_vertex_buffers(cmd_buf, 0, &[self.ui_vbo.unwrap().0], &[0]);
        self.device.cmd_bind_index_buffer(cmd_buf, self.ui_ibo.unwrap().0, 0, vk::IndexType::UINT32);

        let viewport = vk::Viewport {
            x: 0.0,
            y: 0.0,
            width: self.extent.width as f32,
            height: self.extent.height as f32,
            min_depth: 0.0,
            max_depth: 1.0,
        };
        self.device.cmd_set_viewport(cmd_buf, 0, &[viewport]);

        // self.device.cmd_set_scissor(cmd_buf, 0, &[self.scissor.unwrap()]);

        self.device.cmd_bind_pipeline(cmd_buf, vk::PipelineBindPoint::GRAPHICS, self.gfxs_pipeline);
        self.device.cmd_bind_descriptor_sets(cmd_buf, vk::PipelineBindPoint::GRAPHICS, descriptor.pipeline_layout(),
                                             1, &descriptor.descriptor_sets(&[1]), &[]);
        self.device.cmd_draw_indexed(cmd_buf, self.ui_ibo.unwrap().2, 1, 0, 0, 0);
    }

    pub(crate) unsafe fn destroy(&self) {
        if let Some((old_buf, old_mem)) = self.ui_vbo {
            self.device.destroy_buffer(old_buf, None);
            self.device.free_memory(old_mem, None);
        }
        if let Some((old_buf, old_mem, _)) = self.ui_ibo {
            self.device.destroy_buffer(old_buf, None);
            self.device.free_memory(old_mem, None);
        }

        self.device.destroy_pipeline(self.gfxs_pipeline, None);
    }
}
