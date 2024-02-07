use std::rc::Rc;
use ash::{Device, vk};
use winit::event::VirtualKeyCode;
use crate::component::{Component, RenderData, RenderDataPurpose};
use crate::handler::VulkanInstance;
use crate::util::{CmdBufContext, Mat4, matrix_prod};
use crate::world::{CardinalDir, WorldEvent};
use std::{ffi, mem};
use uom::si;
use uom::num_traits::Zero;
use uom::si::f32::{Angle, Length};
use crate::measurement::blox;
use crate::util::{create_host_buffer, matrix_ident, update_buffer};


#[derive(Copy, Clone, Debug)]
pub(crate) struct Length3D {
    pub(crate) x: Length, pub(crate) y: Length, pub(crate) z: Length,
}

impl Length3D {
    pub(crate) fn new(x: Length, y: Length, z: Length) -> Self {
        Self {x,y,z}
    }
}

impl Default for Length3D {
    fn default() -> Self {
        Length3D {x: Length::new::<blox>(0.0), y: Length::new::<blox>(0.0), z: Length::new::<blox>(0.0)}
    }
}

#[derive(Copy, Clone)]
pub(crate) struct Rotation {
    x: Angle, y: Angle, z: Angle
}
impl Default for Rotation {
    fn default() -> Self {
        Rotation {
            x: Angle::new::<si::angle::degree>(0.0),
            y: Angle::new::<si::angle::degree>(0.0),
            z: Angle::new::<si::angle::degree>(0.0)
        }
    }
}

pub(crate) struct CameraComponent {
    descriptor: CameraDescriptor,

