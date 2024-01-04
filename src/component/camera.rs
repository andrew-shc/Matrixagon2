use std::rc::Rc;
use ash::{Device, vk};
use winit::event::VirtualKeyCode;
use crate::component::{Component, ComponentEventResponse, RenderData, RenderDataPurpose};
use crate::handler::VulkanInstance;
use crate::util::{Mat4, matrix_prod};
use crate::world::{WorldEvent, WorldState};
use std::{ffi, mem};
use measurements::{Angle, Length};
use crate::util::{create_host_buffer, matrix_ident, update_buffer};


#[derive(Copy, Clone)]
pub(crate) struct Translation {
    pub(crate) x: Length, pub(crate) y: Length, pub(crate) z: Length
}
impl Default for Translation {
    fn default() -> Self {
        Translation {x: Length::from_meters(0.0), y: Length::from_meters(0.0), z: Length::from_meters(0.0)}
    }
}

#[derive(Copy, Clone)]
pub(crate) struct Rotation {
    x: Angle, y: Angle, z: Angle
}
impl Default for Rotation {
    fn default() -> Self {
        Rotation {x: Angle::from_radians(0.0), y: Angle::from_radians(0.0), z: Angle::from_radians(0.0)}
    }
}

pub(crate) struct CameraComponent {
    descriptor: CameraDescriptor,

    // high-level parameters
    trans_speed: f64,
    rot_speed: f64,
    // camera state
    t: Translation,  // translations are in blocks
    r: Rotation,
    translations: Vec<VirtualKeyCode>,
    rotated: bool,
    // view
    rot_x: Mat4,
    rot_y: Mat4,
    rot_z: Mat4,
    trans: Mat4,
    // projection
    far: f32,
    near: f32,
    aspect_ratio: f32,
    fov: f32,  // RADIANS
}

impl CameraComponent {
    pub(crate) fn new(vi: Rc<VulkanInstance>, device: Rc<Device>,
                      aspect_ratio: f32, fov: f32, trans_speed: f64, rot_speed: f64) -> CameraComponent {
        let init_rot = (180.0f32).to_radians();
        CameraComponent {
            descriptor: unsafe { CameraDescriptor::new(vi.clone(), device.clone()) },
            trans_speed, rot_speed, t: Translation::default(), r: Rotation::default(),
            translations: Vec::new(), rotated: false,
            rot_x: Self::rot_x_mat(0.0),
            rot_y: Self::rot_y_mat(0.0),
            rot_z: Self::rot_z_mat(0.0),
            trans: Self::trans_mat(Translation::default()),
            far: 100000.0, near: 0.1, aspect_ratio, fov: fov.to_radians(),
        }
    }

    pub(crate) fn rotate(&mut self, dr: Rotation) {
        if dr.x.as_radians() != 0.0 {
            self.r.x = self.r.x+dr.x*self.rot_speed;
            self.rot_x = Self::rot_x_mat(self.r.x.as_radians() as f32);
        }
        if dr.y.as_radians() != 0.0 {
            self.r.y = self.r.y+dr.y*self.rot_speed;
            self.rot_y = Self::rot_y_mat(self.r.y.as_radians() as f32);
        }
        if dr.z.as_radians() != 0.0 {
            self.r.z = self.r.z+dr.z*self.rot_speed;
            self.rot_z = Self::rot_z_mat(self.r.z.as_radians() as f32);
        }
    }

    pub(crate) fn move_forward(&mut self, deg: Angle) {
        // by default, 0 degrees means right
        let angle = deg+Angle::from_degrees(90.0);
        self.t.x = self.t.x+Length::from_meters(self.trans_speed*(angle+self.r.y).as_radians().cos());
        self.t.z = self.t.z+Length::from_meters(self.trans_speed*(angle+self.r.y).as_radians().sin());
        self.trans = Self::trans_mat(self.t);
    }

    pub(crate) fn move_vertical(&mut self, multiplier: i64) {
        self.t.y = self.t.y+Length::from_meters(multiplier as f64*self.trans_speed);
        self.trans = Self::trans_mat(self.t);
    }

    pub(crate) fn view_mat(&self) -> Mat4 {
        Self::local_view_mat(self.trans, self.rot_x, self.rot_y, self.rot_z)
    }

    pub(crate) fn proj_mat(&self) -> Mat4 {
        let focal_len = 1.0/(self.fov/2.0).tan();

        [
            [focal_len/self.aspect_ratio, 0.0, 0.0, 0.0],
            [0.0,-focal_len, 0.0, 0.0],
            [0.0, 0.0,self.near/(self.far-self.near),-1.0],
            [0.0, 0.0,self.near*self.far/(self.far-self.near), 0.0],
        ]
    }

    pub(crate) fn local_view_mat(trans: Mat4, rot_x: Mat4, rot_y: Mat4, rot_z: Mat4) -> Mat4 {
        matrix_prod(matrix_prod(matrix_prod(trans, rot_z), rot_y), rot_x)
    }

    fn trans_mat(t: Translation) -> Mat4 {
        [
            [ 1.0, 0.0, 0.0, 0.0],
            [ 0.0, 1.0, 0.0, 0.0],
            [ 0.0, 0.0, 1.0, 0.0],
            [-t.x.as_meters() as f32,-t.y.as_meters() as f32,-t.z.as_meters() as f32, 1.0],
        ]
    }

    // rotation matrices are passed in as radians
    fn rot_x_mat(rx: f32) -> Mat4 {
        [
            [1.0,      0.0,      0.0,       0.0],
            [0.0, rx.cos(), rx.sin(),       0.0],
            [0.0,-rx.sin(), rx.cos(),       0.0],
            [0.0,      0.0,      0.0,       1.0],
        ]
    }

