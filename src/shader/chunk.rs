use std::mem;
use std::rc::Rc;
use ash::{Device, vk};
use crate::component::{RenderData, RenderDataPurpose};
use crate::framebuffer::AttachmentRef;
use crate::{get_vertex_inp};
use crate::shader::{DescriptorManager, destroy_shader_modules, gen_shader_modules_info, Shader};
use crate::shader::debug_ui::DebugUISubShader;

#[derive(Copy, Clone, Debug)]
pub(crate) struct ChunkVertex {
    pub(crate) pos: [f32; 3],
    pub(crate) uv: [f32; 2],
    pub(crate) txtr: f32,
}


pub struct ChunkRasterizer {
    device: Rc<Device>,
    extent: vk::Extent2D,
    renderpass: vk::RenderPass,
    gfxs_pipeline: vk::Pipeline,
    transparent_gfxs_pipeline: vk::Pipeline,

    terrain_vbo: Option<(vk::Buffer, vk::DeviceMemory)>,
    terrain_ibo: Option<(vk::Buffer, vk::DeviceMemory, u32)>,
    transparent_vbo: Option<(vk::Buffer, vk::DeviceMemory)>,
    transparent_ibo: Option<(vk::Buffer, vk::DeviceMemory, u32)>,

    descriptor: DescriptorManager,

    debug_ui_sub_shader: DebugUISubShader,
}

impl ChunkRasterizer {
    pub(crate) unsafe fn new(device: Rc<Device>, extent: vk::Extent2D, color_format: vk::Format,
                             depth_format: vk::Format) -> Self {
        let descriptor = DescriptorManager::new(device.clone(), vec![
            vec![  // set 0 for shader
                (vk::DescriptorType::UNIFORM_BUFFER, vk::ShaderStageFlags::VERTEX),  // proj-view
                (vk::DescriptorType::COMBINED_IMAGE_SAMPLER, vk::ShaderStageFlags::FRAGMENT),  // textures
            ],
            vec![  // set 1 for ui
                (vk::DescriptorType::COMBINED_IMAGE_SAMPLER, vk::ShaderStageFlags::FRAGMENT), // egui debug ui texture
                (vk::DescriptorType::INPUT_ATTACHMENT, vk::ShaderStageFlags::FRAGMENT), // input attachment from previous
            ],
            // vec![  // set 2 for transparency
            //     (vk::DescriptorType::INPUT_ATTACHMENT, vk::ShaderStageFlags::FRAGMENT), // input attachment from main color shader
            // ],
        ]);

        // GRAPHICS PIPELINE

        let (shader_stages, shader_modules) = gen_shader_modules_info(
            device.clone(), vec![
                ("C:/Users/andrewshen/documents/matrixagon2/src/shader/chunk.vert", vk::ShaderStageFlags::VERTEX),
                ("C:/Users/andrewshen/documents/matrixagon2/src/shader/chunk.frag", vk::ShaderStageFlags::FRAGMENT),
            ]);

        let (transparent_shader_stages, transparent_shader_modules) = gen_shader_modules_info(
            device.clone(), vec![
                ("C:/Users/andrewshen/documents/matrixagon2/src/shader/chunk.vert", vk::ShaderStageFlags::VERTEX),
                ("C:/Users/andrewshen/documents/matrixagon2/src/shader/chunk_transparent.frag", vk::ShaderStageFlags::FRAGMENT),
            ]);

        let vertex_inp_info = get_vertex_inp!(ChunkVertex;
            (vk::Format::R32G32B32_SFLOAT, pos),
            (vk::Format::R32G32_SFLOAT, uv),
            (vk::Format::R32_SFLOAT, txtr)
        );

        let dynamic_states = vec![vk::DynamicState::VIEWPORT, vk::DynamicState::SCISSOR];  // vk::DynamicState::CULL_MODE
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
            cull_mode: vk::CullModeFlags::BACK,
            front_face: vk::FrontFace::COUNTER_CLOCKWISE,
            depth_bias_enable: vk::FALSE,
            ..Default::default()
        };

