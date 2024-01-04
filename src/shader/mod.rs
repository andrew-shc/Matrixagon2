pub mod triangle;
pub mod cube;
pub mod chunk;

use std::ffi::CStr;
use std::fs::File;
use std::{fs, process};
use std::rc::Rc;
use ash::{Device, vk};
use ash::util::read_spv;
use crate::component::RenderData;

pub trait Shader {
    fn renderpass(&self) -> vk::RenderPass;
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
