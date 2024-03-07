use alloc::alloc;
use std::mem;
use std::rc::Rc;
use ash::{Device, vk};
use ash::vk::{AccessFlags, ImageLayout, PipelineStageFlags, SUBPASS_EXTERNAL};
use crate::component::{RenderData, RenderDataPurpose};
use crate::framebuffer::FBAttachmentRef;
// use crate::{vertex_input};
use crate::shader::{ColorBlendKind, DescriptorManager, Shader, standard_graphics_pipeline, StandardGraphicsPipelineInfo, VBOFS};
use crate::shader::debug_ui::DebugUISubShader;
use matrixagon_util::{Vertex, VulkanVertexState, create_renderpass};


#[derive(Copy, Clone, Debug, Vertex)]
pub struct ChunkVertex {
    pub(crate) pos: [f32; 3],
    pub(crate) uv: [f32; 2],
    pub(crate) txtr: f32,
}


pub struct ChunkRasterizer {
    device: Rc<Device>,
    extent: vk::Extent2D,
    renderpass: vk::RenderPass,
    clear_values: Vec<vk::ClearValue>,

    gfxs_pipeline: vk::Pipeline,
    transparent_gfxs_pipeline: vk::Pipeline,
    translucent_fluid_gfxs_pipeline: vk::Pipeline,

    terrain_vbo: Option<([vk::Buffer; 1], vk::DeviceMemory)>,
    terrain_ibo: Option<(vk::Buffer, vk::DeviceMemory, u32)>,
    transparent_vbo: Option<([vk::Buffer; 1], vk::DeviceMemory)>,
    transparent_ibo: Option<(vk::Buffer, vk::DeviceMemory, u32)>,
    translucent_fluid_vbo: Option<([vk::Buffer; 1], vk::DeviceMemory)>,
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
        /*
        let renderpass = create_renderpass!{
            [device];
            Attachments {
                presentation: {
                    format: color_fmt,
                    samples: TYPE_1,
                    load: CLEAR,
                    store: STORE,
                    stencil_load: IGNORE,
                    stencil_store: IGNORE,
                    initial: UNDEFINED,
                    final: PRESENT_SRC_KHR,
                }
                depth1: {
                    format: depth_fmt,
                    samples: TYPE_1,
                    load: CLEAR,
                    store: STORE,
                    stencil_load: IGNORE,
                    stencil_store: IGNORE,
                    initial: UNDEFINED,
                    final: PRESENT_SRC_KHR,
                }
            }
            Subpasses {
                block: {
                    input: presentation~GENERAL,
                    color: presentation~GENERAL presentation~COLOR_OPTIMAL,
                    resolve: ,
                    preserve: presentation,
                    depth: depth1~DEPTH_ATTACHMENT,
                }
            }
            Dependencies {
                ->block {

                }
                block->composition {
                    src_stage:  COLOR_ATTACHMENT_OUTPUT | EARLY_FRAGMENT_TESTS,
                    dst_stage:  COLOR_ATTACHMENT_OUTPUT | EARLY_FRAGMENT_TESTS,
                    src_access: COLOR_ATTACHMENT_WRITE,
                    dst_access: COLOR_ATTACHMENT_WRITE | DEPTH_STENCIL_ATTACHMENT_WRITE,
                }
                composition-> {

                }
            }
        }
         */
        let renderpass = create_renderpass!{
            [device];
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
                composition: {
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
                terrain->composition: {
                    src_stage:  COLOR_ATTACHMENT_OUTPUT,
                    dst_stage:  FRAGMENT_SHADER,
                    src_access: COLOR_ATTACHMENT_WRITE,
                    dst_access: INPUT_ATTACHMENT_READ,
                }
            }
        };