    // high-level parameters
    trans_speed: f32,
    rot_speed: f32,
    // camera state
    t: Length3D,  // translations are in blocks
    r: Rotation,
    translations: Vec<VirtualKeyCode>,
    rotated: bool,
    direction: CardinalDir,
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
                      aspect_ratio: f32, fov: f32, trans_speed: f32, rot_speed: f32,
                      init_pos: Length3D
    ) -> CameraComponent {
        // let init_rot = (180.0f32).to_radians();
        CameraComponent {
            descriptor: unsafe { CameraDescriptor::new(vi.clone(), device.clone()) },
            trans_speed, rot_speed, t: init_pos, r: Rotation::default(),
            translations: Vec::new(), rotated: false, direction: Self::determine_dir(Angle::zero()),
            rot_x: Self::rot_x_mat(0.0),
            rot_y: Self::rot_y_mat(0.0),
            rot_z: Self::rot_z_mat(0.0),
            trans: Self::trans_mat(init_pos),
            far: 100000.0, near: 0.1, aspect_ratio, fov: fov.to_radians(),
        }
    }

    pub(crate) fn rotate(&mut self, mut dr: Rotation) {
        if dr.x.get::<si::angle::radian>() != 0.0 {
            dr.x.value *= self.rot_speed;
            self.r.x = self.r.x+dr.x;
            self.rot_x = Self::rot_x_mat(self.r.x.get::<si::angle::radian>());
        }
        if dr.y.get::<si::angle::radian>() != 0.0 {
            dr.y.value *= self.rot_speed;
            self.r.y = self.r.y+dr.y;
            self.rot_y = Self::rot_y_mat(self.r.y.get::<si::angle::radian>());
        }
        if dr.z.get::<si::angle::radian>() != 0.0 {
            dr.z.value *= self.rot_speed;
            self.r.z = self.r.z+dr.z;
            self.rot_z = Self::rot_z_mat(self.r.z.get::<si::angle::radian>());
        }
    }

    pub(crate) fn move_forward(&mut self, deg: Angle) {
        // by default, 0 degrees means right
        let angle = deg+Angle::new::<si::angle::degree>(90.0);
        self.t.x = self.t.x+Length::new::<blox>((self.trans_speed*(angle+self.r.y).cos()).get::<si::ratio::ratio>());
        self.t.z = self.t.z+Length::new::<blox>((self.trans_speed*(angle+self.r.y).sin()).get::<si::ratio::ratio>());
        self.trans = Self::trans_mat(self.t);
    }

    pub(crate) fn move_vertical(&mut self, multiplier: i64) {
        self.t.y = self.t.y+Length::new::<blox>(multiplier as f32*self.trans_speed);
        self.trans = Self::trans_mat(self.t);
    }

    pub(crate) fn determine_dir(angle: Angle) -> CardinalDir {
        let mod_angle = Angle::new::<si::angle::radian>(
            angle.value.rem_euclid(Angle::new::<si::angle::degree>(360.0).value)
        );

        if Angle::new::<si::angle::degree>(360.0-45.0) < mod_angle || mod_angle <= Angle::new::<si::angle::degree>(0.0+45.0) {
            // because this is at a point where the modulo jumps back to 0 and we assume the max will
            // always be 360.0deg, so we just use ||
            CardinalDir::NORTH
        } else if Angle::new::<si::angle::degree>(90.0-45.0) < mod_angle && mod_angle <= Angle::new::<si::angle::degree>(90.0+45.0) {
            CardinalDir::EAST
        } else if Angle::new::<si::angle::degree>(180.0-45.0) < mod_angle && mod_angle <= Angle::new::<si::angle::degree>(180.0+45.0) {
            CardinalDir::SOUTH
        } else if Angle::new::<si::angle::degree>(270.0-45.0) < mod_angle && mod_angle <= Angle::new::<si::angle::degree>(270.0+45.0) {
            CardinalDir::WEST
        } else {
            CardinalDir::UNDEFINED
        }
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

    fn trans_mat(t: Length3D) -> Mat4 {
        [
            [ 1.0, 0.0, 0.0, 0.0],
            [ 0.0, 1.0, 0.0, 0.0],
            [ 0.0, 0.0, 1.0, 0.0],
            [-t.x.get::<blox>(),-t.y.get::<blox>(),-t.z.get::<blox>(), 1.0],
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

    fn respond_event(&mut self, event: WorldEvent) -> Vec<WorldEvent> {
        let mut dir_changed = false;
        match event {
            WorldEvent::MouseMotion((x, y)) => {
                self.rotate(Rotation {
                    x: Angle::new::<si::angle::degree>(y as f32),
                    y: Angle::new::<si::angle::degree>(x as f32),
                    z: Angle::new::<si::angle::degree>(0.0)
                });
                self.rotated = true;

                if self.direction != Self::determine_dir(self.r.y) {
                    self.direction = Self::determine_dir(self.r.y);
                    dir_changed = true;
                }
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
            WorldEvent::Start => {
                dir_changed = true;
            }
            _ => {}
        }

        let mut new_events = Vec::new();
        if dir_changed {
            new_events.push(WorldEvent::UserFaceDir(self.direction));
        }
        if !self.translations.is_empty() {
            new_events.push(WorldEvent::UserPosition(self.t));
        }

        new_events
    }

    fn update(&mut self) {
        if self.rotated || !self.translations.is_empty() {
            for key in self.translations.clone() {
                if let VirtualKeyCode::W = key {
                    self.move_forward(Angle::new::<si::angle::degree>(180.0));
                }
                if let VirtualKeyCode::A = key {
                    self.move_forward(Angle::new::<si::angle::degree>(90.0));
                }
                if let VirtualKeyCode::S = key {
                    self.move_forward(Angle::new::<si::angle::degree>(0.0));
                }
                if let VirtualKeyCode::D = key {
                    self.move_forward(Angle::new::<si::angle::degree>(270.0));
                }
                if let VirtualKeyCode::LShift = key {
                    self.move_vertical(-1);
                }
                if let VirtualKeyCode::Space = key {
                    self.move_vertical(1);
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

    unsafe fn load_descriptors(&mut self, _: CmdBufContext) -> Vec<RenderData> {
        let data = CameraUBO {
            view: self.view_mat(),
            proj: self.proj_mat(),
        };
        self.descriptor.update(data);

        vec![RenderData::InitialDescriptorBuffer(
            vec![self.descriptor.descriptor_buffer_info()], RenderDataPurpose::CameraViewProjection
        )]
    }

    unsafe fn destroy(&mut self) {
        self.descriptor.destroy();
    }
}


// TODO: UNIFORM OBJECTS HAVE TO BE ALIGNED
#[derive(Copy, Clone)]
struct CameraUBO {
    pub(crate) view: [[f32;4];4],
    pub(crate) proj: [[f32;4];4],
}

impl Default for CameraUBO {
    fn default() -> Self {
        Self { view: matrix_ident(), proj: matrix_ident() }
    }
}


// CAMERA DESCRIPTOR
 struct CameraDescriptor {
    device: Rc<Device>,
    // descriptor fields of uniform buffer
    ubo_buf: vk::Buffer,
    ubo_mem: vk::DeviceMemory,
    ubo_ptr: *mut ffi::c_void,
    ubo_size: vk::DeviceSize,
}

impl CameraDescriptor {
     unsafe fn new(vi: Rc<VulkanInstance>, device: Rc<Device>) -> CameraDescriptor {
        let (ubo_buf, ubo_mem, ubo_ptr, ubo_size) =
            create_host_buffer(vi.clone(), device.clone(), &[CameraUBO::default()], vk::BufferUsageFlags::UNIFORM_BUFFER, false);

        CameraDescriptor {
            device, ubo_buf, ubo_mem, ubo_ptr, ubo_size,
        }
    }

    fn update(&mut self, ubo: CameraUBO) {
        unsafe {
            update_buffer(self.ubo_ptr, &[ubo], self.ubo_size);
        }
    }

    fn descriptor_buffer_info(&self) -> vk::DescriptorBufferInfo {
        vk::DescriptorBufferInfo {
            buffer: self.ubo_buf,
            offset: 0 as vk::DeviceSize,
            range: mem::size_of::<CameraUBO>() as vk::DeviceSize,
        }
    }

    unsafe fn destroy(&self) {
        self.device.destroy_buffer(self.ubo_buf, None);
        self.device.free_memory(self.ubo_mem, None);
    }
}
