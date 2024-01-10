/*
    Matrixagon 2: An experimental open-world voxel renderer.
    Copyright (C) 2024  Andrew Shen

    This program is free software: you can redistribute it and/or modify
    it under the terms of the GNU Affero General Public License as published
    by the Free Software Foundation, either version 3 of the License, or
    (at your option) any later version.

    This program is distributed in the hope that it will be useful,
    but WITHOUT ANY WARRANTY; without even the implied warranty of
    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
    GNU Affero General Public License for more details.

    You should have received a copy of the GNU Affero General Public License
    along with this program.  If not, see <https://www.gnu.org/licenses/>.
 */

#[macro_use]
extern crate uom;

use std::collections::HashMap;
use ash::vk;
use egui::{Context, Id, Modifiers, Pos2, RawInput, Rect, ViewportId, ViewportIdMap, ViewportInfo};
use egui::ahash::HashMapExt;
use winit::dpi::{PhysicalPosition, PhysicalSize};
use winit::event::{DeviceEvent, ElementState, Event, KeyboardInput, MouseButton, VirtualKeyCode, WindowEvent};
use winit::event_loop::{ControlFlow, EventLoop};
use winit::window::{Fullscreen, Window, WindowBuilder};
use crate::component::camera::CameraComponent;
use crate::component::debug_ui::{DebugUI, DebugUIData};
use crate::debug::DebugVisibility;
use crate::handler::VulkanHandler;
use crate::world::{World, WorldEvent};
use crate::component::terrain::Terrain;
use crate::component::texture::TextureHandler;
use crate::shader::chunk::ChunkRasterizer;
use crate::shader::Shader;
use crate::swapchain::{best_surface_color_and_depth_format, SwapchainManager};

mod handler;
pub mod debug;
mod shader;
mod world;
mod component;
mod util;
mod chunk_mesh;
mod measurement;
mod swapchain;
mod framebuffer;


pub struct MatrixagonApp {
    // Debug
    debug_visibility: DebugVisibility,
    // Window management
    event_loop: EventLoop<()>,
    window: Window,
    window_render: bool,
    mouse_lock: bool,
    // Main app fields
    world: World,
    handler: VulkanHandler,
}

impl MatrixagonApp {
    pub fn init(validate: bool, debug_visibility: DebugVisibility, fullscreen: bool, mouse_lock: bool) -> MatrixagonApp {
        /*
        - Window Management [app itself]
        - Camera [as a component]
        - Initial components (default + user-defined) [defined in creating world]
        - Descriptors [among the components]
        - World [created in the app]
        - ChunkMesh & UIMesh [created in necessary components to handle spatial data discretely]
        - Shader [TODO: ??? needs vulkan instance, yet the world needs shader to render and accept data]
        - VulkanHandler [created at the end to provide the render context]
         */

        let initial_extent = if fullscreen {
            vk::Extent2D {
                width: 2560,
                height: 1600,
            }
        } else {
            vk::Extent2D {
                width: 1000,
                height: 1000,
            }
        };

        let event_loop = EventLoop::new();
        let window = if fullscreen {
            WindowBuilder::new().with_fullscreen(Some(Fullscreen::Borderless(None)))
        } else {
            WindowBuilder::new().with_inner_size(PhysicalSize::<u32>::from((initial_extent.width, initial_extent.height)))
        }
            .with_visible(true)
            .with_title("Matrixagon 2")
            .build(&event_loop)
            .expect("Window builder failed");

        window.set_cursor_position(PhysicalPosition::new(
            initial_extent.width as f32/2.0, initial_extent.height as f32/2.0
        )).unwrap();

        let mut handler = VulkanHandler::init(&event_loop, &window, validate, debug_visibility);

        if debug_visibility.mtxg_output {
            println!("WINDOW SCALE FACTOR {:?}", window.scale_factor() as f32);
        }

        // let mut ui_handler = EguiHandler::new(handler.vi.clone(), handler.device.clone());
        let init_raw_input = RawInput {
            viewport_id: ViewportId(Id::new(0)),
            viewports: {
                let mut m = ViewportIdMap::new();
                m.insert(
                    ViewportId(Id::new(0)),
                    ViewportInfo {
                        parent: None,
                        title: None,
                        events: vec![],
                        native_pixels_per_point: Some(window.scale_factor() as f32),
                        monitor_size: None,
                        inner_rect: Some(Rect {
                            min: Pos2::from((0.0, 0.0)),
                            max: Pos2::from((window.inner_size().width as f32, window.inner_size().height as f32))
                        }),
                        outer_rect: Some(Rect {
                            min: Pos2::from((0.0, 0.0)),
                            max: Pos2::from((window.outer_size().width as f32, window.outer_size().height as f32))
                        }),
                        minimized: None,
                        maximized: None,
                        fullscreen: Some(fullscreen),
                        focused: Some(true),
                    }
                );
                m
            },
            screen_rect: Some(Rect {
                min: Pos2::from((0.0, 0.0)),
                max: Pos2::from((160.0, 500.0))
            }),
            modifiers: Modifiers::default(),
            events: Vec::new(),
            focused: true,
            ..Default::default()
        };

        let ratio = initial_extent.width as f32/initial_extent.height as f32;
        let mut world = World::new(debug_visibility, vec![
            Box::new(CameraComponent::new(
                handler.vi.clone(), handler.device.clone(), ratio, 70.0, 0.01, 0.05
            )),
            Box::new(Terrain::new(handler.vi.clone(), handler.device.clone())),
            Box::new(TextureHandler::new(handler.vi.clone(), handler.device.clone())),
            Box::new(DebugUI::new(handler.vi.clone(), handler.device.clone(), init_raw_input)),
        ]);

        let format = best_surface_color_and_depth_format(debug_visibility, handler.vi.clone());
        let mut shader = unsafe {
            ChunkRasterizer::new(handler.device.clone(), initial_extent, format.0, format.1)
        };

        let mut descriptors = unsafe {
            world.load_descriptors(handler.cmd_pool, handler.gfxs_queue)
        };
        let swpc = unsafe {
            SwapchainManager::new(debug_visibility, handler.vi.clone(), handler.device.clone(), shader.renderpass(), shader.attachments())
        };
        descriptors.append(&mut unsafe { swpc.fbm.get_input_attachment_descriptors() });
        unsafe { shader.write_descriptors(descriptors); }

        handler.load_swapchain(swpc);
        handler.load_shader(shader);

        MatrixagonApp {
            debug_visibility,
            event_loop,
            window,
            window_render: false,
            mouse_lock,
            world,
            handler,
        }
    }

