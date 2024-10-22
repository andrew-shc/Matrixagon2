use winit::event::VirtualKeyCode;
use crate::component::{Component, RenderData};
use crate::world::WorldEvent;

pub(crate) struct WorldFlags {
    spectator_mode: bool
}

impl WorldFlags {
    pub(crate) fn new() -> Self {
        Self {
            spectator_mode: false,
        }
    }
}

impl Component for WorldFlags {
    fn render(&self) -> Vec<RenderData> {
        vec![]
    }

    fn respond_event(&mut self, event: WorldEvent) -> Vec<WorldEvent> {
        match event {
            WorldEvent::KeyPressed(key) => {
                match key {
                    VirtualKeyCode::O => {
                        self.spectator_mode = !self.spectator_mode;
                        vec![
                            WorldEvent::SpectatorMode(self.spectator_mode)
                        ]
                    }
                    _ => {vec![]}
                }
            }
            _ => {vec![]}
        }
    }

    fn update(&mut self) {

    }
}
