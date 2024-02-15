use std::rc::Rc;
use ash::{Device, vk};
use ash::extensions::khr::Swapchain;
use crate::debug::DebugVisibility;
use crate::framebuffer::{FBAttachmentRef, FramebufferManager};
use crate::handler::VulkanInstance;

pub(crate) struct SwapchainManager {
    dbv: DebugVisibility,
    vi: Rc<VulkanInstance>,
    device: Rc<Device>,

    pub(crate) loader: Swapchain,
    pub(crate) swapchain: vk::SwapchainKHR,
    pub(crate) extent: vk::Extent2D,
    pub(crate) capb: vk::SurfaceCapabilitiesKHR,
    pub(crate) fmt: vk::SurfaceFormatKHR,
    pub(crate) prsnt: vk::PresentModeKHR,
    pub(crate) fbm: FramebufferManager,

    prsnt_inp: bool,

    // per renderpass
    renderpass: vk::RenderPass,
    attachments: Vec<FBAttachmentRef>,
}

impl SwapchainManager {
    pub(crate) unsafe fn new(
        dbv: DebugVisibility, vi: Rc<VulkanInstance>, device: Rc<Device>,
        renderpass: vk::RenderPass, attachments: Vec<FBAttachmentRef>, prsnt_inp: bool,
    ) -> Self {
        // prsnt_inp: make the presentation attachment also an input attachment

        let (capb, fmt, prsnt) = query_swapchain_support(dbv, &vi);
        let (fmt, prsnt) = select_swapchain_support(fmt, prsnt);

        let swapchain_loader = Swapchain::new(&vi.inst, &device.clone());

        // assuming graphics and presentation queue families are the same ind.
        let swapchain_create_info = vk::SwapchainCreateInfoKHR {
            surface: vi.surf,
            min_image_count: capb.min_image_count+1,
            image_format: fmt.format,
            image_color_space: fmt.color_space,
            image_extent: capb.current_extent,
            image_array_layers: 1,
            image_usage: if prsnt_inp {
                vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::INPUT_ATTACHMENT
            }  else {
                vk::ImageUsageFlags::COLOR_ATTACHMENT
            },
            image_sharing_mode: vk::SharingMode::EXCLUSIVE,
            pre_transform: capb.current_transform,
            composite_alpha: vk::CompositeAlphaFlagsKHR::OPAQUE,
            present_mode: prsnt,
            clipped: vk::TRUE,
            old_swapchain: vk::SwapchainKHR::null(),
            ..Default::default()
        };

        let swapchain = swapchain_loader.create_swapchain(&swapchain_create_info, None)
            .expect("Failed to create swapchain");

        if dbv.vk_swapchain_output {
            println!("Swapchain Object: {:?}", swapchain);
        }

        let swapchain_images = swapchain_loader.get_swapchain_images(swapchain)
            .expect("Failed to get swapchain images");

        let fbm = FramebufferManager::new_swapchain_bounded(
            dbv, vi.clone(), device.clone(), renderpass, attachments.clone(), swapchain_images,
            fmt.format, best_depth_format_support(), capb.current_extent, prsnt_inp
        );

        Self {
            dbv, vi: vi.clone(), device: device.clone(),
            loader: swapchain_loader, swapchain, extent: capb.current_extent, capb, fmt, prsnt, fbm,
            prsnt_inp, renderpass, attachments,
        }
    }

    pub(crate) unsafe fn recreate(&mut self) {
        let (capb, fmt, prsnt) = query_swapchain_support(self.dbv, &self.vi);
        let (fmt, prsnt) = select_swapchain_support(fmt, prsnt);

        // assuming graphics and presentation queue families are the same ind.
        let swapchain_create_info = vk::SwapchainCreateInfoKHR {
            surface: self.vi.surf,
            min_image_count: capb.min_image_count+1,
            image_format: fmt.format,
            image_color_space: fmt.color_space,
            image_extent: capb.current_extent,
            image_array_layers: 1,
            image_usage: if self.prsnt_inp {
                vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::INPUT_ATTACHMENT
            } else {
                vk::ImageUsageFlags::COLOR_ATTACHMENT
            },
            image_sharing_mode: vk::SharingMode::EXCLUSIVE,
            pre_transform: capb.current_transform,
            composite_alpha: vk::CompositeAlphaFlagsKHR::OPAQUE,
            present_mode: prsnt,
            clipped: vk::TRUE,
            old_swapchain: self.swapchain,
            ..Default::default()
        };

        let swapchain = self.loader.create_swapchain(&swapchain_create_info, None)
            .expect("Failed to create swapchain");

        if self.dbv.vk_swapchain_output {
            println!("Swapchain Object: {:?}", swapchain);
        }

        let swapchain_images = self.loader.get_swapchain_images(swapchain)
            .expect("Failed to get swapchain images");

        let fbm = FramebufferManager::new_swapchain_bounded(
            self.dbv, self.vi.clone(), self.device.clone(), self.renderpass, self.attachments.clone(), swapchain_images,
            fmt.format, best_depth_format_support(), capb.current_extent, self.prsnt_inp
        );

        self.device.device_wait_idle().unwrap();
        self.destroy();

        self.fbm = fbm;
        self.swapchain = swapchain;
    }

    pub(crate) unsafe fn destroy(&self) {
        self.fbm.destroy();

        self.loader.destroy_swapchain(self.swapchain, None);
    }
}


pub(crate) unsafe fn query_swapchain_support(dbgv: DebugVisibility, vi: &VulkanInstance)
                                  -> (vk::SurfaceCapabilitiesKHR, Vec<vk::SurfaceFormatKHR>, Vec<vk::PresentModeKHR>) {
    let capabilities = vi.get_physical_device_surface_capabilities();
    let formats = vi.get_physical_device_surface_formats();
    let present_modes = vi.get_physical_device_surface_present_modes();
    if dbgv.vk_swapchain_output {
        println!("Supported Surface capabilities: {:?}", capabilities);
        println!("Supported Surface formats: {:?}", formats);
        println!("Supported Surface presentation modes: {:?}", present_modes);
    }

    (capabilities, formats, present_modes)
}

unsafe fn select_swapchain_support(surf_fmt: Vec<vk::SurfaceFormatKHR>, prsnt_mode: Vec<vk::PresentModeKHR>)
                                   -> (vk::SurfaceFormatKHR, vk::PresentModeKHR) {
    // generally, you want to choose the best format, presentation mode, and extent
    // but we'll just assume :)

    let format = vk::SurfaceFormatKHR {
        format: vk::Format::B8G8R8A8_SRGB,
        color_space: vk::ColorSpaceKHR::SRGB_NONLINEAR,
    };
    let prsnt_mode = vk::PresentModeKHR::MAILBOX;

    (format, prsnt_mode)
}

unsafe fn best_depth_format_support() -> vk::Format {
    // TODO

    vk::Format::D32_SFLOAT
}

pub(crate) fn best_surface_color_and_depth_format(dbv: DebugVisibility, vi: Rc<VulkanInstance>) -> (vk::Format, vk::Format) {
    unsafe {
        let (_, fmt, prsnt) = query_swapchain_support(dbv, &vi);
        let (fmt, _) = select_swapchain_support(fmt, prsnt);
        (fmt.format, best_depth_format_support())
    }
}