        let transparent_rasterizer_info = vk::PipelineRasterizationStateCreateInfo {
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

        let depth_stencil = vk::PipelineDepthStencilStateCreateInfo {
            depth_test_enable: vk::TRUE,
            depth_write_enable: vk::TRUE,
            depth_compare_op: vk::CompareOp::GREATER,
            depth_bounds_test_enable: vk::FALSE,
            stencil_test_enable: vk::FALSE,
            ..Default::default()
        };

        // SUBPASS & ATTACHMENTS

        // TODO: DEBUG UI SENSITIVE
        let prsnt_attachment = vk::AttachmentDescription {
            format: color_format,
            samples: vk::SampleCountFlags::TYPE_1,
            load_op: vk::AttachmentLoadOp::CLEAR,
            store_op: vk::AttachmentStoreOp::STORE,
            stencil_load_op: vk::AttachmentLoadOp::DONT_CARE,
            stencil_store_op: vk::AttachmentStoreOp::DONT_CARE,
            initial_layout: vk::ImageLayout::UNDEFINED,
            final_layout: vk::ImageLayout::PRESENT_SRC_KHR,
            ..Default::default()
        };

        let depth_attachment = vk::AttachmentDescription {
            format: depth_format,
            samples: vk::SampleCountFlags::TYPE_1,
            load_op: vk::AttachmentLoadOp::CLEAR,
            store_op: vk::AttachmentStoreOp::DONT_CARE,
            stencil_load_op: vk::AttachmentLoadOp::DONT_CARE,
            stencil_store_op: vk::AttachmentStoreOp::DONT_CARE,
            initial_layout: vk::ImageLayout::UNDEFINED,
            final_layout: vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL,
            ..Default::default()
        };

        let prsnt_attachment_ref = vk::AttachmentReference { attachment: 0, layout: vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL };
        let depth_attachment_ref = vk::AttachmentReference { attachment: 1, layout: vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL };
        let block_subpass = vk::SubpassDescription::builder()
            .pipeline_bind_point(vk::PipelineBindPoint::GRAPHICS)
            .color_attachments(&[prsnt_attachment_ref])
            .depth_stencil_attachment(&depth_attachment_ref)
            .build();
        //
        // let inp_prsnt_attachment_ref = vk::AttachmentReference { attachment: 0, layout: vk::ImageLayout::GENERAL };
        // let prsnt_attachment_ref = vk::AttachmentReference { attachment: 0, layout: vk::ImageLayout::GENERAL };
        // let depth_attachment_ref = vk::AttachmentReference { attachment: 1, layout: vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL };
        // let transparency_subpass = vk::SubpassDescription::builder()
        //     .pipeline_bind_point(vk::PipelineBindPoint::GRAPHICS)
        //     .input_attachments(&[inp_prsnt_attachment_ref])
        //     .color_attachments(&[prsnt_attachment_ref])
        //     .depth_stencil_attachment(&depth_attachment_ref)
        //     .build();

        let inp_prsnt_attachment_ref = vk::AttachmentReference { attachment: 0, layout: vk::ImageLayout::GENERAL };
        let prsnt_attachment_ref = vk::AttachmentReference { attachment: 0, layout: vk::ImageLayout::GENERAL };
        let comp_subpass = vk::SubpassDescription::builder()
            .pipeline_bind_point(vk::PipelineBindPoint::GRAPHICS)
            .input_attachments(&[inp_prsnt_attachment_ref])
            .color_attachments(&[prsnt_attachment_ref])
            .build();

        let dependency = vk::SubpassDependency {
            src_subpass: vk::SUBPASS_EXTERNAL,
            dst_subpass: 0,
            src_stage_mask: vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT | vk::PipelineStageFlags::EARLY_FRAGMENT_TESTS,
            dst_stage_mask: vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT | vk::PipelineStageFlags::EARLY_FRAGMENT_TESTS,
            src_access_mask: vk::AccessFlags::empty(),
            dst_access_mask: vk::AccessFlags::COLOR_ATTACHMENT_WRITE | vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE,
            ..Default::default()
        };

        // let transparency_dependency = vk::SubpassDependency {
        //     src_subpass: 0,
        //     dst_subpass: 1,
        //     src_stage_mask: vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
        //     dst_stage_mask: vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
        //     src_access_mask: vk::AccessFlags::COLOR_ATTACHMENT_WRITE | vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE,
        //     dst_access_mask: vk::AccessFlags::COLOR_ATTACHMENT_WRITE | vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE,
        //     ..Default::default()
        // };

        let comp_dependency = vk::SubpassDependency {
            src_subpass: 0,
            dst_subpass: 1,
            src_stage_mask: vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
            dst_stage_mask: vk::PipelineStageFlags::FRAGMENT_SHADER,
            src_access_mask: vk::AccessFlags::COLOR_ATTACHMENT_WRITE,
            dst_access_mask: vk::AccessFlags::INPUT_ATTACHMENT_READ,
            ..Default::default()
        };

        let renderpass_info = vk::RenderPassCreateInfo::builder()
            .attachments(&[prsnt_attachment, depth_attachment])
            .subpasses(&[block_subpass, comp_subpass])
            .dependencies(&[dependency, comp_dependency])
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
            p_depth_stencil_state: &depth_stencil,
            p_color_blend_state: &color_blend_info,
            p_dynamic_state: &dynamic_state_info,

            layout: descriptor.pipeline_layout,
            render_pass: renderpass,
            subpass: 0,
            ..Default::default()
        };

