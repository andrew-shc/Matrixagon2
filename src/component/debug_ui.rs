use std::collections::{HashMap, VecDeque};
use std::rc::Rc;
use ash::{Device, vk};
use ash::vk::{CommandPool, Queue};
use egui::{ClippedPrimitive, Context, ImageData, Mesh, RawInput, TextureFilter, TextureId};
use egui::epaint::{ImageDelta, Primitive, Vertex};
use uom::fmt::DisplayStyle;
use crate::component::{Component, RenderData, RenderDataPurpose};
use crate::handler::VulkanInstance;
use crate::measurement::blox;
use crate::util::{cmd_recording, create_host_buffer, create_local_image};
use crate::world::{CardinalDir, WorldEvent};


#[derive(Clone)]
pub(crate) struct DebugUIData {
    face_direction: String,
    fps: String,
    pos: String,

    fps_hist: VecDeque<f32>,
}

impl Default for DebugUIData {
    fn default() -> Self {
        Self {
            face_direction: String::from(".face_direction: <UNDEFINED>"),
            fps: String::from(".fps: <UNDEFINED>"),
            pos: String::from(".pos: <UNDEFINED>"),
            fps_hist: VecDeque::new(),
        }
    }
}

pub(crate) struct DebugUI {
    ui_handler: EguiHandler,
    render_data: Vec<(Vec<Vertex>, Vec<u32>, vk::Rect2D, TextureId)>,
    pub(crate) ui_data: DebugUIData,
}

impl DebugUI {
    const FPS_SAMPLES: usize = 200;

    pub(crate) fn new(vi: Rc<VulkanInstance>, device: Rc<Device>, init_raw_input: RawInput) -> Self {
        let mut s = Self {
            ui_handler: EguiHandler::new(vi.clone(), device.clone(), init_raw_input),
            render_data: Vec::new(),
            ui_data: DebugUIData::default(),
        };
        unsafe {
            // needs to ensure ui_handler is display() ed before to obtain texture
            let mut render_data = s.ui_handler.display(s.ui_data.clone(), Self::ui_program());
            s.render_data.append(&mut render_data);
        }
        s
    }

    fn ui_program() -> impl FnOnce(&Context, DebugUIData) {
        |ctx: &Context, data: DebugUIData| {
            egui::CentralPanel::default().show(&ctx, |ui| {
                ui.label(data.face_direction);
                ui.label(data.fps);
                ui.label(data.pos);
            });
        }
    }
}

impl Component for DebugUI {
    fn render(&self) -> Vec<RenderData> {
        // TODO: do we need to make sure the buffer object lasts long through DebugUI?

        // TODO: assuming a single render data
        let (vert, indx, scissor, _) = &self.render_data[0];

        // no staging buffer for DebugUI, since it is debug and you would want the fastest update (and its just UI)

        let (vertex_buffer, vertex_buffer_mem, _, _) = unsafe {
            create_host_buffer(self.ui_handler.vi.clone(), self.ui_handler.device.clone(), vert, vk::BufferUsageFlags::VERTEX_BUFFER, true)
        };

        let (index_buffer, index_buffer_mem, _, _) = unsafe {
            create_host_buffer(self.ui_handler.vi.clone(), self.ui_handler.device.clone(), indx, vk::BufferUsageFlags::INDEX_BUFFER, true)
        };

        vec![
            RenderData::RecreateVertexBuffer(vertex_buffer, vertex_buffer_mem, RenderDataPurpose::DebugUI),
            RenderData::RecreateIndexBuffer(index_buffer, index_buffer_mem, indx.len() as u32, RenderDataPurpose::DebugUI),
            RenderData::SetScissorDynamicState(*scissor, RenderDataPurpose::DebugUI),
        ]
    }

