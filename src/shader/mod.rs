pub mod chunk;
mod debug_ui;

use std::ffi::CStr;
use std::fs::File;
use std::{fs, mem, process};
use std::rc::Rc;
use ash::{Device, vk};
use ash::util::read_spv;
use crate::component::{RenderData, RenderDataPurpose};
use crate::framebuffer::AttachmentRef;
use crate::shader::chunk::ChunkVertex;

pub trait Shader {
    fn renderpass(&self) -> vk::RenderPass;
    fn attachments(&self) -> Vec<AttachmentRef>;
    unsafe fn write_descriptors(&mut self, descriptor_buffers: Vec<RenderData>);
    fn update_extent(&mut self, new_extent: vk::Extent2D);
    fn recreate_buffer(&mut self, render_data: RenderData);
    unsafe fn draw_command(&self, cmd_buf: vk::CommandBuffer, framebuffer: vk::Framebuffer);
    unsafe fn destroy(&self);
}

// C:/VulkanSDK/1.3.261.1/bin/glslc.exe src/shader/cube.frag -o src/shader/cube.frag.spv
// glslc has an option to compile shader to human readable bytecode


// https://github.com/ash-rs/ash/blob/master/ash-examples/src/lib.rs
#[macro_export]
macro_rules! offset_of {  // Simple offset_of macro akin to C++ offsetof
    ($base:path, $field:ident) => {{
        #[allow(unused_unsafe)]
        unsafe {
            let b: $base = mem::zeroed();
            std::ptr::addr_of!(b.$field) as isize - std::ptr::addr_of!(b) as isize
        }
    }};
}


pub(crate) struct DescriptorManager {
    device: Rc<Device>,

    pipeline_layout: vk::PipelineLayout,
    descriptor_layout: Vec<Vec<(vk::DescriptorType, vk::ShaderStageFlags)>>,
    descriptor_set_layout: Vec<vk::DescriptorSetLayout>,
    descriptor_pool: vk::DescriptorPool,
    descriptor_set: Vec<vk::DescriptorSet>,
}

impl DescriptorManager {
    pub(crate) unsafe fn new(device: Rc<Device>, descriptors: Vec<Vec<(vk::DescriptorType, vk::ShaderStageFlags)>>) -> Self {
        // assumes descriptor count of 1 always

        let mut set_layouts = Vec::new();
        let mut pool_sizes = Vec::new();
        for set in &descriptors {
            let mut bindings = Vec::new();
            for (binding_ind, (binding_type, binding_stage)) in set.into_iter().enumerate() {
                let set_layout_binding = vk::DescriptorSetLayoutBinding {
                    binding: binding_ind as u32,
                    descriptor_count: 1,  // arrays of the same binding
                    descriptor_type: *binding_type,
                    p_immutable_samplers: std::ptr::null(),
                    stage_flags: *binding_stage,
                };
                bindings.push(set_layout_binding);

                let pool_size = vk::DescriptorPoolSize {
                    ty: *binding_type,
                    descriptor_count: 1,
                };
                pool_sizes.push(pool_size);
            }

            let descriptor_set_layout_info = vk::DescriptorSetLayoutCreateInfo::builder()
                .bindings(&bindings)
                .build();
            let descriptor_set_layout = device.create_descriptor_set_layout(&descriptor_set_layout_info, None)
                .expect("Failed to create descriptor set layout");
            set_layouts.push(descriptor_set_layout);
        }
        let pipeline_layout_info = vk::PipelineLayoutCreateInfo::builder()
            .set_layouts(&set_layouts)
            .build();
        let pipeline_layout = device.create_pipeline_layout(&pipeline_layout_info, None).unwrap();

        let descriptor_pool_info = vk::DescriptorPoolCreateInfo::builder()
            .pool_sizes(&pool_sizes)
            .max_sets(set_layouts.len() as u32)
            .build();
        let descriptor_pool = device.create_descriptor_pool(&descriptor_pool_info, None)
            .expect("Failed to create descriptor pool");

        let descriptor_set_alloc = vk::DescriptorSetAllocateInfo::builder()
            .descriptor_pool(descriptor_pool)
            .set_layouts(&set_layouts)
            .build();

        let descriptor_set = device.allocate_descriptor_sets(&descriptor_set_alloc)
            .expect("Failed to allocate descriptor sets");

        Self {
            device,
            pipeline_layout,
            descriptor_layout: descriptors,
            descriptor_set_layout: set_layouts,
            descriptor_pool,
            descriptor_set,
        }
    }

    pub(crate) unsafe fn write_buffer(&mut self, set: u32, binding: u32, buf: Vec<vk::DescriptorBufferInfo>) {
        self.device.update_descriptor_sets(&[
            vk::WriteDescriptorSet::builder()
                .dst_set(self.descriptor_set[set as usize])
                .dst_binding(binding)
                .dst_array_element(0)
                .descriptor_type(self.descriptor_layout[set as usize][binding as usize].0)
                .buffer_info(&buf)
                .build()
        ], &[]);
    }