        // let renderpass = {
        //     let presentation = ash::vk::AttachmentDescription {
        //             format: color_format,
        //             samples: ash::vk::SampleCountFlags::TYPE_1,
        //             load_op: ash::vk::AttachmentLoadOp::CLEAR,
        //             store_op: ash::vk::AttachmentStoreOp::STORE,
        //             stencil_load_op: ash::vk::AttachmentLoadOp::DONT_CARE,
        //             stencil_store_op: ash::vk::AttachmentStoreOp::DONT_CARE,
        //             initial_layout: ash::vk::ImageLayout::UNDEFINED,
        //             final_layout: ash::vk::ImageLayout::PRESENT_SRC_KHR,
        //             ..Default::default()
        //         };
        //     let depth = ash::vk::AttachmentDescription {
        //             format: depth_format,
        //             samples: ash::vk::SampleCountFlags::TYPE_1,
        //             load_op: ash::vk::AttachmentLoadOp::CLEAR,
        //             store_op: ash::vk::AttachmentStoreOp::DONT_CARE,
        //             stencil_load_op: ash::vk::AttachmentLoadOp::DONT_CARE,
        //             stencil_store_op: ash::vk::AttachmentStoreOp::DONT_CARE,
        //             initial_layout: ash::vk::ImageLayout::UNDEFINED,
        //             final_layout: ash::vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL,
        //             ..Default::default()
        //         };
        //     let terrain = ash::vk::SubpassDescription::builder()
        //             .pipeline_bind_point(ash::vk::PipelineBindPoint::GRAPHICS)
        //             // .input_attachments(&Vec::new())
        //             .color_attachments(&[
        //                 ash::vk::AttachmentReference {
        //                     attachment: 0,
        //                     layout: ash::vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
        //                 },
        //             ])
        //             // .resolve_attachments(&Vec::new())
        //             // .preserve_attachments(&Vec::new())
        //             .depth_stencil_attachment( &ash::vk::AttachmentReference {
        //                         attachment: 1,
        //                         layout: ash::vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL,
        //                     }
        //             )
        //             .build();
        //     let composition = ash::vk::SubpassDescription::builder()
        //             .pipeline_bind_point(ash::vk::PipelineBindPoint::GRAPHICS)
        //             .input_attachments(&[ vk::AttachmentReference {
        //                     attachment: 0,
        //                     layout: ash::vk::ImageLayout::GENERAL,
        //                 },
        //             ])
        //             .color_attachments(&[ vk::AttachmentReference {
        //                     attachment: 0,
        //                     layout: ash::vk::ImageLayout::GENERAL,
        //                 },
        //             ])
        //             // .resolve_attachments(&Vec::new())
        //             // .preserve_attachments(&Vec::new())
        //             .build();
        //     let __dependencies = vec![
        //             vk::SubpassDependency {
        //                 src_subpass: SUBPASS_EXTERNAL,
        //                 dst_subpass: 0,
        //                 src_stage_mask: {
        //                     ash::vk::PipelineStageFlags::empty()
        //                         | ash::vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT
        //                         | ash::vk::PipelineStageFlags::EARLY_FRAGMENT_TESTS
        //                 },
        //                 dst_stage_mask: {
        //                     ash::vk::PipelineStageFlags::empty()
        //                         | ash::vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT
        //                         | ash::vk::PipelineStageFlags::EARLY_FRAGMENT_TESTS
        //                 },
        //                 src_access_mask: { ash::vk::AccessFlags::empty() },
        //                 dst_access_mask: {
        //                     ash::vk::AccessFlags::empty()
        //                         | ash::vk::AccessFlags::COLOR_ATTACHMENT_WRITE
        //                         | ash::vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE
        //                 },
        //                 ..Default::default()
        //             },
        //             vk::SubpassDependency {
        //                 src_subpass: 0,
        //                 dst_subpass: 1,
        //                 src_stage_mask: {
        //                     ash::vk::PipelineStageFlags::empty()
        //                         | ash::vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT
        //                 },
        //                 dst_stage_mask: {
        //                     ash::vk::PipelineStageFlags::empty()
        //                         | ash::vk::PipelineStageFlags::FRAGMENT_SHADER
        //                 },
        //                 src_access_mask: {
        //                     ash::vk::AccessFlags::empty()
        //                         | ash::vk::AccessFlags::COLOR_ATTACHMENT_WRITE
        //                 },
        //                 dst_access_mask: {
        //                     ash::vk::AccessFlags::empty()
        //                         | ash::vk::AccessFlags::INPUT_ATTACHMENT_READ
        //                 },
        //                 ..Default::default()
        //             },
        //         ];
        //     let __attachments = vec![presentation, depth];
        //     let __subpasses = vec![terrain, composition];
        //     let __renderpass_info = ash::vk::RenderPassCreateInfo::builder()
        //         .attachments(&__attachments)
        //         .subpasses(&__subpasses)
        //         .dependencies(&__dependencies)
        //         .build();
        //     device.create_render_pass(&__renderpass_info, None).unwrap()
        // };