        let transparent_pipeline_info = vk::GraphicsPipelineCreateInfo {
            stage_count: 2,
            p_stages: transparent_shader_stages.as_ptr(),
            p_vertex_input_state: &vertex_inp_info,
            p_input_assembly_state: &input_assembly_info,
            p_viewport_state: &viewport_state_info,
            p_rasterization_state: &transparent_rasterizer_info,
            p_multisample_state: &multisampling_info,
            p_depth_stencil_state: &depth_stencil,
            p_color_blend_state: &color_blend_info,
            p_dynamic_state: &dynamic_state_info,

            layout: descriptor.pipeline_layout,
            render_pass: renderpass,
            subpass: 0,
            ..Default::default()
        };

        let gfxs_pipeline = device.create_graphics_pipelines(vk::PipelineCache::null(), &[pipeline_info, transparent_pipeline_info], None).unwrap();

        destroy_shader_modules(device.clone(), shader_modules);
        destroy_shader_modules(device.clone(), transparent_shader_modules);

        Self {
            // transparency_sub_shader: ChunkTransparencyRasterizerSubShader::new(),
            // TODO: DEBUG UI SENSITIVE
            debug_ui_sub_shader: DebugUISubShader::new(device.clone(), extent, descriptor.pipeline_layout, renderpass),

            device: device.clone(),
            extent,
            renderpass,
            gfxs_pipeline: gfxs_pipeline[0],
            transparent_gfxs_pipeline: gfxs_pipeline[1],
            terrain_vbo: None, terrain_ibo: None,
            transparent_vbo: None, transparent_ibo: None,
            descriptor,
        }
    }
}

impl Shader for ChunkRasterizer {
    fn renderpass(&self) -> vk::RenderPass {
        self.renderpass
    }

    fn attachments(&self) -> Vec<AttachmentRef> {
        vec![  // TODO: DEBUG UI SENSITIVE
            AttachmentRef::Depth,
        ]
    }

    unsafe fn write_descriptors(&mut self, descriptor_buffers: Vec<RenderData>) {
        for render_data in descriptor_buffers {
            match render_data {
                RenderData::InitialDescriptorBuffer(buf, RenderDataPurpose::CameraViewProjection) => {
                    self.descriptor.write_buffer(0, 0, buf);
                },
                RenderData::InitialDescriptorImage(img, RenderDataPurpose::BlockTextures) => {
                    self.descriptor.write_image(0, 1, img);
                },
                // TODO: DEBUG UI SENSITIVE
                RenderData::InitialDescriptorImage(img, RenderDataPurpose::DebugUI) => {
                    self.descriptor.write_image(1, 0, img);  // egui debug ui textures
                }
                RenderData::InitialDescriptorImage(img, RenderDataPurpose::PresentationInpAttachment) => {
                    self.descriptor.write_image(1, 1, img);
                }
                _ => {},
            }
        }
    }

