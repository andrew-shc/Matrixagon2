use std::{ffi, mem};
use std::rc::Rc;
use ash::{Device, vk};
use uom::num_traits::FloatConst;
use crate::component::{Component, RenderData, RenderDataPurpose};
use crate::handler::VulkanInstance;
use crate::util::{CmdBufContext, create_host_buffer, update_buffer};
use crate::world::WorldEvent;

pub(crate) struct TickSync {
    device: Rc<Device>,

    tick: bool,
    time: f32,
    increment: f32,

    ubo_buf: vk::Buffer,
    ubo_mem: vk::DeviceMemory,
    ubo_ptr: *mut ffi::c_void,
    ubo_size: vk::DeviceSize,
}

impl TickSync {
    pub(crate) fn new(vi: Rc<VulkanInstance>, device: Rc<Device>, speed: f32) -> Self {
        let time = 0.0;

        let (ubo_buf, ubo_mem, ubo_ptr, ubo_size) = unsafe {
            create_host_buffer(vi.clone(), device.clone(), &[time], vk::BufferUsageFlags::UNIFORM_BUFFER, false)
        };

        Self {
            device,
            tick: false,
            time,
            increment: speed,
            ubo_buf,
            ubo_mem,
            ubo_ptr,
            ubo_size
        }
    }

    fn update_animation_time(&mut self, delta: f32) {
        if(self.time >= 2.0*f32::PI()) {  // makes looping animation using trig easier
            self.time = 0.0;
        } else {
            self.time += self.increment*delta;
        }

        unsafe {
            update_buffer(self.ubo_ptr, &[self.time], self.ubo_size);
        }
    }
}

impl Component for TickSync {
    fn render(&self) -> Vec<RenderData> {
        vec![]
    }

    fn respond_event(&mut self, event: WorldEvent) -> Vec<WorldEvent> {
        match event {
            WorldEvent::DeltaTime(dur) => {
                self.tick = false;

                self.update_animation_time(dur.as_secs_f32());

                vec![
                    WorldEvent::Tick,
                ]
            }
            _ => {vec![]}
        }
    }

    fn update(&mut self) {
        self.tick = true;
    }

    unsafe fn load_descriptors(&mut self, _: CmdBufContext) -> Vec<RenderData> {
        vec![
            RenderData::InitialDescriptorBuffer(
                vec![vk::DescriptorBufferInfo {
                    buffer: self.ubo_buf,
                    offset: 0 as vk::DeviceSize,
                    range: mem::size_of::<f32>() as vk::DeviceSize,
                }],
                RenderDataPurpose::Time
            )
        ]
    }

    unsafe fn destroy(&mut self) {
        self.device.destroy_buffer(self.ubo_buf, None);
        self.device.free_memory(self.ubo_mem, None);
    }
}
