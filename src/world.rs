use std::mem;
use std::time::Duration;
use ash::vk;
use winit::event::{VirtualKeyCode};
use crate::component::{Component, RenderData};
use crate::component::camera::Length3D;
use crate::component::texture::TextureIDMapper;
use crate::debug::DebugVisibility;
use crate::util::CmdBufContext;
use crate::shader::Shader;


#[derive(Debug, Copy, Clone, PartialEq)]
pub(crate) enum CardinalDir {
    EAST,
    SOUTH,
    WEST,
    NORTH,
    UNDEFINED,
}

// NO REFERENCES (potentially be used for async/multithreading purposes)
#[derive(Clone, Debug)]
pub(crate) enum WorldEvent {
    // general sync events
    Tick,
    Start,
    DeltaTime(Duration),
    // resources
    NewTextureMapper(TextureIDMapper),
    // window events
    LeftButtonPressed,
    LeftButtonReleased,
    RightButtonPressed,
    RightButtonReleased,
    MiddleButtonPressed,
    MiddleButtonReleased,
    MouseMotion((f64, f64)),
    KeyPressed(VirtualKeyCode),
    KeyReleased(VirtualKeyCode),
    // app events
    UserFaceDir(CardinalDir),
    UserPosition(Length3D),

    // TODO: request events? to reduce constant events emission
}

pub(crate) struct World {
    dbgv: DebugVisibility,
    components: Vec<Box<dyn Component>>,
    events: Vec<WorldEvent>,  // assumes all WorldEvent enums are unique
    events_buffer: Vec<WorldEvent>,
}

impl World {
    pub(crate) fn new(dbgv: DebugVisibility, components: Vec<Box<dyn Component>>) -> World {
        World {
            dbgv,
            components,
            events: vec![WorldEvent::Start],
            events_buffer: Vec::new(),
        }
    }

    pub(crate) unsafe fn load_descriptors(&mut self, ctx: CmdBufContext) -> Vec<RenderData> {
        let mut descriptor_infos = Vec::new();
        for component in &mut self.components {
            let mut t = component.load_descriptors(ctx.clone());
            descriptor_infos.append(&mut t);
        }

        descriptor_infos
    }

    pub(crate) unsafe fn destroy_descriptors(&mut self) {
        for component in &mut self.components {
            component.destroy();
        }
    }

    pub(crate) fn add_window_event(&mut self, e: WorldEvent) {
        self.events_buffer.push(e);
    }

    pub(crate) fn update(&mut self) {
        for mut component in &mut self.components {
            // any events to be removed before next component
            for world_event in &mut self.events {
                let mut event_resp = component.respond_event(world_event.clone());
                self.events_buffer.append(&mut event_resp);
            }
            component.update();
        }

        self.events.clear();
        mem::swap(&mut self.events, &mut self.events_buffer);
        self.events_buffer.clear();
    }

    pub(crate) fn render(&self, shader: &mut Box<dyn Shader>) {
        let mut shader_data = Vec::new();
        for component in &self.components {
            let mut render_data = component.render();
            shader_data.append(&mut render_data);
        }

        if self.dbgv.mtxg_render_output {
            println!("RENDER DATA {:?}", shader_data.len());
        }
        for rd in shader_data {
            shader.recreate_buffer(rd);
        }
    }
}