    fn respond_event(&mut self, event: WorldEvent) -> Vec<WorldEvent> {
        match event {
            WorldEvent::UserFaceDir(new_dir) => {
                let dir_name = match new_dir {
                    CardinalDir::EAST => { String::from("EAST") }
                    CardinalDir::SOUTH => { String::from("SOUTH") }
                    CardinalDir::WEST => { String::from("WEST") }
                    CardinalDir::NORTH => { String::from("NORTH") }
                    CardinalDir::UNDEFINED => { String::from("[UNDEFINED]") }
                };
                self.ui_data.face_direction = format!("Direction: {}", dir_name);
            }
            WorldEvent::DeltaTime(dur) => {
                let sample = 1.0/dur.as_secs_f32();
                if self.ui_data.fps_hist.len() < Self::FPS_SAMPLES {
                    self.ui_data.fps_hist.push_back(sample);
                } else {
                    self.ui_data.fps_hist.push_back(sample);
                    self.ui_data.fps_hist.pop_front();
                }

                let fps_avg = self.ui_data.fps_hist.iter().sum::<f32>()/self.ui_data.fps_hist.len() as f32;

                self.ui_data.fps = format!("FPS: {}", fps_avg.round());
            }
            WorldEvent::UserPosition(pos) => {
                self.ui_data.pos = format!("Position: {} {} {}",
                                           pos.x.round::<blox>().into_format_args(blox, DisplayStyle::Abbreviation),
                                           pos.y.round::<blox>().into_format_args(blox, DisplayStyle::Abbreviation),
                                           pos.z.round::<blox>().into_format_args(blox, DisplayStyle::Abbreviation));
            }
            _ => {}
        }

        // TODO: for creating raw input
        // self.ui_handler.modify_raw_input(event);

        vec![]
    }

    fn update(&mut self) {
        unsafe { self.render_data = self.ui_handler.display(self.ui_data.clone(), Self::ui_program()); }
    }

    unsafe fn load_descriptors(&mut self, cmd_pool: CommandPool, queue: Queue) -> Vec<RenderData> {
        // TODO: assume egui will only create new descriptor texture once (for now)
        // TODO: ... also needs the full output from running the closure

        cmd_recording(self.ui_handler.device.clone(), cmd_pool, queue, |cmd_buf| {
            for (_, ui_txtr) in &self.ui_handler.textures {
                self.ui_handler.transfer_img_barrier(cmd_buf, ui_txtr);
            }
        });
        self.ui_handler.create_img_views();

        self.ui_handler.textures.iter()
            .map(|(_, txtr_descriptor)| {
                RenderData::InitialDescriptorImage(
                    vec![vk::DescriptorImageInfo {
                        image_layout: vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
                        image_view: txtr_descriptor.img_view.unwrap(),
                        sampler: txtr_descriptor.sampler,
                    }],
                    RenderDataPurpose::DebugUI,
                )
            })
            .collect::<Vec<RenderData>>()
    }

    unsafe fn destroy(&mut self) {
        self.ui_handler.destroy();
    }
}



struct UITextureDescriptor {
    sampler: vk::Sampler,
    img_view: Option<vk::ImageView>,
    host_buf: vk::Buffer,
    host_buf_mem: vk::DeviceMemory,
    local_img: vk::Image,
    local_img_mem: vk::DeviceMemory,
    extent: vk::Extent3D,
}


struct EguiHandler {
    // Vulkan loaders
    vi: Rc<VulkanInstance>,
    device: Rc<Device>,

    // Egui state components
    ctx: egui::Context,
    raw_input: RawInput,

    // Egui renders
    textures: HashMap<TextureId, UITextureDescriptor>,
}

impl EguiHandler {
    fn new(vi: Rc<VulkanInstance>, device: Rc<Device>, init_raw_input: RawInput) -> Self {
        Self {
            vi, device,
            ctx: egui::Context::default(),
            raw_input: init_raw_input,
            textures: HashMap::new(),
        }
    }

    // fn modify_raw_input(&mut self, event: WorldEvent) {
    //
    // }

    unsafe fn display(&mut self, data: DebugUIData, cb: impl FnOnce(&Context, DebugUIData)) -> Vec<(Vec<Vertex>, Vec<u32>, vk::Rect2D, TextureId)> {
        // TODO: aggregate all the events here and create raw input only in here
        // TODO: custom closure that also pass in dynamic info (vector of trait object that describes
        // TODO: ... what data it is and check compatibility between component updates and UI's compatibility

        let full_output = self.ctx.run(self.raw_input.clone(), |ctx| {
            cb(ctx, data);
        });
        // non render output from egui
        let _non_render_output = full_output.platform_output;

        // render output from egui
        let clipped_primitives = self.ctx.tessellate(full_output.shapes, full_output.pixels_per_point);

        // textures to create before rendering
        self.create_textures(full_output.textures_delta.set);

        let mut primitive_meshes = Vec::new();
        for ClippedPrimitive {clip_rect, primitive} in clipped_primitives {
            // TODO: scissor takes in "pixels", make sure its the same as logical pixels from egui
            let scissor = vk::Rect2D {
                offset: vk::Offset2D {x: clip_rect.min.x as i32, y: clip_rect.min.y as i32},
                extent: vk::Extent2D {
                    width: (clip_rect.max.x-clip_rect.min.x) as u32,
                    height: (clip_rect.max.y-clip_rect.min.y) as u32
                },
            };
            if let Primitive::Mesh(Mesh {indices, vertices, texture_id: txtr_id}) = primitive {
                // println!("MESH {indices:?} {vertices:?}");
                primitive_meshes.push((vertices, indices, scissor, txtr_id));
            }
        }

        // textures to free/destroy will happen at the end of program
        // (assuming we dont help egui to add new texture in between execution)

        primitive_meshes
    }

