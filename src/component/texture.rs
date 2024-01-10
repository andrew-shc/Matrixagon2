use std::fs;
use std::rc::Rc;
use ash::{Device, vk};
use png;
use crate::component::{Component, ComponentEventResponse, RenderData, RenderDataPurpose};
use crate::handler::VulkanInstance;
use crate::util::{cmd_recording, create_host_buffer, create_local_image};
use crate::world::{WorldEvent, WorldState};

pub(crate) struct TextureHandler {
    device: Rc<Device>,

    staging_buf: vk::Buffer,
    staging_buf_mem: vk::DeviceMemory,
    img: vk::Image,
    img_mem: vk::DeviceMemory,
    img_extent: vk::Extent3D,
    // two modes of accessing image: ImgView for simple viewing of image,
    //      Sampler for frag shader to sample textures (distinct from image)
    img_view: Option<vk::ImageView>,
    img_sampler: vk::Sampler,
}

impl TextureHandler {
    pub(crate) fn new(vi: Rc<VulkanInstance>, device: Rc<Device>) -> Self {
        let decoder = png::Decoder::new(
            fs::File::open("C:/Users/andrewshen/documents/matrixagon2/src/resource/block_textures/dirt.png").unwrap()
        );
        let mut reader = decoder.read_info().unwrap();
        let mut raw_buf = vec![0; reader.output_buffer_size()];
        let info = reader.next_frame(&mut raw_buf).unwrap();

        println!("TEXTURE FORMAT {:?}", info.color_type);

        let bytes = &raw_buf[..info.buffer_size()];
        let img_extent = vk::Extent3D {
            width: info.width, height: info.height, depth: 1
        };

        unsafe {
            let (buf, buf_mem, _, _) = create_host_buffer(vi.clone(), device.clone(), bytes, vk::BufferUsageFlags::TRANSFER_SRC, true);

            let (img, img_mem) = create_local_image(vi.clone(), device.clone(), img_extent, vk::Format::R8G8B8A8_SRGB, vk::ImageUsageFlags::TRANSFER_DST | vk::ImageUsageFlags::SAMPLED,);

            let sampler_info = vk::SamplerCreateInfo {
                mag_filter: vk::Filter::NEAREST,
                min_filter: vk::Filter::NEAREST,
                address_mode_u: vk::SamplerAddressMode::REPEAT,
                address_mode_v: vk::SamplerAddressMode::REPEAT,
                address_mode_w: vk::SamplerAddressMode::REPEAT,
                anisotropy_enable: vk::TRUE,
                max_anisotropy: vi.get_physical_device_properties().limits.max_sampler_anisotropy,
                border_color: vk::BorderColor::INT_OPAQUE_BLACK,
                unnormalized_coordinates: vk::FALSE,
                compare_enable: vk::FALSE,
                compare_op: vk::CompareOp::ALWAYS,
                mipmap_mode: vk::SamplerMipmapMode::LINEAR,
                mip_lod_bias: 0.0,
                min_lod: 0.0,
                max_lod: 0.0,
                ..Default::default()
            };
            let sampler = device.create_sampler(&sampler_info, None)
                .expect("Failed to create sampler");

            Self {
                device,
                staging_buf: buf,
                staging_buf_mem: buf_mem,
                img,
                img_mem,
                img_extent,
                img_view: None,
                img_sampler: sampler,
            }
        }
    }

    fn record(&self) -> impl FnMut(vk::CommandBuffer)+'_ {
        |cmd_buf| unsafe {
            // transition image layout to prepare for transfer

            let transfer_barrier = vk::ImageMemoryBarrier {
                old_layout: vk::ImageLayout::UNDEFINED,
                new_layout: vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                src_queue_family_index: vk::QUEUE_FAMILY_IGNORED,
                dst_queue_family_index: vk::QUEUE_FAMILY_IGNORED,
                image: self.img,
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
                image_extent: self.img_extent,
            };
            self.device.cmd_copy_buffer_to_image(
                cmd_buf, self.staging_buf, self.img, vk::ImageLayout::TRANSFER_DST_OPTIMAL, &[region]
            );

            // transition image layout from transfer to be read by shaders

            let shader_barrier = vk::ImageMemoryBarrier {
                old_layout: vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                new_layout: vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
                src_queue_family_index: vk::QUEUE_FAMILY_IGNORED,
                dst_queue_family_index: vk::QUEUE_FAMILY_IGNORED,
                image: self.img,
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
    }
}

impl Component for TextureHandler {
    fn render(&self) -> Vec<RenderData> {
        vec![]
    }

    fn respond_event(&mut self, event: WorldEvent) -> ComponentEventResponse {
        ComponentEventResponse::default()
    }

    fn update_state(&mut self, state: &mut WorldState) {

    }

    unsafe fn load_descriptors(&mut self, cmd_pool: vk::CommandPool, queue: vk::Queue) -> Vec<RenderData> {
        cmd_recording(self.device.clone(), cmd_pool, queue, self.record());

        let img_view_info = vk::ImageViewCreateInfo {
            image: self.img,
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
        self.img_view.replace(
            self.device.create_image_view(&img_view_info, None)
                .expect("Failed to create texture image view")
        );

        vec![RenderData::InitialDescriptorImage(
            vec![vk::DescriptorImageInfo {
                image_layout: vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
                image_view: self.img_view.unwrap(),
                sampler: self.img_sampler,
            }],
            RenderDataPurpose::BlockTextures
        )]
    }

    unsafe fn destroy_descriptor(&mut self) {
        self.device.destroy_sampler(self.img_sampler, None);
        if let Some(img_view) = self.img_view {
            self.device.destroy_image_view(img_view, None);
        }

        self.device.destroy_buffer(self.staging_buf, None);
        self.device.free_memory(self.staging_buf_mem, None);

        self.device.destroy_image(self.img, None);
        self.device.free_memory(self.img_mem, None);
    }
}