    fn update_extent(&mut self, new_extent: vk::Extent2D) {
        self.extent = new_extent;
        self.debug_ui_sub_shader.update_extent(new_extent);
    }

    fn recreate_buffer(&mut self, render_data: RenderData) {
        match render_data {
            RenderData::RecreateVertexBuffer(buf, mem, RenderDataPurpose::TerrainVertices) => unsafe {
                println!("RECREATE [OPAQUE/DEFAULT] VERTEX BUFFER");
                if let Some((old_buf, old_mem)) = self.terrain_vbo {
                    self.device.device_wait_idle().unwrap();
                    self.device.destroy_buffer(old_buf, None);
                    self.device.free_memory(old_mem, None);
                }
                self.terrain_vbo = Some((buf, mem));
            }
            RenderData::RecreateIndexBuffer(buf, mem, len, RenderDataPurpose::TerrainVertices) => unsafe {
                println!("RECREATE [OPAQUE/DEFAULT] INDEX BUFFER");
                if let Some((old_buf, old_mem, _)) = self.terrain_ibo {
                    self.device.device_wait_idle().unwrap();
                    self.device.destroy_buffer(old_buf, None);
                    self.device.free_memory(old_mem, None);
                }
                self.terrain_ibo = Some((buf, mem, len));
            }
            RenderData::RecreateVertexBuffer(buf, mem, RenderDataPurpose::TransparentVertices) => unsafe {
                println!("RECREATE [TRANSPARENT] VERTEX BUFFER");
                if let Some((old_buf, old_mem)) = self.transparent_vbo {
                    self.device.device_wait_idle().unwrap();
                    self.device.destroy_buffer(old_buf, None);
                    self.device.free_memory(old_mem, None);
                }
                self.transparent_vbo = Some((buf, mem));
            }
            RenderData::RecreateIndexBuffer(buf, mem, len, RenderDataPurpose::TransparentVertices) => unsafe {
                println!("RECREATE [TRANSPARENT] INDEX BUFFER");
                if let Some((old_buf, old_mem, _)) = self.transparent_ibo {
                    self.device.device_wait_idle().unwrap();
                    self.device.destroy_buffer(old_buf, None);
                    self.device.free_memory(old_mem, None);
                }
                self.transparent_ibo = Some((buf, mem, len));
            }
            // TODO: DEBUG UI SENSITIVE
            RenderData::RecreateVertexBuffer(buf, mem, RenderDataPurpose::DebugUI) => unsafe {
                // println!("RECREATE [DEBUG UI] VERTEX BUFFER");
                if let Some((old_buf, old_mem)) = self.debug_ui_sub_shader.ui_vbo {
                    self.device.device_wait_idle().unwrap();
                    self.device.destroy_buffer(old_buf, None);
                    self.device.free_memory(old_mem, None);
                }
                self.debug_ui_sub_shader.ui_vbo = Some((buf, mem));
            }
            RenderData::RecreateIndexBuffer(buf, mem, len, RenderDataPurpose::DebugUI) => unsafe {
                // println!("RECREATE [DEBUG UI] INDEX BUFFER");
                if let Some((old_buf, old_mem, _)) = self.debug_ui_sub_shader.ui_ibo {
                    self.device.device_wait_idle().unwrap();
                    self.device.destroy_buffer(old_buf, None);
                    self.device.free_memory(old_mem, None);
                }
                self.debug_ui_sub_shader.ui_ibo  = Some((buf, mem, len));
            }
            RenderData::SetScissorDynamicState(scissor, RenderDataPurpose::DebugUI) => unsafe {
                self.debug_ui_sub_shader.scissor.replace(scissor);
            }
            _ => {},
        }
    }