    unsafe fn destroy(&self) {
        for (txtr_id, txtr) in &self.textures {
            println!("FREE TEXTURE {txtr_id:?}");

            self.device.destroy_sampler(txtr.sampler, None);
            if let Some(img_view) = txtr.img_view {
                self.device.destroy_image_view(img_view, None);
            }

            self.device.destroy_buffer(txtr.host_buf, None);
            self.device.free_memory(txtr.host_buf_mem, None);

            self.device.destroy_image(txtr.local_img, None);
            self.device.free_memory(txtr.local_img_mem, None);
        }
    }

    fn convert_filter(filter: TextureFilter) -> vk::Filter {
        match filter {
            TextureFilter::Linear => vk::Filter::LINEAR,
            TextureFilter::Nearest => vk::Filter::NEAREST
        }
    }

    unsafe fn create_textures(&mut self, new_txtrs: Vec<(TextureId, ImageDelta)>) {
        for (txtr_id, ImageDelta {image, options, pos} ) in new_txtrs {
            // TODO: create vk buffer image
            println!("SET {txtr_id:?}");

            let (host_buf, host_buf_mem, local_img, local_img_mem, extent) = match image {
                ImageData::Color(color) => {
                    println!("COLOR");

                    let bytes = color.as_raw();
                    let img_extent = vk::Extent3D {
                        width: color.width() as u32,
                        height: color.height() as u32,
                        depth: 1,
                    };

                    let (buf, buf_mem, _, _) = create_host_buffer(self.vi.clone(), self.device.clone(), bytes, vk::BufferUsageFlags::TRANSFER_SRC, true);

                    let (img, img_mem) = create_local_image(self.vi.clone(), self.device.clone(), img_extent, 1, vk::Format::R8G8B8A8_SRGB, vk::ImageUsageFlags::TRANSFER_DST | vk::ImageUsageFlags::SAMPLED, None);

                    (buf, buf_mem, img, img_mem, img_extent)
                }
                ImageData::Font(font) => {
                    println!("FONT");

                    // vk::Format::R32_SFLOAT  // red pixel representing coverage (alpha)
                    let bytes = font.srgba_pixels(None)
                        .map(|col| {
                            col.to_srgba_unmultiplied()
                        })
                        .fold(Vec::new(), |mut b, next| {
                            b.extend_from_slice(&next);
                            b
                        });

                    let img_extent = vk::Extent3D {
                        width: font.width() as u32,
                        height: font.height() as u32,
                        depth: 1,
                    };

                    let (buf, buf_mem, _, _) = create_host_buffer(self.vi.clone(), self.device.clone(), &bytes, vk::BufferUsageFlags::TRANSFER_SRC, true);

                    let (img, img_mem) = create_local_image(self.vi.clone(), self.device.clone(), img_extent, 1, vk::Format::R8G8B8A8_SRGB, vk::ImageUsageFlags::TRANSFER_DST | vk::ImageUsageFlags::SAMPLED, None);

                    (buf, buf_mem, img, img_mem, img_extent)
                }
            };

            let sampler_info = vk::SamplerCreateInfo {
                mag_filter: Self::convert_filter(options.magnification),
                min_filter: Self::convert_filter(options.minification),
                address_mode_u: vk::SamplerAddressMode::CLAMP_TO_EDGE,
                address_mode_v: vk::SamplerAddressMode::CLAMP_TO_EDGE,
                address_mode_w: vk::SamplerAddressMode::CLAMP_TO_EDGE,
                anisotropy_enable: vk::FALSE,
                border_color: vk::BorderColor::INT_OPAQUE_BLACK,
                unnormalized_coordinates: vk::FALSE,
                compare_enable: vk::FALSE,
                compare_op: vk::CompareOp::ALWAYS,
                mipmap_mode: vk::SamplerMipmapMode::NEAREST,
                mip_lod_bias: 0.0,
                min_lod: 0.0,
                max_lod: 0.0,
                ..Default::default()
            };
            let sampler = self.device.create_sampler(&sampler_info, None)
                .expect("Failed to create UI sampler");

            self.textures.insert(txtr_id, UITextureDescriptor {
                sampler, img_view: None, host_buf, host_buf_mem, local_img, local_img_mem, extent
            });
        }
    }

