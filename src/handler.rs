use std::borrow::Cow;
use std::ffi::{c_char, CStr};
use std::os::raw::c_void;
use std::rc::Rc;
use ash::extensions::ext::DebugUtils;
use ash::extensions::khr::{Surface, Swapchain};
use ash::{Device, Instance, vk};
use ash_window::create_surface;
use winit::event_loop::EventLoop;
use raw_window_handle::{HasRawDisplayHandle, HasRawWindowHandle};
use winit::window::Window;
use crate::debug::DebugVisibility;
use crate::shader::Shader;
use crate::util::create_local_depth_image;


const DEVICE_EXTS: &[*const c_char] = &[
    unsafe {CStr::from_bytes_with_nul_unchecked(b"VK_KHR_swapchain\0").as_ptr()},
];
const VALIDATION_LYRS: &[*const c_char] = &[
    unsafe {CStr::from_bytes_with_nul_unchecked(b"VK_LAYER_KHRONOS_validation\0").as_ptr()},
    // unsafe {CStr::from_bytes_with_nul_unchecked(b"VK_LAYER_LUNARG_api_dump\0").as_ptr()},
];


pub(crate) struct SwapchainMTXG {
    loader: Swapchain,
    obj: vk::SwapchainKHR,
    depth_image: vk::Image,
    depth_image_mem: vk::DeviceMemory,
    images: Vec<vk::Image>,
    pub(crate) extent: vk::Extent2D,
    capabilities: vk::SurfaceCapabilitiesKHR,
    pub(crate) format: vk::SurfaceFormatKHR,
    prsnt_mode: vk::PresentModeKHR,
    image_views: Vec<vk::ImageView>,
    depth_view: vk::ImageView,
    framebuffers: Vec<vk::Framebuffer>,
}

struct SyncMTXG {
    image_available_smph: vk::Semaphore,
    render_finished_smph: vk::Semaphore,
    in_flight_fence: vk::Fence,
}

pub struct VulkanHandler {
    debug_output: DebugVisibility,
    validate: bool,
    debug_loader: Option<DebugUtils>,
    debug: Option<vk::DebugUtilsMessengerEXT>,

    pub(crate) vi: Rc<VulkanInstance>,
    pub(crate) device: Rc<Device>,
    pub(crate) gfxs_queue: vk::Queue,
    pub(crate) prsnt_queue: vk::Queue,
    pub(crate) swapchain: Option<SwapchainMTXG>,
    pub(crate) cmd_pool: vk::CommandPool,

    render_cmd_buf: vk::CommandBuffer,
    sync: SyncMTXG,

    shader: Option<Box<dyn Shader>>,
}

