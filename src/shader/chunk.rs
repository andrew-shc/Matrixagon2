use std::rc::Rc;
use ash::{Device, vk};
use crate::component::{RenderData, RenderDataPurpose};
use crate::framebuffer::FBAttachmentRef;
use crate::shader::{ColorBlendKind, DescriptorManager, Shader, create_graphics_pipeline, StandardGraphicsPipelineInfo, VBOFS};
use matrixagon_util::{Vertex, VulkanVertexState, create_renderpass, IndexedBuffer};


#[derive(Copy, Clone, Debug, Vertex)]
pub struct ChunkVertex {
    pub(crate) pos: [f32; 3],
    pub(crate) uv: [f32; 2],
    pub(crate) txtr: f32,
}

// emulating the structure of the EguiVertex
#[derive(Copy, Clone, Debug, Vertex)]
pub struct EguiVertex {
    pub(crate) pos: [f32; 2],
    pub(crate) uv: [f32; 2],
    pub(crate) color: [u8; 3],
}


pub struct ChunkRasterizer {
    device: Rc<Device>,

    extent: vk::Extent2D,
    descriptor: DescriptorManager,
    renderpass: vk::RenderPass,
    clear_values: Vec<vk::ClearValue>,

    terrain_pipeline: vk::Pipeline,
    transparent_pipeline: vk::Pipeline,
    translucent_fluid_pipeline: vk::Pipeline,

    terrain_ivbo: IndexedBuffer,
    transparent_ivbo: IndexedBuffer,
    translucent_fluid_ivbo: IndexedBuffer,

    // TODO: EGUI debug pipeline extension for this shader
    debug_scissors: Option<[vk::Rect2D; 1]>,
    debug_pipeline: vk::Pipeline,
    debug_ivbo: IndexedBuffer,

    vbo: Option<([vk::Buffer; 1], vk::DeviceMemory)>,
    ibo: Option<(vk::Buffer, vk::DeviceMemory, u32)>,
}

