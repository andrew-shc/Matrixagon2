use std::rc::Rc;
use ash::{Device, vk};
use crate::component::{RenderData, RenderDataPurpose};
use crate::debug::DebugVisibility;
use crate::handler::VulkanInstance;
use crate::swapchain::SwapchainManager;
use crate::util::create_local_image;


// presentation attachment is not included
// all AttachmentRef refers to all elective attachments (including depth/stencil)
#[derive(Copy, Clone, Debug)]
pub(crate) enum AttachmentRef {
    Color,
    ColorInput,  // color attachment that can be also used as input (attachments)
    Depth, // uses its own depth format
}


pub(crate) struct FramebufferManager {
    device: Rc<Device>,

    prsnt_imgvs: Vec<vk::ImageView>,  // presenting images are controlled by swapchain
    attachment_imgs: Vec<vk::Image>,  // ordered as per the attachment references initial param after the presentation image (if there is one)
    attachment_imgvs: Vec<vk::ImageView>,
    attachment_imgms: Vec<vk::DeviceMemory>,
    inp_attachment_imgvs: Vec<vk::ImageView>,
    pub(crate) framebuffers: Vec<vk::Framebuffer>,  // duplicated to the same amount as presentation images
}

impl FramebufferManager {
    // pub(crate) unsafe fn new(renderpass: vk::RenderPass, attachments: Vec<vk::AttachmentReference>) -> Self {
    //     Self {
    //
    //     }
    // }

