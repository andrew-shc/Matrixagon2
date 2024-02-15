pub mod chunk;
mod debug_ui;

use std::ffi::CStr;
use std::fs::File;
use std::{fs, mem, process};
use std::rc::Rc;
use ash::{Device, vk};
use ash::util::read_spv;
use crate::component::{RenderData};
use crate::framebuffer::FBAttachmentRef;

pub trait Shader {
    fn renderpass(&self) -> vk::RenderPass;
    fn attachments(&self) -> Vec<FBAttachmentRef>;
    unsafe fn write_descriptors(&mut self, descriptor_buffers: Vec<RenderData>);
    fn update_extent(&mut self, new_extent: vk::Extent2D);
    fn recreate_buffer(&mut self, render_data: RenderData);
    unsafe fn draw_command(&self, cmd_buf: vk::CommandBuffer, framebuffer: vk::Framebuffer);
    unsafe fn destroy(&self);
}

// C:/VulkanSDK/1.3.261.1/bin/glslc.exe src/shader/cube.frag -o src/shader/cube.frag.spv
// glslc has an option to compile shader to human readable bytecode


pub(crate) fn disabled_cba() -> vk::PipelineColorBlendAttachmentState {
    vk::PipelineColorBlendAttachmentState {
        color_write_mask: vk::ColorComponentFlags::R | vk::ColorComponentFlags::G |
            vk::ColorComponentFlags::B | vk::ColorComponentFlags::A,
        blend_enable: vk::FALSE,
        ..Default::default()
    }
}

pub(crate) fn transparent_cba() -> vk::PipelineColorBlendAttachmentState {
    vk::PipelineColorBlendAttachmentState {
        color_write_mask: vk::ColorComponentFlags::R | vk::ColorComponentFlags::G |
            vk::ColorComponentFlags::B | vk::ColorComponentFlags::A,
        blend_enable: vk::TRUE,
        src_color_blend_factor: vk::BlendFactor::SRC_ALPHA,
        dst_color_blend_factor: vk::BlendFactor::ONE_MINUS_SRC_ALPHA,
        color_blend_op: vk::BlendOp::ADD,
        src_alpha_blend_factor: vk::BlendFactor::ONE,
        dst_alpha_blend_factor: vk::BlendFactor::ZERO,
        alpha_blend_op: vk::BlendOp::ADD,
        ..Default::default()
    }
}

pub(crate) struct StandardGraphicsPipelineInfo {
    shader_stages: Vec<vk::PipelineShaderStageCreateInfo>,
    vertex_input_state: vk::PipelineVertexInputStateCreateInfo,
    back_face_culling: bool,
    depth_testing: bool,
    color_blend_attachment_state: Vec<vk::PipelineColorBlendAttachmentState>,
    // ^^^ corresponds to the color attachment for the respective subpass this pipeline is in
}

pub(crate) unsafe fn standard_graphics_pipeline(
    device: Rc<Device>,
    pipeline_infos: Vec<StandardGraphicsPipelineInfo>,
    pipeline_layout: vk::PipelineLayout,
    renderpass: vk::RenderPass
) -> Vec<vk::Pipeline> {
    let input_assembly_info = vk::PipelineInputAssemblyStateCreateInfo {
        topology: vk::PrimitiveTopology::TRIANGLE_LIST,
        primitive_restart_enable: vk::FALSE,
        ..Default::default()
    };

    let viewport_state_info = vk::PipelineViewportStateCreateInfo {
        viewport_count: 1,
        scissor_count: 1,
        ..Default::default()
    };

    let mut rasterizer_info = vk::PipelineRasterizationStateCreateInfo {
        depth_clamp_enable: vk::FALSE,
        rasterizer_discard_enable: vk::FALSE,
        polygon_mode: vk::PolygonMode::FILL,
        line_width: 1.0,
        cull_mode: vk::CullModeFlags::NONE,
        front_face: vk::FrontFace::COUNTER_CLOCKWISE,
        depth_bias_enable: vk::FALSE,
        ..Default::default()
    };

    let multisampling_info = vk::PipelineMultisampleStateCreateInfo {
        sample_shading_enable: vk::FALSE,
        rasterization_samples: vk::SampleCountFlags::TYPE_1,
        ..Default::default()
    };

    let depth_stencil = vk::PipelineDepthStencilStateCreateInfo {
        depth_test_enable: vk::TRUE,
        depth_write_enable: vk::TRUE,
        depth_compare_op: vk::CompareOp::GREATER,
        depth_bounds_test_enable: vk::FALSE,
        stencil_test_enable: vk::FALSE,
        ..Default::default()
    };

    let dynamic_states = vec![vk::DynamicState::VIEWPORT, vk::DynamicState::SCISSOR];  // vk::DynamicState::CULL_MODE
    let dynamic_state_info = vk::PipelineDynamicStateCreateInfo::builder()
        .dynamic_states(&dynamic_states)
        .build();

    let mut pipeline_create_infos = vec![];

    for info in pipeline_infos {
        let color_blend_info = vk::PipelineColorBlendStateCreateInfo::builder()
            .logic_op_enable(false)
            .attachments(&info.color_blend_attachment_state)
            .blend_constants([0.0, 0.0, 0.0, 0.0])
            .build();

        if info.back_face_culling {
            rasterizer_info.cull_mode = vk::CullModeFlags::BACK;
        }

        let pipeline_create_info = vk::GraphicsPipelineCreateInfo {
            stage_count: info.shader_stages.len() as u32,
            p_stages: info.shader_stages.as_ptr(),
            p_vertex_input_state: &info.vertex_input_state,
            p_input_assembly_state: &input_assembly_info,
            p_viewport_state: &viewport_state_info,
            p_rasterization_state: &rasterizer_info,
            p_multisample_state: &multisampling_info,
            p_depth_stencil_state: if info.depth_testing {&depth_stencil} else {&vk::PipelineDepthStencilStateCreateInfo::default()},
            p_color_blend_state: &color_blend_info,
            p_dynamic_state: &dynamic_state_info,

            layout: pipeline_layout,
            render_pass: renderpass,
            subpass: 0,
            ..Default::default()
        };

        pipeline_create_infos.push(pipeline_create_info);
    }

    device.create_graphics_pipelines(vk::PipelineCache::null(), &pipeline_create_infos, None)
        .unwrap()
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
