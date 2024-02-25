mod chunk_gen;
mod chunk_gen_hf;
mod chunk_gen_mf;
mod mesh_util;
mod terrain_gen;

use std::rc::Rc;
use ash::{Device, vk};
use noise::NoiseFn;
use crate::chunk_mesh::{ChunkGeneratable, ChunkMesh, ChunkRadius, UpdateChunk};
use crate::component::{Component, RenderData, RenderDataPurpose};
use crate::component::camera::Length3D;
use crate::component::terrain::chunk_gen::ChunkGeneratorEF;
use crate::component::terrain::chunk_gen_hf::ChunkGeneratorHF;
use crate::component::terrain::chunk_gen_mf::ChunkGeneratorMF;
use crate::component::terrain::terrain_gen::TerrainGenerator;
use crate::handler::VulkanInstance;
use crate::shader::chunk::ChunkVertex;
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
    Empty,
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
    BorderVisible0(Block),  // visible only if any of its adjacent side is Empty|AlwaysVisible|BorderVisibleFluid|ObscuredFluid
    BorderVisible1(Block),  // a block is adjacent
    BorderVisible2(Block),  // two blocks are adjacent
    BorderVisible3(Block),  // three blocks are adjacent
    BorderVisibleFluid0(Block),  // visible only if any of its adjacent side is Empty|AlwaysVisible
    BorderVisibleFluid1(Block),
    BorderVisibleFluid2(Block),
    BorderVisibleFluid3(Block),
    Obscured,  // when BorderVisible is surrounded by other BorderVisible|Obscured
    ObscuredFluid,  // when BorderVisibleFluid is surrounded by other BorderVisible|BorderVisibleFluid|Obscured|ObscuredFluid
}

impl BlockCullType {
    pub fn decrease_visibility(self) -> Self {
        match self {
            BlockCullType::BorderVisible0(b) => {BlockCullType::BorderVisible1(b)}
            BlockCullType::BorderVisible1(b) => {BlockCullType::BorderVisible2(b)}
            BlockCullType::BorderVisible2(b) => {BlockCullType::BorderVisible3(b)}
            BlockCullType::BorderVisible3(_) => {BlockCullType::Obscured}
            BlockCullType::BorderVisibleFluid0(f) => {BlockCullType::BorderVisibleFluid1(f)}
            BlockCullType::BorderVisibleFluid1(f) => {BlockCullType::BorderVisibleFluid2(f)}
            BlockCullType::BorderVisibleFluid2(f) => {BlockCullType::BorderVisibleFluid3(f)}
            BlockCullType::BorderVisibleFluid3(_) => {BlockCullType::ObscuredFluid}
            _ => {self}
        }
    }
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

    terrain_gen: Rc<TerrainGenerator>,

    chunk_mesh_ef: Option<ChunkMesh<ChunkGeneratorEF<'b>>>,
    chunk_mesh_hf: Option<ChunkMesh<ChunkGeneratorHF<'b>>>,
    chunk_mesh_mf: Option<ChunkMesh<ChunkGeneratorMF<'b>>>,
    to_render: Vec<RenderData>,
    chunk_update_ef: bool,
    chunk_update_hf: bool,
    chunk_update_mf: bool,

    spectator_mode: bool,
}

