use std::mem;
use std::rc::Rc;
use ash::{Device, vk};
use crate::component::{RenderData, RenderDataPurpose};
use crate::offset_of;
use crate::shader::{destroy_shader_modules, gen_shader_modules_info, Shader};

#[derive(Copy, Clone, Debug)]
pub(crate) struct ChunkVertex {
    pub(crate) pos: [f32; 3],
    pub(crate) uv: [f32; 2],
}

impl ChunkVertex {
    fn get_binding_description() -> vk::VertexInputBindingDescription {
        vk::VertexInputBindingDescription {
            binding: 0,
            stride: mem::size_of::<ChunkVertex>() as u32,
            input_rate: vk::VertexInputRate::VERTEX,
            ..Default::default()
        }
    }

    fn get_attribute_description() -> Vec<vk::VertexInputAttributeDescription> {
        vec![
            vk::VertexInputAttributeDescription {
                binding: 0,
                location: 0,
                format: vk::Format::R32G32B32_SFLOAT,
                offset: offset_of!(ChunkVertex, pos) as u32,
            },
            vk::VertexInputAttributeDescription {
                binding: 0,
                location: 1,
                format: vk::Format::R32G32_SFLOAT,
                offset: offset_of!(ChunkVertex, uv) as u32,
            },
        ]
    }
}


pub struct ChunkRasterizer {
    device: Rc<Device>,
    extent: vk::Extent2D,
    pipeline_layout: vk::PipelineLayout,
    renderpass: vk::RenderPass,
    gfxs_pipeline: vk::Pipeline,

    terrain_vbo: Option<(vk::Buffer, vk::DeviceMemory)>,
    terrain_ibo: Option<(vk::Buffer, vk::DeviceMemory, u32)>,

    descriptor_set_layout: vk::DescriptorSetLayout,
    descriptor_pool: vk::DescriptorPool,
    descriptor_set: Vec<vk::DescriptorSet>,

}

