use std::{ffi, mem};
use std::rc::Rc;
use ash::{Device, vk};
use crate::handler::VulkanInstance;

// column major
pub type Mat4 = [[f32;4];4];
pub type Vec4 = [f32;4];

// pub(crate) fn matrix_prod_vector(a: Mat4, b: Vec4) -> Vec4 {
//     // let m = |aa: usize| a[aa][0]*b[0]+a[aa][1]*b[1]+a[aa][2]*b[2]+a[aa][3]*b[3];
//     let m = |aa: usize| a[0][aa]*b[0]+a[1][aa]*b[1]+a[2][aa]*b[2]+a[3][aa]*b[3];
//     [
//         m(0),
//         m(1),
//         m(2),
//         m(3),
//     ]
// }

pub(crate) fn matrix_prod(a: Mat4, b: Mat4) -> Mat4 {
    let m = |aa: usize, bb: usize| a[aa][0]*b[0][bb]+a[aa][1]*b[1][bb]+a[aa][2]*b[2][bb]+a[aa][3]*b[3][bb];
    // let m = |aa: usize, bb: usize| a[0][aa]*b[0][bb]+a[1][aa]*b[1][bb]+a[2][aa]*b[2][bb]+a[3][aa]*b[3][bb];
    [
        [m(0,0), m(0,1), m(0,2), m(0,3)],
        [m(1,0), m(1,1), m(1,2), m(1,3)],
        [m(2,0), m(2,1), m(2,2), m(2,3)],
        [m(3,0), m(3,1), m(3,2), m(3,3)],
    ]
}

