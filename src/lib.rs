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

use std::path::Path;
use std::time::Instant;
use ash::vk;
use egui::{Id, Modifiers, Pos2, RawInput, Rect, ViewportId, ViewportIdMap, ViewportInfo};
use egui::ahash::HashMapExt;
use uom::si::f32::Length;
use winit::dpi::{PhysicalPosition, PhysicalSize};
use winit::event::{DeviceEvent, ElementState, Event, KeyboardInput, MouseButton, VirtualKeyCode, WindowEvent};
use winit::event_loop::{ControlFlow, EventLoop};
use winit::window::{Fullscreen, Window, WindowBuilder};
use crate::component::camera::{CameraComponent, Length3D};
use crate::component::debug_ui::{DebugUI};
use crate::component::flags::GameFlags;
use crate::debug::DebugVisibility;
use crate::handler::VulkanHandler;
use crate::world::{World, WorldEvent};
use crate::component::terrain::{BlockData, MeshType, Terrain, TextureMapper, TransparencyType};
use crate::component::texture::TextureHandler;
use crate::component::tick::TickSync;
use crate::measurement::{blox};
use crate::shader::chunk::ChunkRasterizer;
use crate::shader::Shader;
use crate::swapchain::{best_surface_color_and_depth_format, SwapchainManager};

mod handler;
pub mod debug;
mod shader;
mod world;
pub mod component;
mod util;
pub mod chunk_mesh;
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
    // Misc
    frame_time: Instant,
}

impl MatrixagonApp {
    pub fn init(validate: bool, debug_visibility: DebugVisibility, fullscreen: bool, mouse_lock: bool) -> MatrixagonApp {
        let prsnt_inp = true;

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
            Box::new(TickSync::new(
                handler.vi.clone(), handler.device.clone(), 1.5,
            )),
            Box::new(GameFlags::new()),
            Box::new(TextureHandler::new(handler.vi.clone(), handler.device.clone(), vec![
                Path::new("C:/Users/andrewshen/documents/matrixagon2/src/resource/block_textures/null.png"),
                Path::new("C:/Users/andrewshen/documents/matrixagon2/src/resource/block_textures/stone.png"),
                Path::new("C:/Users/andrewshen/documents/matrixagon2/src/resource/block_textures/grass_top.png"),
                Path::new("C:/Users/andrewshen/documents/matrixagon2/src/resource/block_textures/grass_side.png"),
                Path::new("C:/Users/andrewshen/documents/matrixagon2/src/resource/block_textures/dirt.png"),
                Path::new("C:/Users/andrewshen/documents/matrixagon2/src/resource/block_textures/sand.png"),
                Path::new("C:/Users/andrewshen/documents/matrixagon2/src/resource/block_textures/grass_flora.png"),
                Path::new("C:/Users/andrewshen/documents/matrixagon2/src/resource/block_textures/grass_top.png"),
                Path::new("C:/Users/andrewshen/documents/matrixagon2/src/resource/block_textures/flower.png"),
                Path::new("C:/Users/andrewshen/documents/matrixagon2/src/resource/block_textures/water.png"),
            ])),
            Box::new(CameraComponent::new(
                handler.vi.clone(), handler.device.clone(), ratio, 70.0, 1.0, 0.05,
                Length3D::new(
                    Length::new::<blox>(0.0),
                    Length::new::<blox>(40.0),
                    Length::new::<blox>(0.0),
                )
            )),
            Box::new(Terrain::new(handler.vi.clone(), handler.device.clone(), handler.get_cmd_buf_context(), vec![
                BlockData {
                    ident: "grass_block",
                    texture_id: TextureMapper::Lateral("grass_top", "dirt", "grass_side"),
                    mesh: MeshType::Cube,
                    transparency: TransparencyType::Opaque,
                },
                BlockData {
                    ident: "dirt",
                    texture_id: TextureMapper::All("dirt"),
                    mesh: MeshType::Cube,
                    transparency: TransparencyType::Opaque,
                },
                BlockData {
                    ident: "stone",
                    texture_id: TextureMapper::All("stone"),
                    mesh: MeshType::Cube,
                    transparency: TransparencyType::Opaque,
                },
                BlockData {
                    ident: "sand",
                    texture_id: TextureMapper::All("sand"),
                    mesh: MeshType::Cube,
                    transparency: TransparencyType::Opaque,
                },
                BlockData {
                    ident: "grass",
                    texture_id: TextureMapper::All("grass_flora"),
                    mesh: MeshType::XCross,
                    transparency: TransparencyType::Transparent,
                },
                BlockData {
                    ident: "flower",
                    texture_id: TextureMapper::All("flower"),
                    mesh: MeshType::XCross,
                    transparency: TransparencyType::Transparent,
                },
                BlockData {
                    ident: "water",
                    texture_id: TextureMapper::All("water"),
                    mesh: MeshType::Fluid,
                    transparency: TransparencyType::Translucent,
                },
                BlockData {
                    ident: "air",
                    texture_id: TextureMapper::All("null"),
                    mesh: MeshType::Empty,
                    transparency: TransparencyType::Transparent,
                },
            ])),
            Box::new(DebugUI::new(handler.vi.clone(), handler.device.clone(), init_raw_input)),
        ]);

        let format = best_surface_color_and_depth_format(debug_visibility, handler.vi.clone());
        let mut shader = unsafe {
            ChunkRasterizer::new(handler.device.clone(), initial_extent, format.0, format.1)
        };

        let mut descriptors = unsafe {
            world.load_descriptors(handler.get_cmd_buf_context())
        };
        let swpc = unsafe {
            SwapchainManager::new(debug_visibility, handler.vi.clone(), handler.device.clone(), shader.renderpass(), shader.attachments(), prsnt_inp)
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
            frame_time: Instant::now(),
        }
    }

    pub fn run(self) {
        // guarantees to move the entire struct, instead of partially moving due to the nature
        // of this closure
        let mut app = self;

        app.event_loop.run(move |e, _, ctrl_flow| match e {
            Event::NewEvents(_) => {
                // begin events (for benchmarking)
                let now = Instant::now();
                let delta = now-app.frame_time;
                app.world.add_window_event(WorldEvent::DeltaTime(delta));

                app.frame_time = now;
            }
            Event::MainEventsCleared => {
                if app.mouse_lock {
                    let _ = app.window.set_cursor_position(PhysicalPosition::new(
                        app.handler.swapchain.as_ref().unwrap().extent.width as f32 / 2.0,
                        app.handler.swapchain.as_ref().unwrap().extent.height as f32 / 2.0
                    ));
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
//     fn test_fps_clock() {
//         loop {
//             thread
//         }
//     }
// }