    pub(crate) unsafe fn new_swapchain_bounded(
        dbv: DebugVisibility, vi: Rc<VulkanInstance>, device: Rc<Device>, renderpass: vk::RenderPass,
        attachments: Vec<AttachmentRef>, prsnt_imgs: Vec<vk::Image>,
        color_fmt: vk::Format, depth_fmt: vk::Format, extent: vk::Extent2D,
        prsnt_inp: bool,
    ) -> Self {
        let mut attachment_imgs = Vec::new();
        let mut attachment_imgvs = Vec::new();
        let mut attachment_imgms = Vec::new();
        let mut inp_attachment_imgvs = Vec::new();

        if dbv.vk_swapchain_output {
            println!("NEW FB ATTACHMENTS {attachments:?}");
        }
        for attachment in attachments {
            if dbv.vk_swapchain_output {
                println!("FB ATTACHMENT {attachment:?}");
            }
            match attachment {
                AttachmentRef::Depth => {
                    // TODO: maybe we can but the image format (color, depth, etc.) separate
                    // TODO: when creating image buffer
                    let (depth_img, depth_img_mem) = create_local_image(
                        vi.clone(), device.clone(),
                        vk::Extent3D {width: extent.width, height: extent.height, depth: 1},
                        1, depth_fmt, vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT,
                    );

                    let depth_imgv_create_info = vk::ImageViewCreateInfo {
                        image: depth_img,
                        view_type: vk::ImageViewType::TYPE_2D,
                        format: depth_fmt,
                        components: vk::ComponentMapping {
                            r: vk::ComponentSwizzle::IDENTITY,
                            g: vk::ComponentSwizzle::IDENTITY,
                            b: vk::ComponentSwizzle::IDENTITY,
                            a: vk::ComponentSwizzle::IDENTITY,
                        },
                        subresource_range: vk::ImageSubresourceRange {
                            aspect_mask: vk::ImageAspectFlags::DEPTH,
                            base_mip_level: 0,
                            level_count: 1,
                            base_array_layer: 0,
                            layer_count: 1,
                        },
                        ..Default::default()
                    };
                    let depth_view = device.create_image_view(&depth_imgv_create_info, None)
                        .expect("Failed to create image view");

                    attachment_imgs.push(depth_img);
                    attachment_imgvs.push(depth_view);
                    attachment_imgms.push(depth_img_mem);
                }
                AttachmentRef::Color => {
                    let (color_img, color_img_mem) = create_local_image(
                        vi.clone(), device.clone(),
                        vk::Extent3D {width: extent.width, height: extent.height, depth: 1},
                        1, color_fmt, vk::ImageUsageFlags::COLOR_ATTACHMENT,
                    );

                    let color_imgv_create_info = vk::ImageViewCreateInfo {
                        image: color_img,
                        view_type: vk::ImageViewType::TYPE_2D,
                        format: color_fmt,
                        components: vk::ComponentMapping {
                            r: vk::ComponentSwizzle::IDENTITY,
                            g: vk::ComponentSwizzle::IDENTITY,
                            b: vk::ComponentSwizzle::IDENTITY,
                            a: vk::ComponentSwizzle::IDENTITY,
                        },
                        subresource_range: vk::ImageSubresourceRange {
                            aspect_mask: vk::ImageAspectFlags::COLOR,
                            base_mip_level: 0,
                            level_count: 1,
                            base_array_layer: 0,
                            layer_count: 1,
                        },
                        ..Default::default()
                    };
                    let color_view = device.create_image_view(&color_imgv_create_info, None)
                        .expect("Failed to create image view");

                    attachment_imgs.push(color_img);
                    attachment_imgvs.push(color_view);

                    attachment_imgms.push(color_img_mem);
                }
                AttachmentRef::ColorInput => {
                    let (color_img, color_img_mem) = create_local_image(
                        vi.clone(), device.clone(),
                        vk::Extent3D {width: extent.width, height: extent.height, depth: 1},
                        1, color_fmt, vk::ImageUsageFlags::INPUT_ATTACHMENT | vk::ImageUsageFlags::COLOR_ATTACHMENT,
                    );

                    let color_imgv_create_info = vk::ImageViewCreateInfo {
                        image: color_img,
                        view_type: vk::ImageViewType::TYPE_2D,
                        format: color_fmt,
                        components: vk::ComponentMapping {
                            r: vk::ComponentSwizzle::IDENTITY,
                            g: vk::ComponentSwizzle::IDENTITY,
                            b: vk::ComponentSwizzle::IDENTITY,
                            a: vk::ComponentSwizzle::IDENTITY,
                        },
                        subresource_range: vk::ImageSubresourceRange {
                            aspect_mask: vk::ImageAspectFlags::COLOR,
                            base_mip_level: 0,
                            level_count: 1,
                            base_array_layer: 0,
                            layer_count: 1,
                        },
                        ..Default::default()
                    };
                    let color_view = device.create_image_view(&color_imgv_create_info, None)
                        .expect("Failed to create image view");

                    attachment_imgs.push(color_img);
                    attachment_imgvs.push(color_view);
                    attachment_imgms.push(color_img_mem);
                    inp_attachment_imgvs.push(color_view);
                }
            }
        }

        let mut prsnt_imgvs = Vec::new();
        let mut framebuffers = Vec::new();
        for swapchain_image in &prsnt_imgs {
            let prsnt_view_create_info = vk::ImageViewCreateInfo {
                image: *swapchain_image,
                view_type: vk::ImageViewType::TYPE_2D,
                format: color_fmt,
                components: vk::ComponentMapping {
                    r: vk::ComponentSwizzle::IDENTITY,
                    g: vk::ComponentSwizzle::IDENTITY,
                    b: vk::ComponentSwizzle::IDENTITY,
                    a: vk::ComponentSwizzle::IDENTITY,
                },
                subresource_range: vk::ImageSubresourceRange {
                    aspect_mask: vk::ImageAspectFlags::COLOR,
                    base_mip_level: 0,
                    level_count: 1,
                    base_array_layer: 0,
                    layer_count: 1,
                },
                ..Default::default()
            };

            let view = device.create_image_view(&prsnt_view_create_info, None)
                .expect("Failed to create image view");

            let mut attachments = attachment_imgvs.clone();
            attachments.insert(0, view);

            let framebuffer_info = vk::FramebufferCreateInfo::builder()
                .render_pass(renderpass)
                .attachments(&attachments)
                .width(extent.width)
                .height(extent.height)
                .layers(1)
                .build();

            let fb = device.create_framebuffer(&framebuffer_info, None)
                .expect("Failed to create framebuffer");

            prsnt_imgvs.push(view);
            if prsnt_inp {
                inp_attachment_imgvs.push(view);
            }
            framebuffers.push(fb);
        }

        Self {
            device,prsnt_imgvs, attachment_imgs, attachment_imgvs, attachment_imgms,
            inp_attachment_imgvs, framebuffers
        }
    }

    pub(crate) unsafe fn get_input_attachment_descriptors(&self) -> Vec<RenderData> {
        self.inp_attachment_imgvs.iter()
            .map(|imgv| {
                RenderData::InitialDescriptorImage(
                    vec![vk::DescriptorImageInfo {
                        sampler: vk::Sampler::null(),
                        image_view: *imgv,
                        image_layout: vk::ImageLayout::READ_ONLY_OPTIMAL,  // TODO: change for presentation attachment as input?
                    }],
                    RenderDataPurpose::PresentationInpAttachment,  // TODO: change to variable
                )
            })
            .collect()
    }

    pub(crate) unsafe fn destroy(&self) {
        for attachment_imgv in &self.attachment_imgvs {
            self.device.destroy_image_view(*attachment_imgv, None);
        }
        for attachment_img in &self.attachment_imgs {
            self.device.destroy_image(*attachment_img, None);
        }
        for attachment_imgm in &self.attachment_imgms {
            self.device.free_memory(*attachment_imgm, None);
        }


        for prsnt_imgv in &self.prsnt_imgvs {
            self.device.destroy_image_view(*prsnt_imgv, None);
        }

        for framebuffer in &self.framebuffers {
            self.device.destroy_framebuffer(*framebuffer, None);
        }
    }
}