impl VulkanHandler {
    pub(crate) fn init(event_loop: &EventLoop<()>, window: &Window, validate: bool, debug_output: DebugVisibility) -> Self
    {
        let debug_loader;
        let debug;
        let vi;
        let device;
        let gfxs_queue;
        let prsnt_queue;
        let cmd_pool;
        let render_cmd_buf;
        let sync;
        unsafe {
            let entry = ash::Entry::linked(); // ash::Entry::load().expect("VK Entry failed to load");
            let mut surf_exts = ash_window::enumerate_required_extensions(event_loop.raw_display_handle())
                .expect("Enumerate required extensions for raw display handle failed")
                .to_vec();
            if validate {
                surf_exts.push(CStr::from_bytes_with_nul_unchecked(b"VK_EXT_debug_utils\0").as_ptr());
            }

            if debug_output.vk_setup_output {
                println!("Instance required extensions: {:?}", surf_exts);
                println!("Instance required layers: {:?}", VALIDATION_LYRS);
            }

            let app_info = vk::ApplicationInfo::builder()
                .application_name(&CStr::from_bytes_with_nul_unchecked(b"Matrixagon 2.0\0"))
                .application_version(vk::make_api_version(0, 1, 0, 0))
                .engine_name(&CStr::from_bytes_with_nul_unchecked(b"No Engine\0"))
                .engine_version(0)
                .api_version(vk::make_api_version(0, 1, 3, 0))
                .build();

            let mut debug_cinfo = vk::DebugUtilsMessengerCreateInfoEXT {
                message_severity: vk::DebugUtilsMessageSeverityFlagsEXT::VERBOSE |
                    vk::DebugUtilsMessageSeverityFlagsEXT::INFO |
                    vk::DebugUtilsMessageSeverityFlagsEXT::WARNING |
                    vk::DebugUtilsMessageSeverityFlagsEXT::ERROR,
                message_type: vk::DebugUtilsMessageTypeFlagsEXT::GENERAL |
                    vk::DebugUtilsMessageTypeFlagsEXT::PERFORMANCE |
                    vk::DebugUtilsMessageTypeFlagsEXT::VALIDATION,
                pfn_user_callback: Some(vulkan_validation_debug_callback),
                ..Default::default()
            };

            if debug_output.vk_setup_output {
                let exts = entry.enumerate_instance_extension_properties(None).unwrap();
                println!("Instance supported extensions: {exts:?}");
            }

            let inst_create_info = if validate {
                vk::InstanceCreateInfo::builder()
                    .application_info(&app_info)
                    .enabled_extension_names(&surf_exts)
                    .enabled_layer_names(&VALIDATION_LYRS)
                    .push_next(&mut debug_cinfo)
                    .build()
            } else {
                vk::InstanceCreateInfo::builder()
                    .application_info(&app_info)
                    .enabled_extension_names(&surf_exts)
                    .build()
            };

            let inst = entry.create_instance(&inst_create_info, None)
                .expect("Failed to create instance from create info");

            debug_loader = if validate {Some(DebugUtils::new(&entry, &inst))} else {None};
            debug = if validate {
                Some(
                    debug_loader.clone().unwrap().create_debug_utils_messenger(&debug_cinfo, None)
                        .expect("Failed to create debug messenger")
                )
            } else {None};

            let surf = create_surface(
                &entry,
                &inst,
                window.raw_display_handle(),
                window.raw_window_handle(),
                None
            )
                .expect("Surface creation failed");

            let surf_loader = Surface::new(&entry, &inst);
            if debug_output.vk_setup_output {
                println!("Surface Object: {:?}", surf);
            }

            vi = Rc::new(VulkanInstance::new(debug_output, inst, surf, surf_loader));

            // CREATING GRAPHICS AND PRESENTATION QUEUES

            let queue_fam_ind = find_queue_families(debug_output, &vi).unwrap();
            let gfxs_queue_create_info = vk::DeviceQueueCreateInfo {
                queue_family_index: queue_fam_ind,
                queue_count: 1,
                p_queue_priorities: &1.0,
                ..Default::default()
            };

            // since our presentation queue will be the same as the graphics queue
            // let prsnt_queue_create_info = vk::DeviceQueueCreateInfo {
            //     queue_family_index: queue_fam_ind,
            //     queue_count: 1,
            //     p_queue_priorities: &1.0,
            //     ..Default::default()
            // };

            // LOGICAL DEVICE CREATION

            let phys_devc_feats = vk::PhysicalDeviceFeatures {
                sampler_anisotropy: vk::TRUE,
                ..Default::default()
            };

            let device_create_info = vk::DeviceCreateInfo::builder()
                .queue_create_infos(&[gfxs_queue_create_info, /*prsnt_queue_create_info*/ ])
                .enabled_features(&phys_devc_feats)
                .enabled_extension_names(&DEVICE_EXTS)
                .build();

            device = Rc::new(vi.inst.create_device(vi.phys_devc, &device_create_info, None)
                .expect("Failed to create logical device"));

            if debug_output.vk_setup_output {
                println!("(Logical) Device Object: {:?}", device.handle());
            }
            gfxs_queue = device.clone().get_device_queue(queue_fam_ind,0);
            prsnt_queue = device.clone().get_device_queue(queue_fam_ind,0);

            // COMMAND BUFFER

            let ind = find_queue_families(debug_output, &vi).unwrap();
            let cmd_pool_info = vk::CommandPoolCreateInfo {
                flags: vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER,
                queue_family_index: ind,
                ..Default::default()
            };
            cmd_pool = device.create_command_pool(&cmd_pool_info, None)
                .expect("Failed to create command pool");

            let cmd_alloc_info = vk::CommandBufferAllocateInfo {
                command_pool: cmd_pool,
                level: vk::CommandBufferLevel::PRIMARY,
                command_buffer_count: 1,
                ..Default::default()
            };
            render_cmd_buf = device.allocate_command_buffers(&cmd_alloc_info)
                .expect("Failed to allocate command buffers");

            // RENDER FRAME SYNCS

            let semaphore_info = vk::SemaphoreCreateInfo::default();
            let fence_info = vk::FenceCreateInfo::builder()
                .flags(vk::FenceCreateFlags::SIGNALED).build();

            let image_available_smph = device.create_semaphore(&semaphore_info, None)
                .expect("Failed to create image available semaphore");
            let render_finished_smph = device.create_semaphore(&semaphore_info, None)
                .expect("Failed to create render finished semaphore");
            let in_flight_fence = device.create_fence(&fence_info, None)
                .expect("Failed to create in flight fence");

            sync = SyncMTXG {
                image_available_smph,
                render_finished_smph,
                in_flight_fence
            }
        }

        VulkanHandler {
            debug_output, validate, debug_loader, debug,
            vi: vi.clone(), device, gfxs_queue, prsnt_queue,
            swapchain: None, cmd_pool,
            render_cmd_buf: render_cmd_buf[0], sync, shader: None,
        }
    }