        // let renderpass = create_renderpass(
        //     device.clone(),
        //     vec![
        //         AttachmentKind::presentation(color_format),
        //         AttachmentKind::depth(depth_format)
        //     ],
        //     vec![
        //         graphics_subpass(  // block subpass
        //             vec![],
        //             vec![(0, ImageLayout::COLOR_ATTACHMENT_OPTIMAL)],
        //             vec![],
        //             vec![],
        //             Some(1)
        //         ).build(),
        //         graphics_subpass(  // composition subpass
        //             vec![(0, ImageLayout::GENERAL)],
        //             vec![(0, ImageLayout::GENERAL)],
        //             vec![],
        //             vec![],
        //             Some(1)
        //         ).build(),
        //     ],
        //     vec![
        //         // block subpass dependency
        //         vk::SubpassDependency::builder()
        //             .src_subpass(SUBPASS_EXTERNAL).dst_subpass(0)
        //             .src_stage_mask(    S::COLOR_ATTACHMENT_OUTPUT | S::EARLY_FRAGMENT_TESTS          )
        //             .dst_stage_mask(    S::COLOR_ATTACHMENT_OUTPUT | S::EARLY_FRAGMENT_TESTS          )
        //             .src_access_mask(   A::empty()                                                    )
        //             .dst_access_mask(   A::COLOR_ATTACHMENT_WRITE | A::DEPTH_STENCIL_ATTACHMENT_WRITE ),
        //         // composition subpass dependency
        //         vk::SubpassDependency::builder()
        //             .src_subpass(0).dst_subpass(1)
        //             .src_stage_mask(    S::COLOR_ATTACHMENT_OUTPUT )
        //             .dst_stage_mask(    S::FRAGMENT_SHADER         )
        //             .src_access_mask(   A::COLOR_ATTACHMENT_WRITE  )
        //             .dst_access_mask(   A::INPUT_ATTACHMENT_READ   ),
        //     ]
        // );