    fn rot_y_mat(ry: f32) -> Mat4 {
        [
            [ ry.cos(), 0.0,-ry.sin(), 0.0],
            [      0.0, 1.0,      0.0, 0.0],
            [ ry.sin(), 0.0, ry.cos(), 0.0],
            [      0.0, 0.0,      0.0, 1.0],
        ]
    }

    fn rot_z_mat(rz: f32) -> Mat4 {
        [
            [ rz.cos(), rz.sin(), 0.0, 0.0],
            [-rz.sin(), rz.cos(), 0.0, 0.0],
            [      0.0,      0.0, 1.0, 0.0],
            [      0.0,      0.0, 0.0, 1.0],
        ]
    }
}

impl Component for CameraComponent {
    fn render(&self) -> Vec<RenderData> {
        Vec::new()
    }

    fn respond_event(&mut self, event: WorldEvent) -> ComponentEventResponse {
        match event {
            WorldEvent::MouseMotion((x, y)) => {
                self.rotate(Rotation {
                    x: Angle::from_degrees(y), y: Angle::from_degrees(x), z: Angle::from_degrees(0.0)
                });
                self.rotated = true;
            }
            WorldEvent::KeyPressed(key) => {
                match key {
                    VirtualKeyCode::W | VirtualKeyCode::A | VirtualKeyCode::S | VirtualKeyCode::D |
                    VirtualKeyCode::LShift | VirtualKeyCode::Space => {
                        self.translations.push(key);
                    }
                    _ => {}
                }
            }
            WorldEvent::KeyReleased(ref remove_key) => {
                match remove_key {
                    VirtualKeyCode::W | VirtualKeyCode::A | VirtualKeyCode::S | VirtualKeyCode::D |
                    VirtualKeyCode::LShift | VirtualKeyCode::Space => {
                        self.translations.retain(|t| t != remove_key);
                    }
                    _ => {}
                }
            }
            _ => {}
        }

        ComponentEventResponse(vec![], false)
    }

    fn update_state(&mut self, _state: &mut WorldState) {
        if self.rotated || !self.translations.is_empty() {
            for key in self.translations.clone() {
                match key {
                    VirtualKeyCode::W => {
                        self.move_forward(Angle::from_degrees(180.0));
                    }
                    VirtualKeyCode::A => {
                        self.move_forward(Angle::from_degrees(90.0));
                    }
                    VirtualKeyCode::S => {
                        self.move_forward(Angle::from_degrees(0.0));
                    }
                    VirtualKeyCode::D => {
                        self.move_forward(Angle::from_degrees(270.0));
                    }
                    VirtualKeyCode::LShift => {
                        self.move_vertical(-1);
                    }
                    VirtualKeyCode::Space => {
                        self.move_vertical(1);
                    }
                    _ => {},
                }
            }

            self.rotated = false;

            let data = CameraUBO {
                view: self.view_mat(),
                proj: self.proj_mat(),
            };
            self.descriptor.update(data);
        }
    }

    unsafe fn load_descriptors(&mut self, _cmd_pool: vk::CommandPool, _queue: vk::Queue) -> Vec<RenderData> {
        let data = CameraUBO {
            view: self.view_mat(),
            proj: self.proj_mat(),
        };
        self.descriptor.update(data);

        vec![RenderData::InitialDescriptorBuffer(
            vec![self.descriptor.descriptor_buffer_info()], RenderDataPurpose::CameraViewProjection
        )]
    }

    unsafe fn destroy_descriptor(&mut self) {
        self.descriptor.destroy();
    }
}


// TODO: UNIFORM OBJECTS HAVE TO BE ALIGNED
#[derive(Copy, Clone)]
pub(crate) struct CameraUBO {
    pub(crate) view: [[f32;4];4],
    pub(crate) proj: [[f32;4];4],
}

impl Default for CameraUBO {
    fn default() -> Self {
        Self { view: matrix_ident(), proj: matrix_ident() }
    }
}


// CAMERA DESCRIPTOR
pub(crate) struct CameraDescriptor {
    device: Rc<Device>,
    // descriptor fields of uniform buffer
    ubo_buf: vk::Buffer,
    ubo_mem: vk::DeviceMemory,
    ubo_ptr: *mut ffi::c_void,
    ubo_size: vk::DeviceSize,
}

impl CameraDescriptor {
    pub(crate) unsafe fn new(vi: Rc<VulkanInstance>, device: Rc<Device>) -> CameraDescriptor {
        let (ubo_buf, ubo_mem, ubo_ptr, ubo_size) =
            create_host_buffer(vi.clone(), device.clone(), &[CameraUBO::default()], vk::BufferUsageFlags::UNIFORM_BUFFER, false);

        CameraDescriptor {
            device, ubo_buf, ubo_mem, ubo_ptr, ubo_size,
        }
    }

    pub(crate) fn update(&mut self, ubo: CameraUBO) {
        unsafe {
            update_buffer(self.ubo_ptr, &[ubo], self.ubo_size);
        }
    }

    pub(crate) fn descriptor_buffer_info(&self) -> vk::DescriptorBufferInfo {
        vk::DescriptorBufferInfo {
            buffer: self.ubo_buf,
            offset: 0 as vk::DeviceSize,
            range: mem::size_of::<CameraUBO>() as vk::DeviceSize,
        }
    }

    pub(crate) unsafe fn destroy(&self) {
        self.device.destroy_buffer(self.ubo_buf, None);
        self.device.free_memory(self.ubo_mem, None);
    }
}