impl<'b> Terrain<'b> {
    pub(crate) fn new(vi: Rc<VulkanInstance>, device: Rc<Device>, ctx: CmdBufContext, block_ind: Vec<BlockData<'b>>) -> Self {
        Self {
            vi, device, ctx: ctx.clone(),
            block_ind,
            terrain_gen: Rc::new(TerrainGenerator::new()),
            chunk_mesh_ef: None, chunk_mesh_mf: None, chunk_mesh_hf: None,
            to_render: vec![],
            chunk_update_ef: true, chunk_update_hf: true, chunk_update_mf: true,
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
                if let Some(ref mut chunk_mesh) = self.chunk_mesh_ef {
                    let need_update = chunk_mesh.update(UpdateChunk::NewPos(pos));
                    self.chunk_update_ef = self.chunk_update_ef || need_update;
                }
                if let Some(ref mut chunk_mesh) = self.chunk_mesh_hf {
                    let need_update = chunk_mesh.update(UpdateChunk::NewPos(pos));
                    self.chunk_update_hf = self.chunk_update_hf || need_update;
                }
                if let Some(ref mut chunk_mesh) = self.chunk_mesh_mf {
                    let need_update = chunk_mesh.update(UpdateChunk::NewPos(pos));
                    self.chunk_update_mf = self.chunk_update_mf || need_update;
                }
            }
            WorldEvent::NewTextureMapper(txtr_mapper) => {
                let mut chunk_mesh_ef = ChunkMesh::new(
                    Length3D::origin(),
                    ChunkRadius(4, 2), None,
                    ChunkGeneratorEF::new(
                        self.block_ind.clone(), txtr_mapper.clone(), self.terrain_gen.clone()
                    ),
                );
                chunk_mesh_ef.update(UpdateChunk::Forced);
                self.chunk_mesh_ef.replace(chunk_mesh_ef);

                let mut chunk_mesh_hf = ChunkMesh::new(
                    Length3D::origin(),
                    ChunkRadius(2, 1), Some(ChunkRadius(4, 2)),
                    ChunkGeneratorHF::new(
                        self.block_ind.clone(), txtr_mapper.clone(), self.terrain_gen.clone()
                    ),
                );
                chunk_mesh_hf.update(UpdateChunk::Forced);
                self.chunk_mesh_hf.replace(chunk_mesh_hf);

                let mut chunk_mesh_mf = ChunkMesh::new(
                    Length3D::origin(),
                    ChunkRadius(2, 1), Some(ChunkRadius(2, 1)),
                    ChunkGeneratorMF::new(
                        self.block_ind.clone(), txtr_mapper.clone(), self.terrain_gen.clone()
                    ),
                );
                chunk_mesh_mf.update(UpdateChunk::Forced);
                self.chunk_mesh_mf.replace(chunk_mesh_mf);
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

        let mut any_chunk_update = false;
        let mut render_data: Vec<(Vec<ChunkVertex>, Vec<u32>, RenderDataPurpose)> = Vec::new();

        let mut data_aggregator = |rd: Vec<(Vec<ChunkVertex>, Vec<u32>, RenderDataPurpose)>| {
            for (mut verts, inds, purpose) in rd {
                let mut appended = false;
                for (v, i, p) in render_data.iter_mut() {
                    if *p == purpose {
                        let ind_count = v.len() as u32;
                        let mut offsetted_ind = inds.iter().map(|i| i+ind_count).collect();

                        v.append(&mut verts);
                        i.append(&mut offsetted_ind);
                        appended = true;
                        break;
                    }
                }
                if !appended {
                    render_data.push((verts, inds, purpose));
                }
            }
        };

        if self.chunk_update_ef {
            if let Some(ref mut chunk_mesh) = &mut self.chunk_mesh_ef {
                data_aggregator(chunk_mesh.generate_vertices());
                self.chunk_update_ef = false;
                any_chunk_update = true;
            }
        }
        if self.chunk_update_hf {
            if let Some(ref mut chunk_mesh) = &mut self.chunk_mesh_hf {
                data_aggregator(chunk_mesh.generate_vertices());
                self.chunk_update_hf = false;
                any_chunk_update = true;
            }
        }
        if self.chunk_update_mf {
            if let Some(ref mut chunk_mesh) = &mut self.chunk_mesh_mf {
                println!("CHUNK UPDATE MF");
                data_aggregator(chunk_mesh.generate_vertices());
                self.chunk_update_mf = false;
                any_chunk_update = true;
            }
        }

        if any_chunk_update {
            self.to_render = render_data.iter()
                .filter(|(verts, inds, purpose)| {
                    println!("RENDER DATA: {:?} {:?} {:?}", verts.len(), inds.len(), purpose);

                    verts.len() != 0 && inds.len() != 0
                })
                .flat_map(|(verts, inds, purpose)| {
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

                    unsafe {
                        self.device.destroy_buffer(host_vbo, None);
                        self.device.free_memory(host_vmo, None);
                        self.device.destroy_buffer(host_ibo, None);
                        self.device.free_memory(host_imo, None);
                    }

                    [
                        RenderData::RecreateVertexBuffer(
                            local_vbo, local_vmo, *purpose
                        ),
                        RenderData::RecreateIndexBuffer(
                            local_ibo, local_imo, inds.len() as u32, *purpose
                        )
                    ]
                })
                .collect();
        }
    }
}