    pub(crate) fn best_surface_color_and_depth_format(&self) -> (vk::Format, vk::Format) {
        unsafe {
            let (_, fmt, prsnt) = query_swapchain_support(self.debug_output, &self.vi);
            let (fmt, _) = select_swapchain_support(fmt, prsnt);
            (fmt.format, best_depth_format_support())
        }
    }

    pub(crate) fn load_shader(&mut self, shader: impl Shader + 'static) {
        self.shader = Some(Box::new(shader) as Box<dyn Shader>);
    }

    pub(crate) fn obtain_shader_mut_ref(&mut self) -> &mut Box<dyn Shader> {
        self.shader.as_mut().unwrap()
    }

    pub(crate) fn create_swapchain(&mut self, initial_extent: vk::Extent2D) {
        unsafe {
            let (capb, fmt, prsnt) = query_swapchain_support(self.debug_output, &self.vi);
            let (fmt, prsnt) = select_swapchain_support(fmt, prsnt);
            let swapchain_loader = Swapchain::new(&self.vi.inst, &*self.device.clone());

            // TODO: find a way to determine which renderpass to use
            let renderpass = self.shader.as_ref().unwrap().renderpass();

            self.swapchain = Some(
                create_swapchain(
                    self.debug_output, self.vi.clone(), self.device.clone(), swapchain_loader, None,
                    renderpass, initial_extent, capb, fmt, prsnt, best_depth_format_support()
                )
            );
        }
    }

    pub(crate) unsafe fn draw_frame(&mut self) {
        let swapchain = self.swapchain.as_ref()
            .expect("Attempted to draw frame when swapchain has not initialized yet!");

        self.device.wait_for_fences(&[self.sync.in_flight_fence], true, u64::MAX).unwrap();

        let acquisition = swapchain.loader.acquire_next_image(swapchain.obj, u64::MAX, self.sync.image_available_smph, vk::Fence::null());
        match acquisition {
            // swapchain suboptimal
            Ok((_, true)) => {
                self.recreate_swapchain();
                return;
            }
            Err(vk::Result::ERROR_OUT_OF_DATE_KHR) => {
                self.recreate_swapchain();
                return;
            }
            Err(e) => {
                panic!("{}", e);
            }
            _ => {}
        }

        self.device.reset_fences(&[self.sync.in_flight_fence]).unwrap();
        let (img_ind, _) = acquisition.unwrap();

        self.device.reset_command_buffer(self.render_cmd_buf, vk::CommandBufferResetFlags::empty()).unwrap();

        // COMMAND RECORDING

        let cmd_begin_info = vk::CommandBufferBeginInfo {
            ..Default::default()
        };
        self.device.begin_command_buffer(self.render_cmd_buf, &cmd_begin_info)
            .expect("Failed to begin recording command buffers");

        self.shader.as_ref().unwrap()
            .draw_command(self.render_cmd_buf, swapchain.framebuffers[img_ind as usize]);

        self.device.end_command_buffer(self.render_cmd_buf)
            .expect("Failed to record command buffers");

        let submit_info = vk::SubmitInfo::builder()
            .wait_semaphores(&[self.sync.image_available_smph])
            .wait_dst_stage_mask(&[vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT])
            .command_buffers(&[self.render_cmd_buf])
            .signal_semaphores(&[self.sync.render_finished_smph]).build();

        self.device.queue_submit(self.gfxs_queue, &[submit_info], self.sync.in_flight_fence)
            .expect("Failed to submit draw command buffer to graphics queue");

        let prsnt_info = vk::PresentInfoKHR::builder()
            .wait_semaphores(&[self.sync.render_finished_smph])
            .swapchains(&[swapchain.obj])
            .image_indices(&[img_ind]).build();
        let swapchain_result = swapchain.loader.queue_present(self.prsnt_queue, &prsnt_info);
        match swapchain_result {
            // swapchain suboptimal
            Ok(true) => {
                self.recreate_swapchain();
            }
            Err(vk::Result::ERROR_OUT_OF_DATE_KHR) => {
                self.recreate_swapchain();
            }
            Err(e) => {
                panic!("{}", e);
            }
            _ => {}
        }
    }

