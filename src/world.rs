use std::mem;
use std::time::Duration;
use ash::vk;
use winit::event::{VirtualKeyCode};
use crate::component::{Component, RenderData};
use crate::component::terrain::CubeFaceDir;
use crate::debug::DebugVisibility;
use crate::shader::Shader;


#[derive(Debug, Copy, Clone, PartialEq)]
pub(crate) enum CardinalDir {
    EAST,
    SOUTH,
    WEST,
    NORTH,
    UNDEFINED,
}

#[derive(Copy, Clone)]
pub(crate) enum WorldEvent {
    // general sync events
    Tick,
    Start,
    DeltaTime(Duration),
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
}

#[derive(Clone, Default)]
pub struct WorldState {

}

pub(crate) struct World {
    dbgv: DebugVisibility,
    components: Vec<Box<dyn Component>>,
    events: Vec<WorldEvent>,  // assumes all WorldEvent enums are unique
    events_buffer: Vec<WorldEvent>,
    persistent_state: WorldState,
    persistent_state_buffer: WorldState,
}

impl World {
    pub(crate) fn new(dbgv: DebugVisibility, components: Vec<Box<dyn Component>>) -> World {
        World {
            dbgv,
            components,
            events: vec![WorldEvent::Start],
            events_buffer: Vec::new(),
            persistent_state: WorldState::default(),
            persistent_state_buffer: WorldState::default(),
        }
    }

    pub(crate) unsafe fn load_descriptors(&mut self, cmd_pool: vk::CommandPool, queue: vk::Queue) -> Vec<RenderData> {
        let mut descriptor_infos = Vec::new();
        for component in &mut self.components {
            let mut t = component.load_descriptors(cmd_pool, queue);
            descriptor_infos.append(&mut t);
        }

        descriptor_infos
    }

    pub(crate) unsafe fn destroy_descriptors(&mut self) {
        for component in &mut self.components {
            component.destroy_descriptor();
        }
    }

    pub(crate) fn add_window_event(&mut self, e: WorldEvent) {
        self.events_buffer.push(e);
    }

    // TODO: make new events from component return from the consistent update() method instead of read events
    pub(crate) fn update(&mut self) {

        let mut new_states = Vec::with_capacity(self.components.len());

        for component in &mut self.components {
            // any events to be removed before next component
            for world_event in &self.events {
                let mut event_resp = component.respond_event(*world_event);
                self.events_buffer.append(&mut event_resp.0);
            }
            // world states
            let mut new_state = self.persistent_state.clone();
            component.update_state(&mut new_state);
            new_states.push(new_state);
        }

        // TODO: currently modification of world state does not modify the whole state
        // TODO: implement states diffing (making sure components handle all writing collision)

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
