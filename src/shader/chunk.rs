use std::mem;
use std::rc::Rc;
use ash::{Device, vk};
use crate::component::{RenderData, RenderDataPurpose};
use crate::framebuffer::FBAttachmentRef;
use crate::{get_vertex_inp};
use crate::shader::{DescriptorManager, destroy_shader_modules, gen_shader_modules_info, Shader, standard_graphics_pipeline, StandardGraphicsPipelineInfo, transparent_cba};
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
    translucent_fluid_gfxs_pipeline: vk::Pipeline,

    terrain_vbo: Option<(vk::Buffer, vk::DeviceMemory)>,
    terrain_ibo: Option<(vk::Buffer, vk::DeviceMemory, u32)>,
    transparent_vbo: Option<(vk::Buffer, vk::DeviceMemory)>,
    transparent_ibo: Option<(vk::Buffer, vk::DeviceMemory, u32)>,
    translucent_fluid_vbo: Option<(vk::Buffer, vk::DeviceMemory)>,
    translucent_fluid_ibo: Option<(vk::Buffer, vk::DeviceMemory, u32)>,

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
            vec![  // set 2 for animations
                (vk::DescriptorType::UNIFORM_BUFFER, vk::ShaderStageFlags::VERTEX)  // time
            ]
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

        let (translucent_fluid_shader_stages, translucent_fluid_shader_modules) = gen_shader_modules_info(
            device.clone(), vec![
                ("C:/Users/andrewshen/documents/matrixagon2/src/shader/chunk_fluid.vert", vk::ShaderStageFlags::VERTEX),
                ("C:/Users/andrewshen/documents/matrixagon2/src/shader/chunk_translucent.frag", vk::ShaderStageFlags::FRAGMENT),
            ]);

        let vertex_input_state = get_vertex_inp!(ChunkVertex;
            (vk::Format::R32G32B32_SFLOAT, pos),
            (vk::Format::R32G32_SFLOAT, uv),
            (vk::Format::R32_SFLOAT, txtr)
        );

        // SUBPASS & ATTACHMENTS

        // let renderpass = vk::RenderPassCreator::new(device, color_format, depth_format)
        //     .attachments(Attachment::Presentation(Purpose::Present, LOAD, STORE))
        //     .attachments(Attachment::Color(Purpose::Transparency, CLEAR, DONT_CARE))
        //     .attachments(Attachment::Depth(Purpose::Depth, DONT_CARE, DONT_CARE))
        //     .subpasses(SubpassName::MainOpaque)
        //     .depth(AttachmentRef(Purpose::Present))
        //     .input(AttachmentRef(Purpose::Present))
        //     .color(AttachmentRef(Purpose::Present))
        //     .resolve(AttachmentRef(Purpose::Present))
        //     .preserve(AttachmentRef(Purpose::Present))
        //     .dependency(SubpassName::Ext)
        //     .src_mask(COLOR_ATTACHMENT_OUTPUT | EARLY_FRAGMENT_TESTS, empty())
        //     .dst_mask(COLOR_ATTACHMENT_OUTPUT | EARLY_FRAGMENT_TESTS, COLOR_ATTACHMENT_WRITE | DEPTH_STENCIL_ATTACHMENT_WRITE,)
        //     .dependency(SubpassName::Ext)
        //     .src_mask(COLOR_ATTACHMENT_OUTPUT | EARLY_FRAGMENT_TESTS, empty())
        //     .dst_mask(COLOR_ATTACHMENT_OUTPUT | EARLY_FRAGMENT_TESTS, COLOR_ATTACHMENT_WRITE | DEPTH_STENCIL_ATTACHMENT_WRITE,)
        //     .subpasses(SubpassName::MainOpaque)
        //     .create();

        /* LIMIT IMAGE LAYOUT Most likely the only valid image layout in a graphics pipeline in subpass
            VK_IMAGE_LAYOUT_GENERAL                             = If same refs are used between input and color/depth
            VK_IMAGE_LAYOUT_COLOR_ATTACHMENT_OPTIMAL            = Writing color/generic attachments
            VK_IMAGE_LAYOUT_DEPTH_STENCIL_ATTACHMENT_OPTIMAL    = Writing depth attachment
            VK_IMAGE_LAYOUT_DEPTH_STENCIL_READ_ONLY_OPTIMAL     = Reading depth attachment
            VK_IMAGE_LAYOUT_SHADER_READ_ONLY_OPTIMAL            = Reading color/generic attachments

            LIMIT SRC/DST MASK
            https://github.com/David-DiGioia/vulkan-diagrams?tab=readme-ov-file#pipeline-stages-and-access-types
            .src_mask(COLOR_ATTACHMENT_OUTPUT(COLOR_ATTACHMENT_WRITE) | EARLY_FRAGMENT_TEST(DEPTH_STENCIL_ATTACHMENT_WRITE))
            .src_mask(COLOR_ATTACHMENT_OUTPUT(COLOR_ATTACHMENT_WRITE) | EARLY_FRAGMENT_TEST(DEPTH_STENCIL_ATTACHMENT_WRITE))
            .src_mask(COLOR_ATTACHMENT_OUTPUT(WRITE) | EARLY_FRAGMENT_TEST(WRITE))
            .src_mask(COLOR_ATTACHMENT_OUTPUT(WRITE) | EARLY_FRAGMENT_TEST(WRITE))
         */

        let renderpass_info = vk::RenderPassCreateInfo::builder()
            .attachments(&[
                vk::AttachmentDescription {  // presentation attachment
                    format: color_format,
                    samples: vk::SampleCountFlags::TYPE_1,  // multi sampling
                    load_op: vk::AttachmentLoadOp::CLEAR, store_op: vk::AttachmentStoreOp::STORE,
                    stencil_load_op: vk::AttachmentLoadOp::DONT_CARE, stencil_store_op: vk::AttachmentStoreOp::DONT_CARE,  // ignore
                    initial_layout: vk::ImageLayout::UNDEFINED,  // dependent on previous renderpass
                    final_layout: vk::ImageLayout::PRESENT_SRC_KHR,  // dependent on type
                    ..Default::default()
                },
                vk::AttachmentDescription {  // depth attachment
                    format: depth_format,
                    samples: vk::SampleCountFlags::TYPE_1,  // multi sampling
                    load_op: vk::AttachmentLoadOp::CLEAR, store_op: vk::AttachmentStoreOp::DONT_CARE,
                    stencil_load_op: vk::AttachmentLoadOp::DONT_CARE, stencil_store_op: vk::AttachmentStoreOp::DONT_CARE,  // ignore
                    initial_layout: vk::ImageLayout::UNDEFINED,  // dependent on previous renderpass
                    final_layout: vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL,  // dependent on type
                    ..Default::default()
                }
            ])
            .subpasses(&[
                // block subpass
                vk::SubpassDescription::builder()
                    .pipeline_bind_point(vk::PipelineBindPoint::GRAPHICS)
                    .color_attachments(&[
                        vk::AttachmentReference { attachment: 0, layout: vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL }
                    ]).depth_stencil_attachment(
                        &vk::AttachmentReference { attachment: 1, layout: vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL }
                    ).build(),
                // composition subpass
                vk::SubpassDescription::builder()
                    .pipeline_bind_point(vk::PipelineBindPoint::GRAPHICS)
                    .input_attachments(&[
                        vk::AttachmentReference { attachment: 0, layout: vk::ImageLayout::GENERAL }
                    ]).color_attachments(&[
                        vk::AttachmentReference { attachment: 0, layout: vk::ImageLayout::GENERAL }
                    ]).build(),
            ])
            .dependencies(&[
                // block subpass dependency
                vk::SubpassDependency {
                    src_subpass: vk::SUBPASS_EXTERNAL, dst_subpass: 0,
                    src_stage_mask: vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT | vk::PipelineStageFlags::EARLY_FRAGMENT_TESTS,
                    dst_stage_mask: vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT | vk::PipelineStageFlags::EARLY_FRAGMENT_TESTS,
                    src_access_mask: vk::AccessFlags::empty(),
                    dst_access_mask: vk::AccessFlags::COLOR_ATTACHMENT_WRITE | vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE,
                    ..Default::default()
                },
                // composition subpass dependency
                vk::SubpassDependency {
                    src_subpass: 0, dst_subpass: 1,
                    src_stage_mask: vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
                    dst_stage_mask: vk::PipelineStageFlags::FRAGMENT_SHADER,
                    src_access_mask: vk::AccessFlags::COLOR_ATTACHMENT_WRITE,
                    dst_access_mask: vk::AccessFlags::INPUT_ATTACHMENT_READ,
                    ..Default::default()
                },
            ])
            .build();
        let renderpass = device.create_render_pass(&renderpass_info, None).unwrap();

        // PIPELINE CREATION

        let graphics_pipelines = standard_graphics_pipeline(
            device.clone(),
            vec![
                StandardGraphicsPipelineInfo {
                    shader_stages, vertex_input_state,
                    back_face_culling: true, depth_testing: true,
                    color_blend_attachment_state: vec![transparent_cba()],
                    subpass_index: 0,
                },
                StandardGraphicsPipelineInfo {
                    shader_stages: transparent_shader_stages, vertex_input_state,
                    back_face_culling: false, depth_testing: true,
                    color_blend_attachment_state: vec![transparent_cba()],
                    subpass_index: 0,
                },
                StandardGraphicsPipelineInfo {
                    shader_stages: translucent_fluid_shader_stages, vertex_input_state,
                    back_face_culling: false, depth_testing: true,
                    color_blend_attachment_state: vec![transparent_cba()],
                    subpass_index: 0,
                },
            ],
            descriptor.pipeline_layout, renderpass,
        );

        destroy_shader_modules(device.clone(), shader_modules);
        destroy_shader_modules(device.clone(), transparent_shader_modules);
        destroy_shader_modules(device.clone(), translucent_fluid_shader_modules);

        Self {
            // transparency_sub_shader: ChunkTransparencyRasterizerSubShader::new(),
            // TODO: DEBUG UI SENSITIVE
            debug_ui_sub_shader: DebugUISubShader::new(device.clone(), extent, descriptor.pipeline_layout, renderpass),

            device: device.clone(),
            extent,
            renderpass,
            gfxs_pipeline: graphics_pipelines[0],
            transparent_gfxs_pipeline: graphics_pipelines[1],
            translucent_fluid_gfxs_pipeline: graphics_pipelines[2],
            terrain_vbo: None, terrain_ibo: None,
            transparent_vbo: None, transparent_ibo: None,
            translucent_fluid_vbo: None, translucent_fluid_ibo: None,
            descriptor,
        }
    }
}