        // let attachments = vec![
        //     vk::AttachmentDescription {  // presentation attachment
        //         format: color_format,
        //         samples: vk::SampleCountFlags::TYPE_1,  // multi sampling
        //         load_op: vk::AttachmentLoadOp::CLEAR, store_op: vk::AttachmentStoreOp::STORE,
        //         stencil_load_op: vk::AttachmentLoadOp::DONT_CARE, stencil_store_op: vk::AttachmentStoreOp::DONT_CARE,  // ignore
        //         initial_layout: vk::ImageLayout::UNDEFINED,  // dependent on previous renderpass
        //         final_layout: vk::ImageLayout::PRESENT_SRC_KHR,  // dependent on type
        //         ..Default::default()
        //     },
        //     vk::AttachmentDescription {  // depth attachment
        //         format: depth_format,
        //         samples: vk::SampleCountFlags::TYPE_1,  // multi sampling
        //         load_op: vk::AttachmentLoadOp::CLEAR, store_op: vk::AttachmentStoreOp::DONT_CARE,
        //         stencil_load_op: vk::AttachmentLoadOp::DONT_CARE, stencil_store_op: vk::AttachmentStoreOp::DONT_CARE,  // ignore
        //         initial_layout: vk::ImageLayout::UNDEFINED,  // dependent on previous renderpass
        //         final_layout: vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL,  // dependent on type
        //         ..Default::default()
        //     }
        // ];
        //
        // let subpasses = vec![
        //     // block subpass
        //     vk::SubpassDescription::builder()
        //         .pipeline_bind_point(vk::PipelineBindPoint::GRAPHICS)
        //         .color_attachments(&[
        //             vk::AttachmentReference { attachment: 0, layout: vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL }
        //         ]).depth_stencil_attachment(
        //         &vk::AttachmentReference { attachment: 1, layout: vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL }
        //     ).build(),
        //     // composition subpass
        //     vk::SubpassDescription::builder()
        //         .pipeline_bind_point(vk::PipelineBindPoint::GRAPHICS)
        //         .input_attachments(&[
        //             vk::AttachmentReference { attachment: 0, layout: vk::ImageLayout::GENERAL }
        //         ]).color_attachments(&[
        //         vk::AttachmentReference { attachment: 0, layout: vk::ImageLayout::GENERAL }
        //     ]).build(),
        // ];
        //
        // let dependencies = vec![
        //     // block subpass dependency
        //     vk::SubpassDependency {
        //         src_subpass: vk::SUBPASS_EXTERNAL, dst_subpass: 0,
        //         src_stage_mask: {vk::PipelineStageFlags::empty() | vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT | vk::PipelineStageFlags::EARLY_FRAGMENT_TESTS},
        //         dst_stage_mask: {vk::PipelineStageFlags::empty() | vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT | vk::PipelineStageFlags::EARLY_FRAGMENT_TESTS},
        //         src_access_mask: {vk::AccessFlags::empty()},
        //         dst_access_mask: {vk::AccessFlags::empty() | vk::AccessFlags::COLOR_ATTACHMENT_WRITE | vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE},
        //         ..Default::default()
        //     },
        //     // composition subpass dependency
        //     vk::SubpassDependency {
        //         src_subpass: 0, dst_subpass: 1,
        //         src_stage_mask: {vk::PipelineStageFlags::empty() | vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT},
        //         dst_stage_mask: {vk::PipelineStageFlags::empty() | vk::PipelineStageFlags::FRAGMENT_SHADER},
        //         src_access_mask: {vk::AccessFlags::empty() | vk::AccessFlags::COLOR_ATTACHMENT_WRITE},
        //         dst_access_mask: {vk::AccessFlags::empty() | vk::AccessFlags::INPUT_ATTACHMENT_READ},
        //         ..Default::default()
        //     },
        // ];
        //
        // let a = vec![attachments[0], attachments[1]];
        // let s = vec![subpasses[0], subpasses[1]];
        // let d = vec![dependencies[0], dependencies[1]];
        //
        // let renderpass_info = vk::RenderPassCreateInfo::builder()
        //     .attachments(&a)
        //     .subpasses(&s)
        //     .dependencies(&d)
        //     .build();
        // let renderpass = device.create_render_pass(&renderpass_info, None).unwrap();

        // GRAPHICS PIPELINE