    unsafe fn transfer_img_barrier(&self, cmd_buf: vk::CommandBuffer, ui_txtr: &UITextureDescriptor) {
        // transition image layout to prepare for transfer

        let transfer_barrier = vk::ImageMemoryBarrier {
            old_layout: vk::ImageLayout::UNDEFINED,
            new_layout: vk::ImageLayout::TRANSFER_DST_OPTIMAL,
            src_queue_family_index: vk::QUEUE_FAMILY_IGNORED,
            dst_queue_family_index: vk::QUEUE_FAMILY_IGNORED,
            image: ui_txtr.local_img,
            subresource_range: vk::ImageSubresourceRange {
                aspect_mask: vk::ImageAspectFlags::COLOR,
                base_mip_level: 0,
                level_count: 1,
                base_array_layer: 0,
                layer_count: 1,
            },
            src_access_mask: vk::AccessFlags::empty(),
            dst_access_mask: vk::AccessFlags::TRANSFER_WRITE,
            ..Default::default()
        };
        self.device.cmd_pipeline_barrier(
            cmd_buf, vk::PipelineStageFlags::TOP_OF_PIPE, vk::PipelineStageFlags::TRANSFER,
            vk::DependencyFlags::empty(), &[], &[], &[transfer_barrier]
        );

        // copy buffer to image

        let region = vk::BufferImageCopy {
            buffer_offset: 0,
            buffer_row_length: 0,
            buffer_image_height: 0,
            image_subresource: vk::ImageSubresourceLayers {
                aspect_mask: vk::ImageAspectFlags::COLOR,
                mip_level: 0,
                base_array_layer: 0,
                layer_count: 1,
            },
            image_offset: vk::Offset3D {x:0, y:0, z:0},
            image_extent: ui_txtr.extent,
        };
        self.device.cmd_copy_buffer_to_image(
            cmd_buf, ui_txtr.host_buf, ui_txtr.local_img, vk::ImageLayout::TRANSFER_DST_OPTIMAL, &[region]
        );

        // transition image layout from transfer to be read by shaders

        let shader_barrier = vk::ImageMemoryBarrier {
            old_layout: vk::ImageLayout::TRANSFER_DST_OPTIMAL,
            new_layout: vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
            src_queue_family_index: vk::QUEUE_FAMILY_IGNORED,
            dst_queue_family_index: vk::QUEUE_FAMILY_IGNORED,
            image: ui_txtr.local_img,
            subresource_range: vk::ImageSubresourceRange {
                aspect_mask: vk::ImageAspectFlags::COLOR,
                base_mip_level: 0,
                level_count: 1,
                base_array_layer: 0,
                layer_count: 1,
            },
            src_access_mask: vk::AccessFlags::TRANSFER_WRITE,
            dst_access_mask: vk::AccessFlags::SHADER_READ,
            ..Default::default()
        };
        self.device.cmd_pipeline_barrier(
            cmd_buf, vk::PipelineStageFlags::TRANSFER, vk::PipelineStageFlags::FRAGMENT_SHADER,
            vk::DependencyFlags::empty(), &[], &[], &[shader_barrier]
        );
    }

    unsafe fn create_img_views(&mut self) {
        for (_, txtr) in &mut self.textures {
            let img_view_info = vk::ImageViewCreateInfo {
                image: txtr.local_img,
                view_type: vk::ImageViewType::TYPE_2D,
                format: vk::Format::R8G8B8A8_SRGB,
                subresource_range: vk::ImageSubresourceRange {
                    aspect_mask: vk::ImageAspectFlags::COLOR,
                    base_mip_level: 0,
                    level_count: 1,
                    base_array_layer: 0,
                    layer_count: 1,
                },
                ..Default::default()
            };
            txtr.img_view.replace(
                self.device.create_image_view(&img_view_info, None)
                    .expect("Failed to create texture image view")
            );
        }
    }
}