    unsafe fn draw_command(&self, cmd_buf: vk::CommandBuffer, framebuffer: vk::Framebuffer) {
        if let (Some(terrain_vbo), Some(terrain_ibo), Some(transparent_vbo), Some(transparent_ibo))
            = (self.terrain_vbo, self.terrain_ibo, self.transparent_vbo, self.transparent_ibo) {
            let renderpass_info = vk::RenderPassBeginInfo::builder()
                .render_pass(self.renderpass)
                .framebuffer(framebuffer)
                .render_area(vk::Rect2D { offset: vk::Offset2D {x:0, y:0}, extent: self.extent})
                .clear_values(&[
                    // TODO: DEBUG UI SENSITIVE
                    vk::ClearValue { color: vk::ClearColorValue {float32: [0.2, 0.3, 0.9, 1.0]} },
                    vk::ClearValue { color: vk::ClearColorValue {float32: [0.0, 0.0, 0.0, 0.0]} },
                ])
                .build();

            self.device.cmd_begin_render_pass(cmd_buf, &renderpass_info, vk::SubpassContents::INLINE);

            let viewport = vk::Viewport {
                x: 0.0,
                y: 0.0,
                width: self.extent.width as f32,
                height: self.extent.height as f32,
                min_depth: 0.0,
                max_depth: 1.0,
            };
            let scissor = vk::Rect2D { offset: vk::Offset2D {x:0,y:0}, extent: self.extent };
            self.device.cmd_set_viewport(cmd_buf, 0, &[viewport]);
            self.device.cmd_set_scissor(cmd_buf, 0, &[scissor]);

            self.device.cmd_bind_descriptor_sets(cmd_buf, vk::PipelineBindPoint::GRAPHICS, self.descriptor.pipeline_layout(),
                                                 0, &self.descriptor.descriptor_sets(&[0]), &[]);

            self.device.cmd_bind_pipeline(cmd_buf, vk::PipelineBindPoint::GRAPHICS, self.gfxs_pipeline);

            // opaque objects
            // self.device.cmd_set_cull_mode(cmd_buf, vk::CullModeFlags::BACK);
            self.device.cmd_bind_vertex_buffers(cmd_buf, 0, &[terrain_vbo.0], &[0]);
            self.device.cmd_bind_index_buffer(cmd_buf, terrain_ibo.0, 0, vk::IndexType::UINT32);
            self.device.cmd_draw_indexed(cmd_buf, terrain_ibo.2, 1, 0, 0, 0);
            // transparent objects
            // self.device.cmd_set_cull_mode(cmd_buf, vk::CullModeFlags::NONE);
            self.device.cmd_bind_pipeline(cmd_buf, vk::PipelineBindPoint::GRAPHICS, self.transparent_gfxs_pipeline);
            self.device.cmd_bind_vertex_buffers(cmd_buf, 0, &[transparent_vbo.0], &[0]);
            self.device.cmd_bind_index_buffer(cmd_buf, transparent_ibo.0, 0, vk::IndexType::UINT32);
            self.device.cmd_draw_indexed(cmd_buf, transparent_ibo.2, 1, 0, 0, 0);

            self.debug_ui_sub_shader.draw_pipeline(cmd_buf, &self.descriptor);

            self.device.cmd_end_render_pass(cmd_buf);
        } else {
            println!("Cube shader cannot draw due to vertex and index buffer not created.");
        }
    }

    unsafe fn destroy(&self) {
        self.debug_ui_sub_shader.destroy();

        if let Some((old_buf, old_mem)) = self.terrain_vbo {
            self.device.destroy_buffer(old_buf, None);
            self.device.free_memory(old_mem, None);
        }
        if let Some((old_buf, old_mem, _)) = self.terrain_ibo {
            self.device.destroy_buffer(old_buf, None);
            self.device.free_memory(old_mem, None);
        }
        if let Some((old_buf, old_mem)) = self.transparent_vbo {
            self.device.destroy_buffer(old_buf, None);
            self.device.free_memory(old_mem, None);
        }
        if let Some((old_buf, old_mem, _)) = self.transparent_ibo {
            self.device.destroy_buffer(old_buf, None);
            self.device.free_memory(old_mem, None);
        }

        self.device.destroy_pipeline(self.gfxs_pipeline, None);
        self.device.destroy_pipeline(self.transparent_gfxs_pipeline, None);
        self.descriptor.destroy();
        self.device.destroy_render_pass(self.renderpass, None);
    }
}
