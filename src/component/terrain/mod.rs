mod chunk_gen;
mod chunk_gen_hf;
mod chunk_gen_mf;
mod mesh_util;

use std::rc::Rc;
use ash::{Device, vk};
use noise::NoiseFn;
use uom::si::f32::Length;
use winit::event::VirtualKeyCode;
use crate::chunk_mesh::{ChunkGeneratable, ChunkMesh};
use crate::component::{Component, RenderData};
use crate::component::camera::Length3D;
use crate::component::terrain::chunk_gen::BlockGenerator;
use crate::handler::VulkanInstance;
use crate::measurement::{blox, chux, chux_hf, chux_mf};
use crate::util::{CmdBufContext, create_host_buffer, create_local_buffer};
use crate::world::WorldEvent;


#[derive(Copy, Clone)]
pub(crate) enum FaceDir {
    FRONT,
    RIGHT,
    BACK,
    LEFT,
    TOP,
    BOTTOM
}

#[derive(Copy, Clone, Debug)]
pub enum MeshType {
    Cube,
    XCross,
    Fluid
}

#[derive(Copy, Clone, Debug)]
pub enum TransparencyType {
    Opaque,
    Transparent,  // full opacity or no opacity
    Translucent,  // partial opacity
}

#[derive(Copy, Clone, Debug)]
pub enum BlockCullType {
    Empty,
    AlwaysVisible(Block),  // always visible (not culled) regardless of any adjacent condition
    BorderVisible(Block),  // visible only if any of its adjacent side is Empty|AlwaysVisible|BorderVisibleFluid|ObscuredFluid
    BorderVisibleFluid(Block),  // visible only if any of its adjacent side is Empty|AlwaysVisible
    Obscured,  // when BorderVisible is surrounded by other BorderVisible|Obscured
    ObscuredFluid,  // when BorderVisibleFluid is surrounded by other BorderVisible|BorderVisibleFluid|Obscured|ObscuredFluid
}

#[derive(Copy, Clone, Debug)]
pub enum TextureMapper<'s> {
    All(&'s str),
    Lateral(&'s str, &'s str, &'s str),  // top, bottom, lateral
    Unique(&'s str, &'s str, &'s str, &'s str, &'s str, &'s str),  // top, bottom, E (right), S (front), W (left), N (back)
}

impl<'s> TextureMapper<'s> {
    fn default(&self) -> &'s str {
        self.top()
    }

    // front facing texture and so on ...
    fn front(&self) -> &'s str {
        match self {
            TextureMapper::All(t) => {t}
            TextureMapper::Lateral(_, _, t) => {t}
            TextureMapper::Unique(_, _, _, t, _, _) => {t}
        }
    }

    fn back(&self) -> &'s str {
        match self {
            TextureMapper::All(t) => {t}
            TextureMapper::Lateral(_, _, t) => {t}
            TextureMapper::Unique(_, _, _, _, _, t) => {t}
        }
    }

    fn left(&self) -> &'s str {
        match self {
            TextureMapper::All(t) => {t}
            TextureMapper::Lateral(_, _, t) => {t}
            TextureMapper::Unique(_, _, _, _, t, _) => {t}
        }
    }

    fn right(&self) -> &'s str {
        match self {
            TextureMapper::All(t) => {t}
            TextureMapper::Lateral(_, _, t) => {t}
            TextureMapper::Unique(_, _, t, _, _, _) => {t}
        }
    }

    fn top(&self) -> &'s str {
        match self {
            TextureMapper::All(t) => {t}
            TextureMapper::Lateral(t, _, _) => {t}
            TextureMapper::Unique(t, _, _, _, _, _) => {t}
        }
    }

    fn bottom(&self) -> &'s str {
        match self {
            TextureMapper::All(t) => {t}
            TextureMapper::Lateral(_, t, _) => {t}
            TextureMapper::Unique(_, t, _, _, _, _) => {t}
        }
    }
}

#[derive(Copy, Clone, Debug)]
pub struct BlockData<'s> {
    pub ident: &'s str,
    pub texture_id: TextureMapper<'s>,
    pub mesh: MeshType,
    pub transparency: TransparencyType,
}

#[derive(Copy, Clone, Debug, Default)]
pub struct Block(u16);


pub(crate) struct Terrain<'b> {
    vi: Rc<VulkanInstance>,
    device: Rc<Device>,
    ctx: CmdBufContext,

    block_ind: Vec<BlockData<'b>>,

    chunk_mesh: Option<ChunkMesh<BlockGenerator<'b>>>,
    to_render: Vec<RenderData>,
    chunk_update: bool,

    chunk_size: u32,

    spectator_mode: bool,
}