impl ChunkRasterizer {
    pub(crate) unsafe fn new(device: Rc<Device>, extent: vk::Extent2D, color_format: vk::Format,
                             depth_format: vk::Format) -> Self {
        let descriptor = DescriptorManager::new(device.clone(), vec![
            vec![  // set 0 for shader
                (vk::DescriptorType::UNIFORM_BUFFER, vk::ShaderStageFlags::VERTEX),  // proj-view
                (vk::DescriptorType::COMBINED_IMAGE_SAMPLER, vk::ShaderStageFlags::FRAGMENT),  // textures
            ],
            vec![  // set 1 for ui  TODO: EGUI debug descriptor-set extension
                (vk::DescriptorType::COMBINED_IMAGE_SAMPLER, vk::ShaderStageFlags::FRAGMENT), // egui debug ui texture
                (vk::DescriptorType::INPUT_ATTACHMENT, vk::ShaderStageFlags::FRAGMENT), // input attachment from previous
            ],
            vec![  // set 2 for animations
                (vk::DescriptorType::UNIFORM_BUFFER, vk::ShaderStageFlags::VERTEX)  // time
            ]
        ]);

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

        let renderpass = create_renderpass!{ [device];
            Attachments {
                presentation: {
                    format: color_format, samples: TYPE_1,
                    load: CLEAR, store: STORE,
                    stencil_load: DONT_CARE, stencil_store: DONT_CARE,
                    initial: UNDEFINED, final: PRESENT_SRC_KHR,
                }
                depth: {
                    format: depth_format, samples: TYPE_1,
                    load: CLEAR, store: DONT_CARE,
                    stencil_load: DONT_CARE, stencil_store: DONT_CARE,
                    initial: UNDEFINED, final: DEPTH_STENCIL_ATTACHMENT_OPTIMAL,
                }
            }
            Subpasses {
                terrain: {
                    input:,
                    color: presentation~COLOR_ATTACHMENT_OPTIMAL,
                    resolve:,
                    preserve:,
                    depth: depth~DEPTH_STENCIL_ATTACHMENT_OPTIMAL,
                }
                composition: {  // TODO: EGUI debug subpass extension (omittable)
                    input: presentation~GENERAL,
                    color: presentation~GENERAL,
                    resolve:,
                    preserve:,
                    depth:,
                }
            }
            Dependencies {
                ->terrain: {
                    src_stage:  COLOR_ATTACHMENT_OUTPUT | EARLY_FRAGMENT_TESTS,
                    dst_stage:  COLOR_ATTACHMENT_OUTPUT | EARLY_FRAGMENT_TESTS,
                    src_access: ,
                    dst_access: COLOR_ATTACHMENT_WRITE | DEPTH_STENCIL_ATTACHMENT_WRITE,
                }
                terrain->composition: {  // TODO: EGUI debug subpass extension (omittable)
                    src_stage:  COLOR_ATTACHMENT_OUTPUT,
                    dst_stage:  FRAGMENT_SHADER,
                    src_access: COLOR_ATTACHMENT_WRITE,
                    dst_access: INPUT_ATTACHMENT_READ,
                }
            }
        };

        let graphics_pipelines = create_graphics_pipeline(
            device.clone(),
            vec![
                StandardGraphicsPipelineInfo {  // opaque pipeline
                    shaders: vec![
                        ("C:/Users/andrewshen/documents/matrixagon2/src/shader/chunk.vert", vk::ShaderStageFlags::VERTEX),
                        ("C:/Users/andrewshen/documents/matrixagon2/src/shader/chunk.frag", vk::ShaderStageFlags::FRAGMENT),
                    ],
                    vertex_input_state: ChunkVertex::VERTEX_INPUT_STATE,
                    back_face_culling: true, depth_testing: true,
                    color_blend_attachment_state: vec![ColorBlendKind::disabled()],
                    subpass_index: 0,
                },
                StandardGraphicsPipelineInfo {  // transparent pipeline
                    shaders: vec![
                        ("C:/Users/andrewshen/documents/matrixagon2/src/shader/chunk.vert", vk::ShaderStageFlags::VERTEX),
                        ("C:/Users/andrewshen/documents/matrixagon2/src/shader/chunk_transparent.frag", vk::ShaderStageFlags::FRAGMENT),
                    ],
                    vertex_input_state: ChunkVertex::VERTEX_INPUT_STATE,
                    back_face_culling: false, depth_testing: true,
                    color_blend_attachment_state: vec![ColorBlendKind::transparent()],
                    subpass_index: 0,
                },
                StandardGraphicsPipelineInfo {  // translucent pipeline
                    shaders: vec![
                        ("C:/Users/andrewshen/documents/matrixagon2/src/shader/chunk_fluid.vert", vk::ShaderStageFlags::VERTEX),
                        ("C:/Users/andrewshen/documents/matrixagon2/src/shader/chunk_translucent.frag", vk::ShaderStageFlags::FRAGMENT),
                    ],
                    vertex_input_state: ChunkVertex::VERTEX_INPUT_STATE,
                    back_face_culling: false, depth_testing: true,
                    color_blend_attachment_state: vec![ColorBlendKind::transparent()],
                    subpass_index: 0,
                },
            ],
            descriptor.pipeline_layout, renderpass,
        );

        // multi-pipeline creation does not like different vertex input, so it's in a separate group
        let debug_graphics_pipeline = create_graphics_pipeline(
            device.clone(),
            vec![
                StandardGraphicsPipelineInfo {  // TODO: EGUI debug pipeline extension
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
            descriptor.pipeline_layout, renderpass,
        );

        Self {
            device: device.clone(),
            extent,
            descriptor,
            renderpass,
            clear_values: vec![
                vk::ClearValue { color: vk::ClearColorValue {float32: [0.2, 0.3, 0.9, 1.0]} },
                vk::ClearValue { color: vk::ClearColorValue {float32: [0.0, 0.0, 0.0, 0.0]} },
            ],

            terrain_pipeline: graphics_pipelines[0],
            transparent_pipeline: graphics_pipelines[1],
            translucent_fluid_pipeline: graphics_pipelines[2],
            terrain_ivbo: IndexedBuffer::new(device.clone()),
            transparent_ivbo: IndexedBuffer::new(device.clone()),
            translucent_fluid_ivbo: IndexedBuffer::new(device.clone()),

            // TODO: EGUI debug pipeline extension
            debug_scissors: None,
            debug_pipeline: debug_graphics_pipeline[0],
            debug_ivbo: IndexedBuffer::new(device.clone()),

            vbo: None, ibo: None
        }
    }
}

impl Shader for ChunkRasterizer {
    fn renderpass(&self) -> vk::RenderPass {
        self.renderpass
    }

    fn attachments(&self) -> Vec<FBAttachmentRef> {
        vec![  // TODO: EGUI debug extension
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
                RenderData::InitialDescriptorImage(img, RenderDataPurpose::DebugUI) => {
                    // TODO: EGUI debug extension
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
    }

    fn recreate_buffer(&mut self, render_data: RenderData) {
        match render_data {
            RenderData::RecreateVertexBuffer(buf, mem, RenderDataPurpose::TerrainOpaque) => unsafe {
                println!("RECREATE [OPAQUE/DEFAULT] VBO");
                self.terrain_ivbo.recreate_vbo([buf], mem);
            }
            RenderData::RecreateIndexBuffer(buf, mem, len, RenderDataPurpose::TerrainOpaque) => unsafe {
                println!("RECREATE [OPAQUE/DEFAULT] IBO");
                self.terrain_ivbo.recreate_ibo(buf, mem, len);
            }
            RenderData::RecreateVertexBuffer(buf, mem, RenderDataPurpose::TerrainTransparent) => unsafe {
                println!("RECREATE [TRANSPARENT] VBO");
                self.transparent_ivbo.recreate_vbo([buf], mem);
            }
            RenderData::RecreateIndexBuffer(buf, mem, len, RenderDataPurpose::TerrainTransparent) => unsafe {
                println!("RECREATE [TRANSPARENT] IBO");
                self.transparent_ivbo.recreate_ibo(buf, mem, len);
            }
            RenderData::RecreateVertexBuffer(buf, mem, RenderDataPurpose::TerrainTranslucent) => unsafe {
                println!("RECREATE [TRANSLUCENT] VBO");
                self.translucent_fluid_ivbo.recreate_vbo([buf], mem);
            }
            RenderData::RecreateIndexBuffer(buf, mem, len, RenderDataPurpose::TerrainTranslucent) => unsafe {
                println!("RECREATE [TRANSLUCENT] IBO");
                self.translucent_fluid_ivbo.recreate_ibo(buf, mem, len);
            }
            // TODO: EGUI debug data extension
            RenderData::RecreateVertexBuffer(buf, mem, RenderDataPurpose::DebugUI) => unsafe {
                // println!("RECREATE [DEBUG UI] VERTEX BUFFER");
                self.debug_ivbo.recreate_vbo([buf], mem);
            }
            RenderData::RecreateIndexBuffer(buf, mem, len, RenderDataPurpose::DebugUI) => unsafe {
                // println!("RECREATE [DEBUG UI] INDEX BUFFER");
                self.debug_ivbo.recreate_ibo(buf, mem, len);
            }
            RenderData::SetScissorDynamicState(scissor, RenderDataPurpose::DebugUI) => unsafe {
                self.debug_scissors.replace([scissor]);
            }
            _ => {},
        }
    }

    unsafe fn draw_command(&self, cmd_buf: vk::CommandBuffer, framebuffer: vk::Framebuffer) {
        let renderpass_info = vk::RenderPassBeginInfo::builder()
            .render_pass(self.renderpass)
            .framebuffer(framebuffer)
            .render_area(vk::Rect2D { offset: vk::Offset2D {x:0, y:0}, extent: self.extent})
            .clear_values(&self.clear_values)
            .build();

        self.device.cmd_begin_render_pass(cmd_buf, &renderpass_info, vk::SubpassContents::INLINE);

        self.device.cmd_bind_descriptor_sets(cmd_buf, vk::PipelineBindPoint::GRAPHICS, self.descriptor.pipeline_layout(),
                                             0, &self.descriptor.descriptor_sets(&[0, 1, 2]), &[]);

        let viewports = [vk::Viewport {
            x: 0.0,
            y: 0.0,
            width: self.extent.width as f32,
            height: self.extent.height as f32,
            min_depth: 0.0,
            max_depth: 1.0,
        }];
        let scissors = [vk::Rect2D { offset: vk::Offset2D { x: 0, y: 0 }, extent: self.extent }];
        self.device.cmd_set_viewport(cmd_buf, 0, &viewports);
        self.device.cmd_set_scissor(cmd_buf, 0, &scissors);

        {
            if let Some((terrain_vbo, terrain_ibo, ibo_len)) = self.terrain_ivbo.obtain_indexed_vbo() {
                // opaque objects
                self.device.cmd_bind_pipeline(cmd_buf, vk::PipelineBindPoint::GRAPHICS, self.terrain_pipeline);
                self.device.cmd_bind_vertex_buffers(cmd_buf, 0, &terrain_vbo, &VBOFS);
                self.device.cmd_bind_index_buffer(cmd_buf, terrain_ibo, 0, vk::IndexType::UINT32);
                self.device.cmd_draw_indexed(cmd_buf, ibo_len, 1, 0, 0, 0);
            }
            if let Some((transparent_vbo, transparent_ibo, ibo_len)) = self.transparent_ivbo.obtain_indexed_vbo() {
                // transparent objects
                self.device.cmd_bind_pipeline(cmd_buf, vk::PipelineBindPoint::GRAPHICS, self.transparent_pipeline);
                self.device.cmd_bind_vertex_buffers(cmd_buf, 0, &transparent_vbo, &VBOFS);
                self.device.cmd_bind_index_buffer(cmd_buf, transparent_ibo, 0, vk::IndexType::UINT32);
                self.device.cmd_draw_indexed(cmd_buf, ibo_len, 1, 0, 0, 0);
            }
            if let Some((translucent_fluid_vbo, translucent_fluid_ibo, ibo_len)) = self.translucent_fluid_ivbo.obtain_indexed_vbo() {
                // translucent objects
                self.device.cmd_bind_pipeline(cmd_buf, vk::PipelineBindPoint::GRAPHICS, self.translucent_fluid_pipeline);
                self.device.cmd_bind_vertex_buffers(cmd_buf, 0, &translucent_fluid_vbo, &VBOFS);
                self.device.cmd_bind_index_buffer(cmd_buf, translucent_fluid_ibo, 0, vk::IndexType::UINT32);
                self.device.cmd_draw_indexed(cmd_buf, ibo_len, 1, 0, 0, 0);
            }
        }

        // TODO: EGUI debug draw extension
        self.device.cmd_next_subpass(cmd_buf, vk::SubpassContents::INLINE);

        {
            if let Some(scissors) = self.debug_scissors {
                self.device.cmd_set_scissor(cmd_buf, 0, &scissors);
            }

            if let Some((ui_vbo, ui_ibo, ibo_len)) = self.debug_ivbo.obtain_indexed_vbo() {
                self.device.cmd_bind_pipeline(cmd_buf, vk::PipelineBindPoint::GRAPHICS, self.debug_pipeline);
                self.device.cmd_bind_vertex_buffers(cmd_buf, 0, &ui_vbo, &VBOFS);
                self.device.cmd_bind_index_buffer(cmd_buf, ui_ibo, 0, vk::IndexType::UINT32);
                self.device.cmd_draw_indexed(cmd_buf, ibo_len, 1, 0, 0, 0);
            }
        }

        self.device.cmd_end_render_pass(cmd_buf);
    }

    unsafe fn destroy(&self) {
        // TODO: EGUI debug extension
        self.debug_ivbo.destroy();
        self.device.destroy_pipeline(self.debug_pipeline, None);

        self.terrain_ivbo.destroy();
        self.transparent_ivbo.destroy();
        self.translucent_fluid_ivbo.destroy();

        self.device.destroy_pipeline(self.terrain_pipeline, None);
        self.device.destroy_pipeline(self.transparent_pipeline, None);
        self.device.destroy_pipeline(self.translucent_fluid_pipeline, None);

        self.descriptor.destroy();
        self.device.destroy_render_pass(self.renderpass, None);
    }
}
