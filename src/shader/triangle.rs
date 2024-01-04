use std::rc::Rc;
use ash::{Device, Instance, vk};
use crate::component::RenderData;
use super::{destroy_shader_modules, gen_shader_modules_info, Shader};


pub struct TriangleRasterizer {
    device: Rc<Device>,
    extent: vk::Extent2D,
    pipeline_layout: vk::PipelineLayout,
    renderpass: vk::RenderPass,
    gfxs_pipeline: vk::Pipeline,
}

impl TriangleRasterizer {
    pub(crate) unsafe fn new(inst: Rc<Instance>, phys_devc: vk::PhysicalDevice, device: Rc<Device>, extent: vk::Extent2D, format: vk::Format) -> Self {
        // PIPELINE LAYOUT
        let (shader_stages, shader_modules) = gen_shader_modules_info(
            device.clone(), vec![
                ("C:/Users/andrewshen/documents/matrixagon2/src/shader/triangle.vert.spv", vk::ShaderStageFlags::VERTEX),
                ("C:/Users/andrewshen/documents/matrixagon2/src/shader/triangle.frag.spv", vk::ShaderStageFlags::FRAGMENT),
            ]);

        let dynamic_states = vec![vk::DynamicState::VIEWPORT, vk::DynamicState::SCISSOR];
        let dynamic_state_info = vk::PipelineDynamicStateCreateInfo::builder()
            .dynamic_states(dynamic_states.as_slice())
            .build();

        let vertex_inp_info = vk::PipelineVertexInputStateCreateInfo::builder()
            .vertex_binding_descriptions(&[])
            .vertex_attribute_descriptions(&[])
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
            cull_mode: vk::CullModeFlags::BACK,
            front_face: vk::FrontFace::CLOCKWISE,
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
            blend_enable: vk::FALSE,
            ..Default::default()
        };

        let color_blend_info = vk::PipelineColorBlendStateCreateInfo {
            logic_op_enable: vk::FALSE,
            attachment_count: 1,
            p_attachments: &color_blend_attachement,
            blend_constants: [0.0, 0.0, 0.0, 0.0],
            ..Default::default()
        };

        let pipeline_layout_info = vk::PipelineLayoutCreateInfo {
            ..Default::default()
        };
        let pipeline_layout = device.create_pipeline_layout(&pipeline_layout_info, None).unwrap();

        // RENDER PASSES

        let color_attachment = vk::AttachmentDescription {
            format: format,
            samples: vk::SampleCountFlags::TYPE_1,
            load_op: vk::AttachmentLoadOp::CLEAR,
            store_op: vk::AttachmentStoreOp::STORE,
            stencil_load_op: vk::AttachmentLoadOp::DONT_CARE,
            stencil_store_op: vk::AttachmentStoreOp::DONT_CARE,
            initial_layout: vk::ImageLayout::UNDEFINED,
            final_layout: vk::ImageLayout::PRESENT_SRC_KHR,
            ..Default::default()
        };

        let color_attachment_ref = vk::AttachmentReference {
            attachment: 0,
            layout: vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
        };

        let subpass = vk::SubpassDescription {
            pipeline_bind_point: vk::PipelineBindPoint::GRAPHICS,
            color_attachment_count: 1,
            p_color_attachments: &color_attachment_ref,
            ..Default::default()
        };

        let dependency = vk::SubpassDependency {
            src_subpass: vk::SUBPASS_EXTERNAL,
            dst_subpass: 0,
            src_stage_mask: vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
            src_access_mask: vk::AccessFlags::empty(),
            dst_stage_mask: vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
            dst_access_mask: vk::AccessFlags::COLOR_ATTACHMENT_WRITE,
            ..Default::default()
        };

        let renderpass_info = vk::RenderPassCreateInfo::builder()
            .attachments(&[color_attachment])
            .subpasses(&[subpass])
            .dependencies(&[dependency])
            .build();
        let renderpass = device.create_render_pass(&renderpass_info, None).unwrap();

        // PIPELINE CREATION

        let pipeline_info = vk::GraphicsPipelineCreateInfo {
            stage_count: 2,
            p_stages: shader_stages.as_ptr(),
            p_vertex_input_state: &vertex_inp_info,
            p_input_assembly_state: &input_assembly_info,
            p_viewport_state: &viewport_state_info,
            p_rasterization_state: &rasterizer_info,
            p_multisample_state: &multisampling_info,
            // p_depth_stencil_state
            p_color_blend_state: &color_blend_info,
            p_dynamic_state: &dynamic_state_info,

            layout: pipeline_layout,
            render_pass: renderpass,
            subpass: 0,
            ..Default::default()
        };

        let gfxs_pipeline = device.create_graphics_pipelines(vk::PipelineCache::null(), &[pipeline_info], None).unwrap();

        destroy_shader_modules(device.clone(), shader_modules);

        Self {
            device: device.clone(),
            extent,
            renderpass,
            pipeline_layout,
            gfxs_pipeline: gfxs_pipeline[0],
        }
    }
}

impl Shader for TriangleRasterizer {
    fn renderpass(&self) -> vk::RenderPass {
        self.renderpass
    }

    fn update_extent(&mut self, new_extent: vk::Extent2D) {
        self.extent = new_extent;
    }

    fn recreate_buffer(&mut self, render_data: RenderData) {
    }

    unsafe fn draw_command(&self, cmd_buf: vk::CommandBuffer, framebuffer: vk::Framebuffer) {
        let renderpass_info = vk::RenderPassBeginInfo {
            render_pass: self.renderpass,
            framebuffer,
            render_area: vk::Rect2D {
                offset: vk::Offset2D {x:0, y:0},
                extent: self.extent,
            },
            clear_value_count: 1,
            p_clear_values: &vk::ClearValue {
                color: vk::ClearColorValue {float32: [0.0, 0.0, 0.0, 0.0]}
            },
            ..Default::default()
        };
        self.device.cmd_begin_render_pass(cmd_buf, &renderpass_info, vk::SubpassContents::INLINE);

        self.device.cmd_bind_pipeline(cmd_buf, vk::PipelineBindPoint::GRAPHICS, self.gfxs_pipeline);

        let viewport = vk::Viewport {
            x: 0.0,
            y: 0.0,
            width: self.extent.width as f32,
            height: self.extent.height as f32,
            min_depth: 0.0,
            max_depth: 1.0,
        };
        self.device.cmd_set_viewport(cmd_buf, 0, &[viewport]);

        let scissor = vk::Rect2D {
            offset: vk::Offset2D {x:0,y:0},
            extent: self.extent,
        };
        self.device.cmd_set_scissor(cmd_buf, 0, &[scissor]);

        self.device.cmd_draw(cmd_buf, 3, 1, 0, 0);

        self.device.cmd_end_render_pass(cmd_buf);
    }

    unsafe fn destroy(&self) {
        self.device.destroy_pipeline(self.gfxs_pipeline, None);
        self.device.destroy_pipeline_layout(self.pipeline_layout, None);
        self.device.destroy_render_pass(self.renderpass, None);
    }
}