        let graphics_pipelines = standard_graphics_pipeline(
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

        Self {
            // transparency_sub_shader: ChunkTransparencyRasterizerSubShader::new(),
            // TODO: DEBUG UI SENSITIVE
            debug_ui_sub_shader: DebugUISubShader::new(device.clone(), descriptor.pipeline_layout, renderpass),

            device: device.clone(),
            extent,
            renderpass,
            clear_values: vec![
                vk::ClearValue { color: vk::ClearColorValue {float32: [0.2, 0.3, 0.9, 1.0]} },
                vk::ClearValue { color: vk::ClearColorValue {float32: [0.0, 0.0, 0.0, 0.0]} },
            ],
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
    }

    fn recreate_buffer(&mut self, render_data: RenderData) {
        match render_data {
            RenderData::RecreateVertexBuffer(buf, mem, RenderDataPurpose::TerrainOpaque) => unsafe {
                println!("RECREATE [OPAQUE/DEFAULT] VERTEX BUFFER");
                if let Some((old_buf, old_mem)) = self.terrain_vbo {
                    self.device.device_wait_idle().unwrap();
                    self.device.destroy_buffer(old_buf[0], None);
                    self.device.free_memory(old_mem, None);
                }
                self.terrain_vbo = Some(([buf], mem));
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
                    self.device.destroy_buffer(old_buf[0], None);
                    self.device.free_memory(old_mem, None);
                }
                self.transparent_vbo = Some(([buf], mem));
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
                    self.device.destroy_buffer(old_buf[0], None);
                    self.device.free_memory(old_mem, None);
                }
                self.translucent_fluid_vbo = Some(([buf], mem));
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
                    self.device.destroy_buffer(old_buf[0], None);
                    self.device.free_memory(old_mem, None);
                }
                self.debug_ui_sub_shader.ui_vbo = Some(([buf], mem));
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
            .clear_values(&self.clear_values)
            .build();

        self.device.cmd_begin_render_pass(cmd_buf, &renderpass_info, vk::SubpassContents::INLINE);

        let viewports = [vk::Viewport {
            x: 0.0,
            y: 0.0,
            width: self.extent.width as f32,
            height: self.extent.height as f32,
            min_depth: 0.0,
            max_depth: 1.0,
        }];
        let scissors = [vk::Rect2D { offset: vk::Offset2D {x:0,y:0}, extent: self.extent }];
        self.device.cmd_set_viewport(cmd_buf, 0, &viewports);
        self.device.cmd_set_scissor(cmd_buf, 0, &scissors);

        self.device.cmd_bind_descriptor_sets(cmd_buf, vk::PipelineBindPoint::GRAPHICS, self.descriptor.pipeline_layout(),
                                             0, &self.descriptor.descriptor_sets(&[0, 1, 2]), &[]);

        if let (Some((terrain_vbo, _)), Some(terrain_ibo)) = (self.terrain_vbo, self.terrain_ibo) {
            // opaque objects
            self.device.cmd_bind_pipeline(cmd_buf, vk::PipelineBindPoint::GRAPHICS, self.gfxs_pipeline);
            self.device.cmd_bind_vertex_buffers(cmd_buf, 0, &terrain_vbo, &VBOFS);
            self.device.cmd_bind_index_buffer(cmd_buf, terrain_ibo.0, 0, vk::IndexType::UINT32);
            self.device.cmd_draw_indexed(cmd_buf, terrain_ibo.2, 1, 0, 0, 0);
        }
        if let (Some((transparent_vbo, _)), Some(transparent_ibo)) = (self.transparent_vbo, self.transparent_ibo) {
            // transparent objects
            self.device.cmd_bind_pipeline(cmd_buf, vk::PipelineBindPoint::GRAPHICS, self.transparent_gfxs_pipeline);
            self.device.cmd_bind_vertex_buffers(cmd_buf, 0, &transparent_vbo, &VBOFS);
            self.device.cmd_bind_index_buffer(cmd_buf, transparent_ibo.0, 0, vk::IndexType::UINT32);
            self.device.cmd_draw_indexed(cmd_buf, transparent_ibo.2, 1, 0, 0, 0);
        }
        if let (Some((translucent_fluid_vbo, _)), Some(translucent_fluid_ibo)) = (self.translucent_fluid_vbo, self.translucent_fluid_ibo) {
            // translucent objects
            self.device.cmd_bind_pipeline(cmd_buf, vk::PipelineBindPoint::GRAPHICS, self.translucent_fluid_gfxs_pipeline);
            self.device.cmd_bind_vertex_buffers(cmd_buf, 0, &translucent_fluid_vbo, &VBOFS);
            self.device.cmd_bind_index_buffer(cmd_buf, translucent_fluid_ibo.0, 0, vk::IndexType::UINT32);
            self.device.cmd_draw_indexed(cmd_buf, translucent_fluid_ibo.2, 1, 0, 0, 0);
        }

        self.debug_ui_sub_shader.draw_pipeline(cmd_buf);

        self.device.cmd_end_render_pass(cmd_buf);
    }

    unsafe fn destroy(&self) {
        self.debug_ui_sub_shader.destroy();

        if let Some((old_buf, old_mem)) = self.terrain_vbo {
            self.device.destroy_buffer(old_buf[0], None);
            self.device.free_memory(old_mem, None);
        }
        if let Some((old_buf, old_mem, _)) = self.terrain_ibo {
            self.device.destroy_buffer(old_buf, None);
            self.device.free_memory(old_mem, None);
        }

        if let Some((old_buf, old_mem)) = self.transparent_vbo {
            self.device.destroy_buffer(old_buf[0], None);
            self.device.free_memory(old_mem, None);
        }
        if let Some((old_buf, old_mem, _)) = self.transparent_ibo {
            self.device.destroy_buffer(old_buf, None);
            self.device.free_memory(old_mem, None);
        }

        if let Some((old_buf, old_mem)) = self.translucent_fluid_vbo {
            self.device.destroy_buffer(old_buf[0], None);
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
