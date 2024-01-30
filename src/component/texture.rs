use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::rc::Rc;
use ash::{Device, vk};
use png;
use crate::component::{Component, RenderData, RenderDataPurpose};
use crate::handler::VulkanInstance;
use crate::util::{cmd_recording, create_host_buffer, create_local_image};
use crate::world::{WorldEvent};


pub(crate) type TextureIDMapper = Rc<HashMap<String, u32>>;

pub(crate) struct TextureHandler {
    vi: Rc<VulkanInstance>,
    device: Rc<Device>,

    staging_buf: vk::Buffer,
    staging_buf_mem: vk::DeviceMemory,
    staging_buf_offsets: Vec<usize>,
    img: vk::Image,
    img_fmt: vk::Format,
    img_mem: vk::DeviceMemory,
    img_extent: vk::Extent3D,
    // two modes of accessing image: ImgView for simple viewing of image,
    //      Sampler for frag shader to sample textures (distinct from image)
    img_view: Option<vk::ImageView>,
    img_sampler: vk::Sampler,

    txtr_mapper: TextureIDMapper,
    txtr_len: u32,
}

impl TextureHandler {
    const TEXTURE_MIPMAP_LEVELS: u32 = 4;
    pub(crate) fn new(vi: Rc<VulkanInstance>, device: Rc<Device>, txtr_path: Vec<&Path>) -> Self {
        let mut raw_buf = Vec::new();
        let mut offsets = Vec::with_capacity(txtr_path.len());
        let mut mapper = HashMap::new();
        offsets.push(0);

        let img_fmt = vk::Format::R8G8B8A8_SRGB;
        let mut img_extent = None;
        let mut txtr_mapper = HashMap::new();
        for (ind, path) in txtr_path.iter().enumerate() {
            println!("LOADING TEXTURE [{}]: {:?}", ind, path.file_stem().unwrap());
            txtr_mapper.insert(String::from(path.file_stem().unwrap().to_str().unwrap()), ind as u32);

            let decoder = png::Decoder::new(
                fs::File::open(path).unwrap()
            );
            let mut reader = decoder.read_info().unwrap();
            let mut txtr_raw_buf = vec![0; reader.output_buffer_size()];
            let info = reader.next_frame(&mut txtr_raw_buf).unwrap();
            println!("\tTEXTURE FORMAT {:?}", info.color_type);

            if let None = img_extent {
                img_extent.replace(vk::Extent3D {
                    width: info.width, height: info.height, depth: 1
                });
                raw_buf.reserve(reader.output_buffer_size()*txtr_path.len());
            } else if let Some(extent) = img_extent {
                if extent.width != info.width || extent.height != info.height {
                    panic!("Texture <{:?}> has a different extent compared to the first", path);
                }
            }

            if offsets.len() < txtr_path.len() {
                // since the offsets starts with an initial length of 1 at 0 offset
                offsets.push(offsets.last().unwrap()+reader.output_buffer_size());
            }
            raw_buf.append(&mut txtr_raw_buf);
            mapper.insert(path.file_stem().unwrap(), ind);
        }

        // let decoder = png::Decoder::new(
        //     fs::File::open("C:/Users/andrewshen/documents/matrixagon2/src/resource/block_textures/grass_side.png").unwrap()
        // );
        // let mut reader = decoder.read_info().unwrap();
        // let mut raw_buf = vec![0; reader.output_buffer_size()];
        // let info = reader.next_frame(&mut raw_buf).unwrap();
        //
        // println!("TEXTURE FORMAT {:?}", info.color_type);
        // let img_fmt = vk::Format::R8G8B8A8_SRGB;
        //
        // let bytes = &raw_buf[..info.buffer_size()];
        // let img_extent = vk::Extent3D {
        //     width: info.width, height: info.height, depth: 1
        // };

        unsafe {
            let (buf, buf_mem, _, _) = create_host_buffer(vi.clone(), device.clone(), &raw_buf, vk::BufferUsageFlags::TRANSFER_SRC, true);

            let (img, img_mem) = create_local_image(
                vi.clone(), device.clone(), img_extent.unwrap(), Self::TEXTURE_MIPMAP_LEVELS, img_fmt,
                vk::ImageUsageFlags::TRANSFER_SRC | vk::ImageUsageFlags::TRANSFER_DST | vk::ImageUsageFlags::SAMPLED,
                Some(txtr_path.len() as u32),
            );

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
                max_lod: Self::TEXTURE_MIPMAP_LEVELS as f32,
                ..Default::default()
            };
            let sampler = device.create_sampler(&sampler_info, None)
                .expect("Failed to create sampler");

            let txtr_len = offsets.clone().len() as u32;
            println!("TEXTURE ARRAY IMAGE LAYERS: {}", txtr_len);

            Self {
                vi, device,
                staging_buf: buf,
                staging_buf_mem: buf_mem,
                staging_buf_offsets: offsets,
                img,
                img_fmt,
                img_mem,
                img_extent: img_extent.unwrap(),
                img_view: None,
                img_sampler: sampler,
                txtr_mapper: Rc::new(txtr_mapper) as TextureIDMapper,
                txtr_len,
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
                    level_count: Self::TEXTURE_MIPMAP_LEVELS,
                    base_array_layer: 0,
                    layer_count: self.txtr_len,
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

            let mut region_layers = Vec::with_capacity(self.staging_buf_offsets.len());
            for (ind, ofs) in self.staging_buf_offsets.iter().enumerate() {
                let region = vk::BufferImageCopy {
                    buffer_offset: *ofs as vk::DeviceSize,
                    buffer_row_length: 0,
                    buffer_image_height: 0,
                    image_subresource: vk::ImageSubresourceLayers {
                        aspect_mask: vk::ImageAspectFlags::COLOR,
                        mip_level: 0,
                        base_array_layer: ind as u32,
                        layer_count: 1,
                    },
                    image_offset: vk::Offset3D {x:0, y:0, z:0},
                    image_extent: self.img_extent,
                };
                region_layers.push(region);
            }
            self.device.cmd_copy_buffer_to_image(
                cmd_buf, self.staging_buf, self.img, vk::ImageLayout::TRANSFER_DST_OPTIMAL, &region_layers
            );

            // generating mipmaps & transitioning each mipmap level to be read by shaders
            let prop = self.vi.get_physical_device_format_properties(self.img_fmt);
            if !prop.optimal_tiling_features.contains(vk::FormatFeatureFlags::SAMPLED_IMAGE_FILTER_LINEAR) {
                panic!("This device does not support linear blitting for mipmaps");
            }

            let mut mipmap_barrier = vk::ImageMemoryBarrier {
                image: self.img,
                src_queue_family_index: vk::QUEUE_FAMILY_IGNORED,
                dst_queue_family_index: vk::QUEUE_FAMILY_IGNORED,
                subresource_range: vk::ImageSubresourceRange {
                    aspect_mask: vk::ImageAspectFlags::COLOR,
                    base_mip_level: 0,
                    level_count: 1,
                    base_array_layer: 0,
                    layer_count: self.txtr_len,
                },
                ..Default::default()
            };
            let mut mip_width = self.img_extent.width.clone();
            let mut mip_height = self.img_extent.height.clone();

            // for layer in 0..self.staging_buf_offsets.len() {
            //
            // }
            for i in 1..Self::TEXTURE_MIPMAP_LEVELS {
                mipmap_barrier.subresource_range.base_mip_level = i-1;
                mipmap_barrier.old_layout = vk::ImageLayout::TRANSFER_DST_OPTIMAL;
                mipmap_barrier.new_layout = vk::ImageLayout::TRANSFER_SRC_OPTIMAL;
                mipmap_barrier.src_access_mask = vk::AccessFlags::TRANSFER_WRITE;
                mipmap_barrier.dst_access_mask = vk::AccessFlags::TRANSFER_READ;

                self.device.cmd_pipeline_barrier(
                    cmd_buf, vk::PipelineStageFlags::TRANSFER, vk::PipelineStageFlags::TRANSFER,
                    vk::DependencyFlags::empty(), &[], &[], &[mipmap_barrier]
                );

                let src_offset = vk::Offset3D {x: mip_width as i32, y: mip_height as i32, z: 1};
                if mip_width > 1 {mip_width /= 2}
                if mip_height > 1 {mip_height /= 2}
                let dst_offset = vk::Offset3D {x: mip_width as i32, y: mip_height as i32, z: 1};

                let blit = vk::ImageBlit {
                    src_offsets: [vk::Offset3D {x: 0, y: 0, z: 0}, src_offset],
                    src_subresource: vk::ImageSubresourceLayers {
                        aspect_mask: vk::ImageAspectFlags::COLOR,
                        mip_level: i-1,
                        base_array_layer: 0,
                        layer_count: self.txtr_len,
                    },
                    dst_offsets: [vk::Offset3D {x: 0, y: 0, z: 0}, dst_offset],
                    dst_subresource: vk::ImageSubresourceLayers {
                        aspect_mask: vk::ImageAspectFlags::COLOR,
                        mip_level: i,
                        base_array_layer: 0,
                        layer_count: self.txtr_len,
                    },
                };

                self.device.cmd_blit_image(
                    cmd_buf, self.img, vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
                    self.img, vk::ImageLayout::TRANSFER_DST_OPTIMAL, &[blit], vk::Filter::LINEAR
                );

                mipmap_barrier.old_layout = vk::ImageLayout::TRANSFER_SRC_OPTIMAL;
                mipmap_barrier.new_layout = vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL;
                mipmap_barrier.src_access_mask = vk::AccessFlags::TRANSFER_READ;
                mipmap_barrier.dst_access_mask = vk::AccessFlags::SHADER_READ;

                self.device.cmd_pipeline_barrier(
                    cmd_buf, vk::PipelineStageFlags::TRANSFER, vk::PipelineStageFlags::FRAGMENT_SHADER,
                    vk::DependencyFlags::empty(), &[], &[], &[mipmap_barrier]
                );
            }

            mipmap_barrier.subresource_range.base_mip_level = Self::TEXTURE_MIPMAP_LEVELS-1;
            mipmap_barrier.old_layout = vk::ImageLayout::TRANSFER_DST_OPTIMAL;
            mipmap_barrier.new_layout = vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL;
            mipmap_barrier.src_access_mask = vk::AccessFlags::TRANSFER_WRITE;
            mipmap_barrier.dst_access_mask = vk::AccessFlags::SHADER_READ;

            self.device.cmd_pipeline_barrier(
                cmd_buf, vk::PipelineStageFlags::TRANSFER, vk::PipelineStageFlags::FRAGMENT_SHADER,
                vk::DependencyFlags::empty(), &[], &[], &[mipmap_barrier]
            );
        }
    }
}

impl Component for TextureHandler {
    fn render(&self) -> Vec<RenderData> {
        vec![]
    }

    fn respond_event(&mut self, event: WorldEvent) -> Vec<WorldEvent> {
        match event {
            WorldEvent::Start => {
                vec![
                    WorldEvent::NewTextureMapper(self.txtr_mapper.clone())
                ]
            }
            _ => {
                vec![]
            }
        }
    }

    fn update(&mut self) {

    }

    unsafe fn load_descriptors(&mut self, cmd_pool: vk::CommandPool, queue: vk::Queue) -> Vec<RenderData> {
        cmd_recording(self.device.clone(), cmd_pool, queue, self.record());

        let img_view_info = vk::ImageViewCreateInfo {
            image: self.img,
            view_type: vk::ImageViewType::TYPE_2D_ARRAY,
            format: vk::Format::R8G8B8A8_SRGB,
            subresource_range: vk::ImageSubresourceRange {
                aspect_mask: vk::ImageAspectFlags::COLOR,
                base_mip_level: 0,
                level_count: Self::TEXTURE_MIPMAP_LEVELS,
                base_array_layer: 0,
                layer_count: self.staging_buf_offsets.len() as u32,
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

    unsafe fn destroy(&mut self) {
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