    pub fn run(mut self) {
        // guarantees to move the entire struct, instead of partially moving due to the nature
        // of this closure
        let mut app = self;

        app.event_loop.run(move |e, _, ctrl_flow| match e {
            Event::NewEvents(_) => {
                // begin events (for benchmarking)
            }
            Event::MainEventsCleared => {
                if app.mouse_lock {
                    app.window.set_cursor_position(PhysicalPosition::new(
                        app.handler.swapchain.as_ref().unwrap().extent.width as f32/2.0,
                        app.handler.swapchain.as_ref().unwrap().extent.height as f32/2.0
                    )).unwrap();
                }

                // update app state
                app.world.update();
            }
            Event::RedrawRequested(_) => {
                if app.debug_visibility.mtxg_output {
                    println!("REDRAW REQUESTED");
                }
            }
            Event::RedrawEventsCleared => unsafe {
                // final event (for drawing and benchmarking)
                if app.window_render {
                    app.world.render(app.handler.obtain_shader_mut_ref());

                    // app.ui_handler.handle_output();

                    app.handler.draw_frame();
                }
            }
            Event::WindowEvent {event: win_event, window_id: _} => {
                match win_event {
                    WindowEvent::CloseRequested => {
                        *ctrl_flow = ControlFlow::Exit;
                    }
                    WindowEvent::Occluded(occluded) => {
                        if app.debug_visibility.vk_setup_output {
                            println!("OCCLUDED? {occluded}")
                        }
                    }
                    WindowEvent::Resized(size) => {
                        if app.debug_visibility.vk_swapchain_output {
                            println!("RESIZED? {size:?}");
                        }
                        if size.height == 0 || size.width == 0 {
                            app.window_render = false;
                        } else {
                            app.window_render = true;
                        }
                    }
                    WindowEvent::MouseInput {state, button, ..} => match state {
                        ElementState::Pressed => match button {
                            MouseButton::Left => {app.world.add_window_event(WorldEvent::LeftButtonPressed)}
                            MouseButton::Right => {app.world.add_window_event(WorldEvent::RightButtonPressed)}
                            MouseButton::Middle => {app.world.add_window_event(WorldEvent::MiddleButtonPressed)}
                            _ => {},
                        },
                        ElementState::Released => match button {
                            MouseButton::Left => {app.world.add_window_event(WorldEvent::LeftButtonReleased)}
                            MouseButton::Right => {app.world.add_window_event(WorldEvent::RightButtonReleased)}
                            MouseButton::Middle => {app.world.add_window_event(WorldEvent::MiddleButtonReleased)}
                            _ => {},
                        }
                    }
                    _ => {}
                }
            }
            Event::DeviceEvent {device_id: _, event: dev_event} => {
                match dev_event {
                    DeviceEvent::MouseMotion {delta} => {
                        app.world.add_window_event(WorldEvent::MouseMotion(delta));
                    }
                    DeviceEvent::Key(KeyboardInput {state, virtual_keycode, ..}) => {
                        if let Some(key) = virtual_keycode {
                            match state {
                                ElementState::Pressed => {
                                    app.world.add_window_event(WorldEvent::KeyPressed(key));
                                }
                                ElementState::Released => {
                                    app.world.add_window_event(WorldEvent::KeyReleased(key));

                                    match key {
                                        VirtualKeyCode::Escape => {
                                            *ctrl_flow = ControlFlow::Exit;
                                        }
                                        VirtualKeyCode::T => {
                                            app.mouse_lock = !app.mouse_lock;
                                        }
                                        _ => {}
                                    };
                                }
                            };
                        }
                    }
                    _ => {}
                }
            }
            Event::LoopDestroyed => unsafe {
                if app.debug_visibility.vk_setup_output {
                    println!("EVENT LOOP DESTROYED: Vulkan object destroyed & cleaned-up");
                }

                app.handler.device.device_wait_idle().unwrap();

                app.world.destroy_descriptors();

                app.handler.destroy();
            }
            _ => {}
        })
    }
}



// #[cfg(test)]
// mod tests {
//     use super::*;
//
//     #[test]
//     fn general() {
//         let mtxg = Matrixagon::init(false);
//         // mtxg.load_shader(StandardRasterizer::new());
//         mtxg.run();
//     }
// }