impl ChunkRasterizer {
    pub(crate) unsafe fn new(device: Rc<Device>, extent: vk::Extent2D, color_format: vk::Format,
                             depth_format: vk::Format, descriptor_buffers: Vec<RenderData>,) -> Self {
        // DESCRIPTION SET LAYOUT
        let camera_view_proj_layout_binding = vk::DescriptorSetLayoutBinding {
            binding: 0,
            descriptor_count: 1,
            descriptor_type: vk::DescriptorType::UNIFORM_BUFFER,
            p_immutable_samplers: std::ptr::null(),
            stage_flags: vk::ShaderStageFlags::VERTEX,
        };
        let texture_layout_binding = vk::DescriptorSetLayoutBinding {
            binding: 1,
            descriptor_count: 1,
            descriptor_type: vk::DescriptorType::COMBINED_IMAGE_SAMPLER,
            p_immutable_samplers: std::ptr::null(),
            stage_flags: vk::ShaderStageFlags::FRAGMENT,
        };

        // PIPELINE LAYOUT
        let descriptor_set_layout_info = vk::DescriptorSetLayoutCreateInfo::builder()
            .bindings(&[camera_view_proj_layout_binding, texture_layout_binding])
            .build();
        let descriptor_set_layout = device.create_descriptor_set_layout(&descriptor_set_layout_info, None)
            .expect("Failed to create descriptor set layout");

        let pipeline_layout_info = vk::PipelineLayoutCreateInfo::builder()
            .set_layouts(&[descriptor_set_layout])
            .build();
        let pipeline_layout = device.create_pipeline_layout(&pipeline_layout_info, None).unwrap();

        // RENDER PASSES

        // GRAPHICS PIPELINE

        let (shader_stages, shader_modules) = gen_shader_modules_info(
            device.clone(), vec![
                ("C:/Users/andrewshen/documents/matrixagon2/src/shader/chunk.vert", vk::ShaderStageFlags::VERTEX),
                ("C:/Users/andrewshen/documents/matrixagon2/src/shader/chunk.frag", vk::ShaderStageFlags::FRAGMENT),
            ]);

        let binding_descrp = ChunkVertex::get_binding_description();
        let attr_descrp = ChunkVertex::get_attribute_description();


        let vertex_inp_info = vk::PipelineVertexInputStateCreateInfo::builder()
            .vertex_binding_descriptions(&[binding_descrp])
            .vertex_attribute_descriptions(&attr_descrp)
            .build();

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
            cull_mode: vk::CullModeFlags::BACK,
            front_face: vk::FrontFace::COUNTER_CLOCKWISE,
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

        let color_attachment = vk::AttachmentDescription {
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

        let color_attachment_ref = vk::AttachmentReference {
            attachment: 0,
            layout: vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
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

        let depth_attachment_ref = vk::AttachmentReference {
            attachment: 1,
            layout: vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL,
        };

        let subpass = vk::SubpassDescription::builder()
            .pipeline_bind_point(vk::PipelineBindPoint::GRAPHICS)
            .color_attachments(&[color_attachment_ref])
            .depth_stencil_attachment(&depth_attachment_ref)
            .build();

        // {
        //     pipeline_bind_point: vk::PipelineBindPoint::GRAPHICS,
        //     color_attachment_count: 1,
        //     p_color_attachments: &color_attachment_ref,
        //     ..Default::default()
        // };

        let dependency = vk::SubpassDependency {
            src_subpass: vk::SUBPASS_EXTERNAL,
            dst_subpass: 0,
            src_stage_mask: vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT | vk::PipelineStageFlags::EARLY_FRAGMENT_TESTS,
            src_access_mask: vk::AccessFlags::empty(),
            dst_stage_mask: vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT | vk::PipelineStageFlags::EARLY_FRAGMENT_TESTS,
            dst_access_mask: vk::AccessFlags::COLOR_ATTACHMENT_WRITE | vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE,
            ..Default::default()
        };

        let renderpass_info = vk::RenderPassCreateInfo::builder()
            .attachments(&[color_attachment, depth_attachment])
            .subpasses(&[subpass])
            .dependencies(&[dependency])
            .build();
        let renderpass = device.create_render_pass(&renderpass_info, None).unwrap();

        let depth_stencil = vk::PipelineDepthStencilStateCreateInfo {
            depth_test_enable: vk::TRUE,
            depth_write_enable: vk::TRUE,
            depth_compare_op: vk::CompareOp::GREATER,
            depth_bounds_test_enable: vk::FALSE,
            stencil_test_enable: vk::FALSE,
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
            p_depth_stencil_state: &depth_stencil,
            p_color_blend_state: &color_blend_info,
            p_dynamic_state: &dynamic_state_info,

            layout: pipeline_layout,
            render_pass: renderpass,
            subpass: 0,
            ..Default::default()
        };

        let gfxs_pipeline = device.create_graphics_pipelines(vk::PipelineCache::null(), &[pipeline_info], None).unwrap();

        // DESCRIPTOR ALLOCATION

        let camera_view_proj_descriptor_pool_size = vk::DescriptorPoolSize {
            ty: vk::DescriptorType::UNIFORM_BUFFER,
            descriptor_count: 1,
        };
        let texture_descriptor_pool_size = vk::DescriptorPoolSize {
            ty: vk::DescriptorType::COMBINED_IMAGE_SAMPLER,
            descriptor_count: 1,
        };

        let descriptor_pool_info = vk::DescriptorPoolCreateInfo::builder()
            .pool_sizes(&[camera_view_proj_descriptor_pool_size, texture_descriptor_pool_size])
            .max_sets(1)
            .build();

        let descriptor_pool = device.create_descriptor_pool(&descriptor_pool_info, None)
            .expect("Failed to create descriptor pool");

        let descriptor_set_alloc = vk::DescriptorSetAllocateInfo::builder()
            .descriptor_pool(descriptor_pool)
            .set_layouts(&[descriptor_set_layout])
            .build();

        let descriptor_set = device.allocate_descriptor_sets(&descriptor_set_alloc)
            .expect("Failed to allocate descriptor sets");


        println!("DESCRIPTORS {descriptor_buffers:?}");

        for render_data in descriptor_buffers {
            match render_data {
                RenderData::InitialDescriptorBuffer(buf, RenderDataPurpose::CameraViewProjection) => {
                    device.update_descriptor_sets(&[
                        vk::WriteDescriptorSet::builder()
                            .dst_set(descriptor_set[0])
                            .dst_binding(0)
                            .dst_array_element(0)
                            .descriptor_type(vk::DescriptorType::UNIFORM_BUFFER)
                            .buffer_info(&buf.clone())
                            .build()
                    ], &[]);
                },
                RenderData::InitialDescriptorImage(img, RenderDataPurpose::BlockTextures) => {
                    device.update_descriptor_sets(&[
                        vk::WriteDescriptorSet::builder()
                            .dst_set(descriptor_set[0])
                            .dst_binding(1)
                            .dst_array_element(0)
                            .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                            .image_info(&img.clone())
                            .build()
                    ], &[]);
                },
                _ => {},
            }
        }

        destroy_shader_modules(device.clone(), shader_modules);

        Self {
            device: device.clone(),
            extent,
            renderpass,
            pipeline_layout,
            gfxs_pipeline: gfxs_pipeline[0],

            terrain_vbo: None,
            terrain_ibo: None,

            descriptor_set_layout,
            descriptor_pool,
            descriptor_set,
        }
    }
}

impl Shader for ChunkRasterizer {
    fn renderpass(&self) -> vk::RenderPass {
        self.renderpass
    }

    fn update_extent(&mut self, new_extent: vk::Extent2D) {
        self.extent = new_extent;
    }

    fn recreate_buffer(&mut self, render_data: RenderData) {
        match render_data {
            RenderData::RecreateVertexBuffer(buf, mem, RenderDataPurpose::TerrainVertices) => unsafe {
                println!("RECREATE VBO");
                if let Some((old_buf, old_mem)) = self.terrain_vbo {
                    println!("DEL OLD VBO");
                    self.device.device_wait_idle().unwrap();
                    self.device.destroy_buffer(old_buf, None);
                    self.device.free_memory(old_mem, None);
                }
                self.terrain_vbo = Some((buf, mem));
            }
            RenderData::RecreateIndexBuffer(buf, mem, len, RenderDataPurpose::TerrainVertices) => unsafe {
                println!("RECREATE IBO");
                if let Some((old_buf, old_mem, _)) = self.terrain_ibo {
                    println!("DEL OLD IBO");
                    self.device.device_wait_idle().unwrap();
                    self.device.destroy_buffer(old_buf, None);
                    self.device.free_memory(old_mem, None);
                }
                self.terrain_ibo = Some((buf, mem, len));
            }
            _ => {},
        }
    }

    unsafe fn draw_command(&self, cmd_buf: vk::CommandBuffer, framebuffer: vk::Framebuffer) {
        if self.terrain_vbo.is_none() || self.terrain_ibo.is_none() {
            println!("Cube shader cannot draw due to vertex and index buffer not created.");
            return;
        }

        let renderpass_info = vk::RenderPassBeginInfo::builder()
            .render_pass(self.renderpass)
            .framebuffer(framebuffer)
            .render_area(vk::Rect2D { offset: vk::Offset2D {x:0, y:0}, extent: self.extent})
            .clear_values(&[
                vk::ClearValue { color: vk::ClearColorValue {float32: [0.2, 0.3, 0.9, 1.0]} },
                vk::ClearValue { color: vk::ClearColorValue {float32: [0.0, 0.0, 0.0, 0.0]} },
            ])
            .build();

        self.device.cmd_begin_render_pass(cmd_buf, &renderpass_info, vk::SubpassContents::INLINE);

        self.device.cmd_bind_pipeline(cmd_buf, vk::PipelineBindPoint::GRAPHICS, self.gfxs_pipeline);

        self.device.cmd_bind_vertex_buffers(cmd_buf, 0, &[self.terrain_vbo.unwrap().0], &[0]);
        self.device.cmd_bind_index_buffer(cmd_buf, self.terrain_ibo.unwrap().0, 0, vk::IndexType::UINT32);

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

        self.device.cmd_bind_descriptor_sets(cmd_buf, vk::PipelineBindPoint::GRAPHICS, self.pipeline_layout, 0, &self.descriptor_set, &[]);

        self.device.cmd_draw_indexed(cmd_buf, self.terrain_ibo.unwrap().2, 1, 0, 0, 0);
        // self.device.cmd_draw(cmd_buf, self.vertices.len() as u32, 1, 0, 0);

        self.device.cmd_end_render_pass(cmd_buf);
    }

    unsafe fn destroy(&self) {
        self.device.destroy_descriptor_pool(self.descriptor_pool, None);
        self.device.destroy_descriptor_set_layout(self.descriptor_set_layout, None);

        if let Some((old_buf, old_mem)) = self.terrain_vbo {
            self.device.destroy_buffer(old_buf, None);
            self.device.free_memory(old_mem, None);
        }
        if let Some((old_buf, old_mem, _)) = self.terrain_ibo {
            self.device.destroy_buffer(old_buf, None);
            self.device.free_memory(old_mem, None);
        }

        self.device.destroy_pipeline(self.gfxs_pipeline, None);
        self.device.destroy_pipeline_layout(self.pipeline_layout, None);
        self.device.destroy_render_pass(self.renderpass, None);
    }
}