    pub(crate) unsafe fn write_image(&mut self, set: u32, binding: u32, img: Vec<vk::DescriptorImageInfo>) {
        self.device.update_descriptor_sets(&[
            vk::WriteDescriptorSet::builder()
                .dst_set(self.descriptor_set[set as usize])
                .dst_binding(binding)
                .dst_array_element(0)
                .descriptor_type(self.descriptor_layout[set as usize][binding as usize].0)
                .image_info(&img)
                .build()
        ], &[]);
    }

    pub(crate) unsafe fn pipeline_layout(&self) -> vk::PipelineLayout {self.pipeline_layout}

    pub(crate) unsafe fn descriptor_sets(&self, indices: &[usize]) -> Vec<vk::DescriptorSet> {
        let mut result = Vec::new();
        for ind in indices {
            result.push(self.descriptor_set[*ind]);
        }
        result
    }

    pub(crate) unsafe fn destroy(&self) {
        self.device.destroy_descriptor_pool(self.descriptor_pool, None);
        for set_layout in &self.descriptor_set_layout {
            self.device.destroy_descriptor_set_layout(*set_layout, None);
        }
        self.device.destroy_pipeline_layout(self.pipeline_layout, None);
    }
}

#[macro_export]
macro_rules! get_vertex_inp {  // Simple offset_of macro akin to C++ offsetof
    ($base:path; $(($fmt:expr, $field:ident)),*) => {{
        let binding_descrp = vk::VertexInputBindingDescription {
            binding: 0,
            stride: mem::size_of::<$base>() as u32,
            input_rate: vk::VertexInputRate::VERTEX,
        };
        let locations = vec![
            $(
                (
                    $fmt,
                    unsafe {  // offset of
                        let b: $base = mem::zeroed();
                        std::ptr::addr_of!(b.$field) as isize - std::ptr::addr_of!(b) as isize
                    } as u32
                )
            ),*
        ];
        let mut attr_descrps = Vec::new();
        for (loc, (format, offset)) in locations.into_iter().enumerate() {
            let attr_descrp = vk::VertexInputAttributeDescription {
                binding: 0,
                location: loc as u32,
                format,
                offset,
            };
            attr_descrps.push(attr_descrp)
        }

        vk::PipelineVertexInputStateCreateInfo::builder()
            .vertex_binding_descriptions(&[binding_descrp])
            .vertex_attribute_descriptions(&attr_descrps)
            .build()
    }};
}


pub(crate) fn get_vertex_inp<T>(locations: Vec<(vk::Format, u32)>) -> vk::PipelineVertexInputStateCreateInfo {
    // assumes a single binding at 0

    let binding_descrp = vk::VertexInputBindingDescription {
        binding: 0,
        stride: mem::size_of::<T>() as u32,
        input_rate: vk::VertexInputRate::VERTEX,
    };

    let mut attr_descrps = Vec::new();
    for (loc, (format, offset)) in locations.into_iter().enumerate() {
        let attr_descrp = vk::VertexInputAttributeDescription {
            binding: 0,
            location: loc as u32,
            format,
            offset,
        };
        attr_descrps.push(attr_descrp)
    }

    vk::PipelineVertexInputStateCreateInfo::builder()
        .vertex_binding_descriptions(&[binding_descrp])
        .vertex_attribute_descriptions(&attr_descrps)
        .build()
}

pub(crate) unsafe fn gen_shader_modules_info(device: Rc<Device>, shaders: Vec<(&str, vk::ShaderStageFlags)>)
    -> (Vec<vk::PipelineShaderStageCreateInfo>, Vec<vk::ShaderModule>) {
    let mut pipeline = vec![];
    let mut modules = vec![];

    for (shader_fpath, shader_stage) in shaders {
        let status = process::Command::new("C:/VulkanSDK/1.3.261.1/bin/glslc.exe")
            .arg(shader_fpath)
            .arg("-o")
            .arg(format!("{shader_fpath}.spv"))
            .status()
            .expect(&*format!("Failed to compile shader {shader_fpath}"));
        println!("Compiled shader <{shader_fpath}> with status of {status}");

        let shader_module = create_shader_module(device.clone(), &*format!("{shader_fpath}.spv"));

        let shader_info = vk::PipelineShaderStageCreateInfo::builder()
            .stage(shader_stage)
            .module(shader_module)
            .name(CStr::from_bytes_with_nul_unchecked(b"main\0"))
            .build();

        pipeline.push(shader_info);
        modules.push(shader_module);

        fs::remove_file(format!("{shader_fpath}.spv"))
            .expect(&*format!("Failed to delete the temp file for the compiled shader {shader_fpath}.spv"));
    }

    (pipeline, modules)
}

pub(crate) unsafe fn destroy_shader_modules(device: Rc<Device>, shader_modules: Vec<vk::ShaderModule>) {
    for shader_module in shader_modules {
        device.destroy_shader_module(shader_module, None);
    }
}

unsafe fn create_shader_module(device: Rc<Device>, fpath: &str) -> vk::ShaderModule {
    let mut fobj = File::open(fpath).unwrap();
    let code = read_spv(&mut fobj).unwrap();

    let create_info = vk::ShaderModuleCreateInfo {
        // code size are in bytes, but code data is aligned to u32 (4 bytes)
        code_size: code.len() * std::mem::size_of::<u32>(),
        p_code: code.as_ptr(),
        ..Default::default()
    };

    device.create_shader_module(&create_info, None).unwrap()
}