pub(crate) fn matrix_ident() -> Mat4 {
    [
        [1.0, 0.0, 0.0, 0.0],
        [0.0, 1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ]
}


// // must enable SIMD (SSE128/AVX256/AVX512)
// pub(crate) fn merge_index_buffer(dst_ind: &mut Vec<u32>, src_ind: &mut Vec<u32>, offset: u32) {
//     for i in src_ind.iter_mut() {
//         *i += offset;
//     }
//     dst_ind.append(src_ind);
// }

#[derive(Clone)]
pub struct CmdBufContext(pub(crate) Rc<Device>, pub(crate) vk::CommandPool, pub(crate) vk::Queue);

impl CmdBufContext {
    pub(crate) unsafe fn record<C: FnMut(vk::CommandBuffer) -> ()>(&self, mut record: C) {
        let cmd_alloc_info = vk::CommandBufferAllocateInfo {
            command_pool: self.1,
            level: vk::CommandBufferLevel::PRIMARY,
            command_buffer_count: 1,
            ..Default::default()
        };
        let cmd_bufs = self.0.allocate_command_buffers(&cmd_alloc_info)
            .expect("Failed to allocate command buffers");

        let cmd_begin_info = vk::CommandBufferBeginInfo {
            flags: vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT,
            ..Default::default()
        };
        self.0.begin_command_buffer(cmd_bufs[0], &cmd_begin_info)
            .expect("Failed to begin recording command buffers");

        record(cmd_bufs[0]);

        self.0.end_command_buffer(cmd_bufs[0])
            .expect("Failed to record command buffers");

        let submit_infos = [
            vk::SubmitInfo::builder()
                .command_buffers(&cmd_bufs).build()
        ];

        self.0.queue_submit(self.2, &submit_infos, vk::Fence::null())
            .expect("Failed to submit draw command buffer to graphics queue");

        self.0.queue_wait_idle(self.2).unwrap();

        self.0.free_command_buffers(self.1, &cmd_bufs)
    }
}

// pub(crate) unsafe fn create_image<D: Copy>(
//     vi: Rc<VulkanInstance>, device: Rc<Device>, width: u32, height: u32, format: vk::Format,
//     pixels: D, unmap: bool,
// ) -> (vk::Image, vk::DeviceMemory, *mut ffi::c_void, vk::DeviceSize) {
//     let (img, img_mem) = allocate_image(vi.clone(), device.clone(), width, height, format, vk::ImageUsageFlags::COLOR_ATTACHMENT);
//
//     let img_size = (width * height * 4) as vk::DeviceSize;  // TODO: parameterize channel & depth
//
//     let data_ptr = device.map_memory(img_mem, 0, img_size, vk::MemoryMapFlags::empty()).unwrap();
//     let mut data_align = ash::util::Align::new(data_ptr, mem::align_of::<D>() as u64, img_size);
//     data_align.copy_from_slice(pixels);
//     if unmap {
//         device.unmap_memory(img_mem);
//     }
//
//     (img, img_mem, data_ptr, img_size)
// }

pub(crate) unsafe fn create_local_image(
    vi: Rc<VulkanInstance>, device: Rc<Device>, img_extent: vk::Extent3D, mip_levels: u32,
    format: vk::Format, usage: vk::ImageUsageFlags, layers: Option<u32>,
) -> (vk::Image, vk::DeviceMemory) {
    let image_info = vk::ImageCreateInfo {
        image_type: vk::ImageType::TYPE_2D,
        extent: img_extent,
        mip_levels,
        array_layers: if let Some(layers) = layers {layers} else {1},
        format,
        tiling: vk::ImageTiling::OPTIMAL,
        initial_layout: vk::ImageLayout::UNDEFINED,
        usage,
        samples: vk::SampleCountFlags::TYPE_1,
        sharing_mode: vk::SharingMode::EXCLUSIVE,
        ..Default::default()
    };

    let (img, img_mem) = allocate_image(
        vi.clone(), device.clone(), &image_info, vk::MemoryPropertyFlags::DEVICE_LOCAL
    );

    (img, img_mem)
}

pub(crate) unsafe fn allocate_image(
    vi: Rc<VulkanInstance>, device: Rc<Device>, image_info: &vk::ImageCreateInfo,
    props: vk::MemoryPropertyFlags
) -> (vk::Image, vk::DeviceMemory) {
    let img = device.create_image(&image_info, None)
        .expect("Failed to create image!");

    let mem_req = device.get_image_memory_requirements(img);

    let alloc_info = vk::MemoryAllocateInfo {
        allocation_size: mem_req.size,
        memory_type_index: find_memory_type(vi.clone(), mem_req, props),
        ..Default::default()
    };
    let img_mem = device.allocate_memory(&alloc_info, None)
        .expect("Failed to allocate image memory");

    device.bind_image_memory(img, img_mem, 0).unwrap();

    (img, img_mem)
}

pub(crate) unsafe fn create_host_buffer<D: Copy>(
    vi: Rc<VulkanInstance>, device: Rc<Device>, data: &[D], usage: vk::BufferUsageFlags, unmap: bool
) -> (vk::Buffer, vk::DeviceMemory, *mut ffi::c_void, vk::DeviceSize) {
    let (buf, buf_mem, buf_size) = allocate_buffer(
        vi.clone(), device.clone(), (mem::size_of::<D>()*data.len()) as vk::DeviceSize, usage, vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT
    );

    let data_ptr = device.map_memory(buf_mem, 0, buf_size, vk::MemoryMapFlags::empty()).unwrap();
    let mut data_align = ash::util::Align::new(data_ptr, mem::align_of::<D>() as u64, buf_size);
    data_align.copy_from_slice(data);
    if unmap {
        device.unmap_memory(buf_mem);
    }

    (buf, buf_mem, data_ptr, buf_size)
}

pub(crate) unsafe fn create_local_buffer(
    vi: Rc<VulkanInstance>, device: Rc<Device>, size: vk::DeviceSize, usage: vk::BufferUsageFlags
) -> (vk::Buffer, vk::DeviceMemory, vk::DeviceSize) {
    let (buf, buf_mem, buf_size) = allocate_buffer(
        vi.clone(), device.clone(), size, usage, vk::MemoryPropertyFlags::DEVICE_LOCAL
    );

    (buf, buf_mem, buf_size)
}

// pub(crate) unsafe fn create_local_buffer(
//     vi: Rc<VulkanInstance>, device: Rc<Device>, usage: vk::BufferUsageFlags, unmap: bool
// ) -> (vk::Buffer, vk::DeviceMemory, *mut ffi::c_void, vk::DeviceSize) {
//     let (buf, buf_mem, buf_size) = allocate_buffer(vi.clone(), device.clone(), data, usage);
//
//     (buf, buf_mem, data_ptr, buf_size)
// }

pub(crate) unsafe fn update_buffer<D: Copy>(data_ptr: *mut ffi::c_void, data: &[D], buf_size: vk::DeviceSize) {
    let mut data_align = ash::util::Align::new(data_ptr, mem::align_of::<D>() as u64, buf_size);
    data_align.copy_from_slice(data);
}

pub(crate) unsafe fn allocate_buffer(
    vi: Rc<VulkanInstance>, device: Rc<Device>, size: vk::DeviceSize, usage: vk::BufferUsageFlags,
    props: vk::MemoryPropertyFlags
) -> (vk::Buffer, vk::DeviceMemory, vk::DeviceSize) {
    let buffer_info = vk::BufferCreateInfo {
        size,
        usage,
        sharing_mode: vk::SharingMode::EXCLUSIVE,
        ..Default::default()
    };

    let buffer = device.create_buffer(&buffer_info, None).unwrap();

    let mem_req = device.get_buffer_memory_requirements(buffer);
    // println!("Memory requirements {mem_req:?}");

    let mem_alloc_info = vk::MemoryAllocateInfo {
        allocation_size: mem_req.size,
        memory_type_index: find_memory_type(vi.clone(), mem_req, props),
        ..Default::default()
    };

    let buffer_mem = device.allocate_memory(&mem_alloc_info, None).unwrap();
    device.bind_buffer_memory(buffer, buffer_mem, 0).unwrap();

    (buffer, buffer_mem, buffer_info.size)
}

pub(crate) unsafe fn find_memory_type(vi: Rc<VulkanInstance>, mem_req: vk::MemoryRequirements,
                                      props: vk::MemoryPropertyFlags) -> u32 {
    let mem_props = vi.get_physical_device_memory_properties();
    // println!("Available memory requirements {mem_req:?}");

    for i in 0..mem_props.memory_type_count {
        if (mem_req.memory_type_bits & (1 << i) != 0) &&
            ((mem_props.memory_types[i as usize].property_flags & props) == props) {
            return i;
        }
    }
    panic!("No suitable memory found with the given requirements")
}