impl<'b> Terrain<'b> {
    pub(crate) fn new(vi: Rc<VulkanInstance>, device: Rc<Device>, ctx: CmdBufContext, block_ind: Vec<BlockData<'b>>) -> Self {
        let chunk_size = Length::new::<chux>(1.0).get::<blox>() as u32;

        Self {
            vi, device, ctx: ctx.clone(),
            block_ind,
            chunk_mesh: None,
            to_render: vec![],
            chunk_update: true,
            chunk_size,
            spectator_mode: false,
        }
    }
}

impl Component for Terrain<'static> {
    fn render(&self) -> Vec<RenderData> {
        // println!("RENDER() {}", self.to_render.len());
        self.to_render.clone()
    }

    fn respond_event(&mut self, event: WorldEvent) -> Vec<WorldEvent> {
        match event {
            WorldEvent::UserPosition(pos) if !self.spectator_mode => {
                if let Some(ref mut chunk_mesh) = self.chunk_mesh {
                    let need_update = chunk_mesh.update(pos);
                    self.chunk_update = self.chunk_update || need_update;
                }
            }
            WorldEvent::NewTextureMapper(txtr_mapper) => {
                let block_generator = BlockGenerator::new(
                    self.chunk_size, self.block_ind.clone(), txtr_mapper
                );

                let mut chunk_mesher = ChunkMesh::new(
                    Length3D {
                        x: Length::new::<blox>(0.0),
                        y: Length::new::<blox>(0.0),
                        z: Length::new::<blox>(0.0),
                    },
                    4, 2,
                    block_generator,
                );

                chunk_mesher.initialize();

                self.chunk_mesh.replace(chunk_mesher);
            }
            WorldEvent::SpectatorMode(enabled) => {
                self.spectator_mode = enabled;
            }
            _ => {}
        }

        vec![]
    }

    fn update(&mut self) {
        self.to_render.clear();
        if let Some(ref mut chunk_mesh) = &mut self.chunk_mesh {
            if self.chunk_update {
                for (verts, inds, purpose) in chunk_mesh.generate_vertices() {
                    let (host_vbo, host_vmo, _, host_vbo_size) = unsafe {
                        create_host_buffer(self.vi.clone(), self.device.clone(), &verts, vk::BufferUsageFlags::TRANSFER_SRC | vk::BufferUsageFlags::VERTEX_BUFFER, true)
                    };
                    let (host_ibo, host_imo, _, host_ibo_size) = unsafe {
                        create_host_buffer(self.vi.clone(), self.device.clone(), &inds, vk::BufferUsageFlags::TRANSFER_SRC | vk::BufferUsageFlags::INDEX_BUFFER, true)
                    };
                    let (local_vbo, local_vmo, _) = unsafe {
                        create_local_buffer(self.vi.clone(), self.device.clone(), host_vbo_size, vk::BufferUsageFlags::TRANSFER_DST | vk::BufferUsageFlags::VERTEX_BUFFER)
                    };
                    let (local_ibo, local_imo, _) = unsafe {
                        create_local_buffer(self.vi.clone(), self.device.clone(), host_ibo_size, vk::BufferUsageFlags::TRANSFER_DST | vk::BufferUsageFlags::INDEX_BUFFER)
                    };

                    unsafe { self.ctx.record(|cmd_buf| {
                        let vert_buf_region = [vk::BufferCopy {src_offset: 0, dst_offset: 0, size: host_vbo_size}];
                        self.device.cmd_copy_buffer(cmd_buf, host_vbo, local_vbo, &vert_buf_region);
                        let indx_buf_region = [vk::BufferCopy {src_offset: 0, dst_offset: 0, size: host_ibo_size}];
                        self.device.cmd_copy_buffer(cmd_buf, host_ibo, local_ibo, &indx_buf_region);
                    }); }

                    self.to_render.push(RenderData::RecreateVertexBuffer(
                        local_vbo, local_vmo, purpose
                    ));
                    self.to_render.push(RenderData::RecreateIndexBuffer(
                        local_ibo, local_imo, inds.len() as u32, purpose
                    ));

                    unsafe {
                        self.device.destroy_buffer(host_vbo, None);
                        self.device.free_memory(host_vmo, None);
                        self.device.destroy_buffer(host_ibo, None);
                        self.device.free_memory(host_imo, None);
                    }
                }
                self.chunk_update = false;
            }
        }
    }
}