impl Shader for ChunkRasterizer {
    fn renderpass(&self) -> vk::RenderPass {
        self.renderpass
    }

    fn attachments(&self) -> Vec<FBAttachmentRef> {
        vec![  // TODO: DEBUG UI SENSITIVE
            FBAttachmentRef::Depth,
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
                RenderData::InitialDescriptorBuffer(buf, RenderDataPurpose::Time) => {
                    self.descriptor.write_buffer(2, 0, buf);
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
            RenderData::RecreateVertexBuffer(buf, mem, RenderDataPurpose::TerrainOpaque) => unsafe {
                println!("RECREATE [OPAQUE/DEFAULT] VERTEX BUFFER");
                if let Some((old_buf, old_mem)) = self.terrain_vbo {
                    self.device.device_wait_idle().unwrap();
                    self.device.destroy_buffer(old_buf, None);
                    self.device.free_memory(old_mem, None);
                }
                self.terrain_vbo = Some((buf, mem));
            }
            RenderData::RecreateIndexBuffer(buf, mem, len, RenderDataPurpose::TerrainOpaque) => unsafe {
                println!("RECREATE [OPAQUE/DEFAULT] INDEX BUFFER");
                if let Some((old_buf, old_mem, _)) = self.terrain_ibo {
                    self.device.device_wait_idle().unwrap();
                    self.device.destroy_buffer(old_buf, None);
                    self.device.free_memory(old_mem, None);
                }
                self.terrain_ibo = Some((buf, mem, len));
            }
            RenderData::RecreateVertexBuffer(buf, mem, RenderDataPurpose::TerrainTransparent) => unsafe {
                println!("RECREATE [TRANSPARENT] VERTEX BUFFER");
                if let Some((old_buf, old_mem)) = self.transparent_vbo {
                    self.device.device_wait_idle().unwrap();
                    self.device.destroy_buffer(old_buf, None);
                    self.device.free_memory(old_mem, None);
                }
                self.transparent_vbo = Some((buf, mem));
            }
            RenderData::RecreateIndexBuffer(buf, mem, len, RenderDataPurpose::TerrainTransparent) => unsafe {
                println!("RECREATE [TRANSPARENT] INDEX BUFFER");
                if let Some((old_buf, old_mem, _)) = self.transparent_ibo {
                    self.device.device_wait_idle().unwrap();
                    self.device.destroy_buffer(old_buf, None);
                    self.device.free_memory(old_mem, None);
                }
                self.transparent_ibo = Some((buf, mem, len));
            }
            RenderData::RecreateVertexBuffer(buf, mem, RenderDataPurpose::TerrainTranslucent) => unsafe {
                println!("RECREATE [TRANSLUCENT] VERTEX BUFFER");
                if let Some((old_buf, old_mem)) = self.translucent_fluid_vbo {
                    self.device.device_wait_idle().unwrap();
                    self.device.destroy_buffer(old_buf, None);
                    self.device.free_memory(old_mem, None);
                }
                self.translucent_fluid_vbo = Some((buf, mem));
            }
            RenderData::RecreateIndexBuffer(buf, mem, len, RenderDataPurpose::TerrainTranslucent) => unsafe {
                println!("RECREATE [TRANSLUCENT] INDEX BUFFER");
                if let Some((old_buf, old_mem, _)) = self.translucent_fluid_ibo {
                    self.device.device_wait_idle().unwrap();
                    self.device.destroy_buffer(old_buf, None);
                    self.device.free_memory(old_mem, None);
                }
                self.translucent_fluid_ibo = Some((buf, mem, len));
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
                                             0, &self.descriptor.descriptor_sets(&[0, 1, 2]), &[]);

        if let (Some((terrain_vbo, _)), Some(terrain_ibo)) = (self.terrain_vbo, self.terrain_ibo) {
            // opaque objects
            self.device.cmd_bind_pipeline(cmd_buf, vk::PipelineBindPoint::GRAPHICS, self.gfxs_pipeline);
            self.device.cmd_bind_vertex_buffers(cmd_buf, 0, &[terrain_vbo], &[0]);
            self.device.cmd_bind_index_buffer(cmd_buf, terrain_ibo.0, 0, vk::IndexType::UINT32);
            self.device.cmd_draw_indexed(cmd_buf, terrain_ibo.2, 1, 0, 0, 0);
        }
        if let (Some((transparent_vbo, _)), Some(transparent_ibo)) = (self.transparent_vbo, self.transparent_ibo) {
            // transparent objects
            self.device.cmd_bind_pipeline(cmd_buf, vk::PipelineBindPoint::GRAPHICS, self.transparent_gfxs_pipeline);
            self.device.cmd_bind_vertex_buffers(cmd_buf, 0, &[transparent_vbo], &[0]);
            self.device.cmd_bind_index_buffer(cmd_buf, transparent_ibo.0, 0, vk::IndexType::UINT32);
            self.device.cmd_draw_indexed(cmd_buf, transparent_ibo.2, 1, 0, 0, 0);
        }
        if let (Some((translucent_fluid_vbo, _)), Some(translucent_fluid_ibo)) = (self.translucent_fluid_vbo, self.translucent_fluid_ibo) {
            // translucent objects
            self.device.cmd_bind_pipeline(cmd_buf, vk::PipelineBindPoint::GRAPHICS, self.translucent_fluid_gfxs_pipeline);
            self.device.cmd_bind_vertex_buffers(cmd_buf, 0, &[translucent_fluid_vbo], &[0]);
            self.device.cmd_bind_index_buffer(cmd_buf, translucent_fluid_ibo.0, 0, vk::IndexType::UINT32);
            self.device.cmd_draw_indexed(cmd_buf, translucent_fluid_ibo.2, 1, 0, 0, 0);
        }

        self.debug_ui_sub_shader.draw_pipeline(cmd_buf, &self.descriptor);

        self.device.cmd_end_render_pass(cmd_buf);
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

        if let Some((old_buf, old_mem)) = self.translucent_fluid_vbo {
            self.device.destroy_buffer(old_buf, None);
            self.device.free_memory(old_mem, None);
        }
        if let Some((old_buf, old_mem, _)) = self.translucent_fluid_ibo {
            self.device.destroy_buffer(old_buf, None);
            self.device.free_memory(old_mem, None);
        }

        self.device.destroy_pipeline(self.gfxs_pipeline, None);
        self.device.destroy_pipeline(self.transparent_gfxs_pipeline, None);
        self.device.destroy_pipeline(self.translucent_fluid_gfxs_pipeline, None);

        self.descriptor.destroy();
        self.device.destroy_render_pass(self.renderpass, None);
    }
}