    pub(crate) unsafe fn recreate_swapchain(&mut self) {
        let (capb, fmt, prsnt) = query_swapchain_support(self.debug_output, &self.vi);
        let (fmt, prsnt) = select_swapchain_support(fmt, prsnt);
        let old_swapchain = self.swapchain.as_ref().expect("Expected swapchain when recreating");
        // TODO: find a way to determine renderpass from multiple shaders
        let new_swapchain = create_swapchain(
            self.debug_output, self.vi.clone(), self.device.clone(),
            old_swapchain.loader.clone(),
            Some(old_swapchain.obj), self.shader.as_ref().unwrap().renderpass(), capb.current_extent,
            capb, fmt, prsnt, best_depth_format_support()
        );
        self.shader.as_mut().unwrap()
            .update_extent(capb.current_extent);

        self.device.device_wait_idle().unwrap();
        self.destroy_swapchain();
        self.swapchain = Some(new_swapchain);

    }

    pub(crate) unsafe fn destroy(&self) {
        self.destroy_swapchain();

        self.device.destroy_semaphore(self.sync.image_available_smph, None);
        self.device.destroy_semaphore(self.sync.render_finished_smph, None);
        self.device.destroy_fence(self.sync.in_flight_fence, None);

        self.device.destroy_command_pool(self.cmd_pool, None);

        if let Some(shader) = &self.shader {
            shader.destroy();
        }

        // --- device level ---
        self.device.destroy_device(None);

        if self.validate {
            self.debug_loader.as_ref().unwrap().destroy_debug_utils_messenger(self.debug.unwrap(), None);
        }
        self.vi.surf_loader.destroy_surface(self.vi.surf, None);
        self.vi.inst.destroy_instance(None);
    }

    pub(crate) unsafe fn destroy_swapchain(&self) {
        if let Some(swapchain) = &self.swapchain {
            self.device.destroy_image_view(swapchain.depth_view, None);
            self.device.destroy_image(swapchain.depth_image, None);
            self.device.free_memory(swapchain.depth_image_mem, None);

            for &framebuffer in swapchain.framebuffers.iter() {
                self.device.destroy_framebuffer(framebuffer, None);
            }

            for &view in swapchain.image_views.iter() {
                self.device.destroy_image_view(view, None);
            }

            swapchain.loader.destroy_swapchain(swapchain.obj, None);
        }
    }
}

unsafe fn find_queue_families(dbgv: DebugVisibility, vi: &VulkanInstance) -> Option<u32> {
    let queue_fams = vi.get_physical_device_queue_family_properties();

    let mut ind = 0;
    for queue_fam in queue_fams {
        if dbgv.vk_setup_output {
            println!("Queue family properties: {:?}", queue_fam);
        }

        let supported = vi.get_physical_device_surface_support(ind);

        if queue_fam.queue_flags.contains(vk::QueueFlags::GRAPHICS) && supported {
            return Some(ind);
        }

        ind += 1;
    }

    None
}

unsafe fn query_swapchain_support(dbgv: DebugVisibility, vi: &VulkanInstance)
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


// unsafe fn choose_swap_surf_format() {
//
// }
//
// unsafe fn choose_swap_prsnt_mode() {
//
// }
//
// unsafe fn choose_swap_extent() {
//
// }

