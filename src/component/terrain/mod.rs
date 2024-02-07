mod block_gen;

use std::rc::Rc;
use ash::{Device, vk};
use noise::{NoiseFn};
use uom::si::f32::Length;
use crate::chunk_mesh::{ChunkMesh};
use crate::component::{Component, RenderData, RenderDataPurpose};
use crate::component::camera::Length3D;
use crate::component::terrain::block_gen::BlockGenerator;
use crate::handler::VulkanInstance;
use crate::measurement::{blox, chux};
use crate::util::create_host_buffer;
use crate::world::{WorldEvent};


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
    XCross
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
}

#[derive(Copy, Clone, Default)]
pub struct Block(u16);

#[derive(Copy, Clone)]
pub enum BlockCullType {
    Empty,
    Transparent(Block),
    Opaque(Block),
    Obscured,
}


pub(crate) struct Terrain<'b> {
    vi: Rc<VulkanInstance>,
    device: Rc<Device>,

    block_ind: Vec<BlockData<'b>>,

    chunk_mesh: Option<ChunkMesh<BlockGenerator<'b>>>,
    to_render: Vec<RenderData>,
    chunk_update: bool,

    chunk_size: u32,
}

impl<'b> Terrain<'b> {
    pub(crate) fn new(vi: Rc<VulkanInstance>, device: Rc<Device>, block_ind: Vec<BlockData<'b>>) -> Self {
        let chunk_size = Length::new::<chux>(1.0).get::<blox>() as u32;

        Self {
            vi,
            device,
            block_ind,
            chunk_mesh: None,
            to_render: vec![],
            chunk_update: true,
            chunk_size,
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
            WorldEvent::UserPosition(pos) => {
                if let Some(ref mut chunk_mesh) = self.chunk_mesh {
                    self.chunk_update = self.chunk_update || chunk_mesh.update(pos);
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
                    Length3D {
                        x: Length::new::<chux>(1.0),
                        y: Length::new::<chux>(1.0),
                        z: Length::new::<chux>(1.0),
                    },
                    2,
                    block_generator,
                );

                chunk_mesher.initialize();

                self.chunk_mesh.replace(chunk_mesher);
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
                    let (vertex_buffer, vertex_buffer_mem, _, _) = unsafe {
                        create_host_buffer(self.vi.clone(), self.device.clone(), &verts, vk::BufferUsageFlags::VERTEX_BUFFER, true)
                    };
                    let (index_buffer, index_buffer_mem, _, _) = unsafe {
                        create_host_buffer(self.vi.clone(), self.device.clone(), &inds, vk::BufferUsageFlags::INDEX_BUFFER, true)
                    };

                    self.to_render.push(RenderData::RecreateVertexBuffer(
                        vertex_buffer, vertex_buffer_mem, purpose
                    ));
                    self.to_render.push(RenderData::RecreateIndexBuffer(
                        index_buffer, index_buffer_mem, inds.len() as u32, purpose
                    ));
                }
                self.chunk_update = false;
            }
        }
    }
}