unsafe fn create_swapchain(
    dbv: DebugVisibility, vi: Rc<VulkanInstance>, device: Rc<Device>, swapchain_loader: Swapchain,
    old_swapchain: Option<vk::SwapchainKHR>, renderpass: vk::RenderPass, swap_extent: vk::Extent2D,
    surf_capabilities: vk::SurfaceCapabilitiesKHR, surf_format: vk::SurfaceFormatKHR, prsnt_mode: vk::PresentModeKHR,
    depth_format: vk::Format,
) -> SwapchainMTXG {
    // create swapchain itself and related objects (imageview and framebuffer)

    // SWAPCHAIN
    let image_count = surf_capabilities.min_image_count+1;

    // assuming graphics and presentation queue families are the same ind.
    let swapchain_create_info = vk::SwapchainCreateInfoKHR {
        surface: vi.surf,
        min_image_count: image_count,
        image_format: surf_format.format,
        image_color_space: surf_format.color_space,
        image_extent: swap_extent,
        image_array_layers: 1,
        image_usage: vk::ImageUsageFlags::COLOR_ATTACHMENT,
        image_sharing_mode: vk::SharingMode::EXCLUSIVE,
        pre_transform: surf_capabilities.current_transform,
        composite_alpha: vk::CompositeAlphaFlagsKHR::OPAQUE,
        present_mode: prsnt_mode,
        clipped: vk::TRUE,
        old_swapchain: if let Some(old_swapchain) = old_swapchain {old_swapchain} else {vk::SwapchainKHR::null()},
        ..Default::default()
    };

    let swapchain = swapchain_loader.create_swapchain(&swapchain_create_info, None)
        .expect("Failed to create swapchain");

    if dbv.vk_swapchain_output {
        println!("Swapchain Object: {:?}", swapchain);
    }

    let swapchain_images = swapchain_loader.get_swapchain_images(swapchain)
        .expect("Failed to get swapchain images");

    let mut image_views = Vec::new();
    for swapchain_image in &swapchain_images {
        let scv_create_info = vk::ImageViewCreateInfo {
            image: *swapchain_image,
            view_type: vk::ImageViewType::TYPE_2D,
            format: surf_format.format,
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

        let view = device.create_image_view(&scv_create_info, None)
            .expect("Failed to create image view");
        image_views.push(view);
    }

    let (depth_img, depth_img_mem) = create_local_depth_image(
        vi.clone(), device.clone(), vk::Extent3D {width: swap_extent.width, height: swap_extent.height, depth: 1}, depth_format
    );
    let depth_imgv_create_info = vk::ImageViewCreateInfo {
        image: depth_img,
        view_type: vk::ImageViewType::TYPE_2D,
        format: depth_format,
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

    // FRAMEBUFFER

    let mut framebuffers = Vec::new();
    for swapchain_view in &image_views {
        let framebuffer_info = vk::FramebufferCreateInfo::builder()
            .render_pass(renderpass)
            .attachments(&[*swapchain_view, depth_view])
            .width(swap_extent.width)
            .height(swap_extent.height)
            .layers(1)
            .build();

        let fb = device.create_framebuffer(&framebuffer_info, None)
            .expect("Failed to create framebuffer");
        framebuffers.push(fb);
    }

    SwapchainMTXG {
        loader: swapchain_loader,
        obj: swapchain,
        depth_image_mem: depth_img_mem,
        depth_image: depth_img,
        images: swapchain_images,
        extent: swap_extent,
        capabilities: surf_capabilities,
        format: surf_format,
        prsnt_mode,
        depth_view,
        image_views,
        framebuffers,
    }
}

unsafe extern "system" fn vulkan_validation_debug_callback(
    msg_severity: vk::DebugUtilsMessageSeverityFlagsEXT,
    msg_type: vk::DebugUtilsMessageTypeFlagsEXT,
    p_callback_data: *const vk::DebugUtilsMessengerCallbackDataEXT,
    _p_user_data: *mut c_void,
) -> vk::Bool32 {
    let callback_data = *p_callback_data;
    let message_id_number: i32 = callback_data.message_id_number as i32;

    let message_id_name = if callback_data.p_message_id_name.is_null() {
        Cow::from("")
    } else {
        CStr::from_ptr(callback_data.p_message_id_name).to_string_lossy()
    };

    let message = if callback_data.p_message.is_null() {
        Cow::from("")
    } else {
        CStr::from_ptr(callback_data.p_message).to_string_lossy()
    };

    println!(
        "[{:?}] {:?}: {} ({}):\n{}",
        msg_severity,
        msg_type,
        message_id_name,
        &message_id_number.to_string(),
        message,
    );

    vk::FALSE
}


pub(crate) struct VulkanInstance {
    pub(crate) inst: Instance,
    pub(crate) surf: vk::SurfaceKHR,
    pub(crate) surf_loader: Surface,
    pub(crate) phys_devc: vk::PhysicalDevice,
}

impl VulkanInstance {
    pub(crate) unsafe fn new(dbgv: DebugVisibility, inst: Instance, surf: vk::SurfaceKHR, surf_loader: Surface) -> Self {
        let mut s = Self {
            inst,
            surf,
            surf_loader,
            phys_devc: vk::PhysicalDevice::null(),
        };
        s.find_physical_device(dbgv);
        s
    }

    unsafe fn find_physical_device(&mut self, dbgv: DebugVisibility) {
        let phys_devcs = self.inst.enumerate_physical_devices()
            .expect("Failed to enumerate physical device");

        if dbgv.vk_setup_output {
            println!("Available physical devices: {:?}", phys_devcs);
        }

        // let mut phys_devc_o = None;
        for phys_devc_i in phys_devcs {
            self.phys_devc = phys_devc_i;
            if self.is_device_suitable(dbgv, &self.inst, phys_devc_i) {
                // phys_devc_o = Some(phys_devc_i);
                if dbgv.vk_setup_output {
                    println!("Suitable physical device found!");
                }
                break;
            }
        }
        // self.phys_devc = phys_devc_o.expect("No suitable physical device found");

        if dbgv.vk_setup_output {
            println!("Selected physical device: {:?}", self.phys_devc);
        }
    }

    unsafe fn is_device_suitable(&self, dbgv: DebugVisibility, inst: &Instance, device: vk::PhysicalDevice) -> bool {
        let props = inst.get_physical_device_properties(device);
        let feats = inst.get_physical_device_features(device);

        let device_ext_props = inst.enumerate_device_extension_properties(device)
            .expect("Failed to enumerate device extension props");

        if dbgv.vk_setup_output {
            println!("Physical device: {:?}", device);
            println!("\tProperties: {:?}", props);
            println!("\tFeatures: {:?}", feats);
        }

        // FIND IF EXTENSIONS SUPPORTED
        for &device_ext in DEVICE_EXTS {
            let mut has = false;
            for device_ext_prop in &device_ext_props {
                if dbgv.vk_setup_output {
                    println!("\tSupported device extension: {:?}", device_ext_prop);
                }
                if CStr::from_ptr(device_ext) == CStr::from_ptr(device_ext_prop.extension_name.as_ptr()){
                    if dbgv.vk_setup_output {
                        println!("\t^^^ Required device extension found ^^^");
                    }
                    has = true;
                    break;
                }
            }
            if !has {
                return false;
            }
        }

        let (_, formats, present_modes) = query_swapchain_support(dbgv, &self);

        props.device_type == vk::PhysicalDeviceType::DISCRETE_GPU &&
            find_queue_families(dbgv, &self).is_some() &&
            !formats.is_empty() &&
            !present_modes.is_empty() &&
            feats.sampler_anisotropy != 0
    }

    pub(crate) unsafe fn get_physical_device_surface_present_modes(&self) -> Vec<vk::PresentModeKHR> {
        self.surf_loader.get_physical_device_surface_present_modes(self.phys_devc, self.surf)
            .unwrap()
    }

    pub(crate) unsafe fn get_physical_device_surface_formats(&self) -> Vec<vk::SurfaceFormatKHR> {
        self.surf_loader.get_physical_device_surface_formats(self.phys_devc, self.surf)
            .unwrap()
    }

    pub(crate) unsafe fn get_physical_device_surface_capabilities(&self) -> vk::SurfaceCapabilitiesKHR {
        self.surf_loader.get_physical_device_surface_capabilities(self.phys_devc, self.surf)
            .unwrap()
    }

    pub(crate) unsafe fn get_physical_device_surface_support(&self, queue_family_index: u32) -> bool {
        self.surf_loader.get_physical_device_surface_support(self.phys_devc, queue_family_index, self.surf)
            .unwrap()
    }

    pub(crate) unsafe fn get_physical_device_properties(&self) -> vk::PhysicalDeviceProperties {
        self.inst.get_physical_device_properties(self.phys_devc)
    }

    pub(crate) unsafe fn get_physical_device_features(&self) -> vk::PhysicalDeviceFeatures {
        self.inst.get_physical_device_features(self.phys_devc)
    }

    pub(crate) unsafe fn enumerate_device_extension_properties(&self) -> Vec<vk::ExtensionProperties> {
        self.inst.enumerate_device_extension_properties(self.phys_devc)
            .unwrap()
    }

    pub(crate) unsafe fn get_physical_device_queue_family_properties(&self) -> Vec<vk::QueueFamilyProperties> {
        self.inst.get_physical_device_queue_family_properties(self.phys_devc)
    }

    pub(crate) unsafe fn get_physical_device_memory_properties(&self) -> vk::PhysicalDeviceMemoryProperties {
        self.inst.get_physical_device_memory_properties(self.phys_devc)
    }
}
