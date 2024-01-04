use std::rc::Rc;
use std::mem;
use ash::{Device, vk};
use measurements::Length;
use crate::chunk_mesh::{Chunk, ChunkMesh};
use crate::component::{Component, ComponentEventResponse, RenderData, RenderDataPurpose};
use crate::component::camera::Translation;
use crate::handler::VulkanInstance;
use crate::shader::chunk::ChunkVertex;
use crate::shader::cube::ExCubeVertex;
use crate::util::create_host_buffer;
use crate::world::{WorldEvent, WorldState};


fn gen_cube(loc: (f32, f32, f32), ind_ofs: u32) -> (Vec<ChunkVertex>, Vec<u32>) {
    (
        vec![
            ChunkVertex { pos: [loc.0+0.0, loc.1+0.0, -loc.2+0.0], uv: [0.0, 1.0] },
            ChunkVertex { pos: [loc.0+1.0, loc.1+0.0, -loc.2+0.0], uv: [1.0, 1.0] },
            ChunkVertex { pos: [loc.0+0.0, loc.1+1.0, -loc.2+0.0], uv: [0.0, 0.0] },
            ChunkVertex { pos: [loc.0+1.0, loc.1+1.0, -loc.2+0.0], uv: [1.0, 0.0] },

            ChunkVertex { pos: [loc.0+0.0, loc.1+0.0, -loc.2-1.0], uv: [0.0, 0.0] },
            ChunkVertex { pos: [loc.0+1.0, loc.1+0.0, -loc.2-1.0], uv: [1.0, 0.0] },
            ChunkVertex { pos: [loc.0+0.0, loc.1+1.0, -loc.2-1.0], uv: [0.0, 1.0] },
            ChunkVertex { pos: [loc.0+1.0, loc.1+1.0, -loc.2-1.0], uv: [1.0, 1.0] },
        ],
        vec![
            0,1,2,3,2,1,  // front
            1,5,3,7,3,5,  // right
            5,4,7,6,7,4,  // back
            4,0,6,2,6,0,  // left
            2,3,6,7,6,3,  // top
            1,0,5,4,5,0,  // bottom
        ].into_iter()
            .map(|ind| ind+ind_ofs)
            .collect()
    )
}

#[derive(Copy, Clone, Default)]
pub struct Block(u32);

#[derive(Copy, Clone)]
pub enum BlockGen {
    Empty,
    Transparent(Block),
    Opaque(Block),
    Obscured,
}

const CHUNK_SIZE: u32 = 64;


pub(crate) struct Terrain {
    vi: Rc<VulkanInstance>,
    device: Rc<Device>,

    chunk_mesh: ChunkMesh<BlockGen, ChunkVertex, u32>,
    vbo: (vk::Buffer, vk::DeviceMemory),
    ibo: (vk::Buffer, vk::DeviceMemory, u32),
    to_render: Vec<RenderData>,

    chunk_update: bool,
}

impl Terrain {
    pub(crate) fn new(vi: Rc<VulkanInstance>, device: Rc<Device>) -> Self {

        let mut chunker = ChunkMesh::new(
            Translation {
                x: Length::from_meters(0.0),
                y: Length::from_meters(0.0),
                z: Length::from_meters(0.0),
            },
            Translation {
                x: Length::from_meters(CHUNK_SIZE as f64),
                y: Length::from_meters(CHUNK_SIZE as f64),
                z: Length::from_meters(CHUNK_SIZE as f64),
            },
            Box::new(|pos: Translation, size: Translation| {
                let test_block_obscured = |block| {
                    mem::discriminant(&block) == mem::discriminant(&BlockGen::Opaque(Block::default())) ||
                        mem::discriminant(&block) == mem::discriminant(&BlockGen::Obscured)
                };
                let access = |x,y,z| (y*CHUNK_SIZE*CHUNK_SIZE+x*CHUNK_SIZE+z) as usize;

                let mut voxel = (0..(size.x.as_meters()*size.y.as_meters()*size.z.as_meters()) as u32)
                    .into_iter()
                    .map(|i| {
                        BlockGen::Opaque(Block(0))
                    })
                    .collect::<Box<[BlockGen]>>();

                for x in 1..size.x.as_meters() as u32-1 {
                    for y in 1..size.y.as_meters() as u32-1 {
                        for z in 1..size.z.as_meters() as u32-1 {
                            if  test_block_obscured(voxel[access(x+1,y,z)]) &&
                                test_block_obscured(voxel[access(x-1,y,z)]) &&
                                test_block_obscured(voxel[access(x,y+1,z)]) &&
                                test_block_obscured(voxel[access(x,y-1,z)]) &&
                                test_block_obscured(voxel[access(x,y,z+1)]) &&
                                test_block_obscured(voxel[access(x,y,z-1)]) {
                                voxel[access(x,y,z)] = BlockGen::Obscured;
                            }
                        }
                    }
                }

                voxel
            }),
            Box::new(|chunks: &Vec<Chunk<BlockGen>>| {
                let access = |x,y,z| (y*CHUNK_SIZE*CHUNK_SIZE+x*CHUNK_SIZE+z) as usize;

                let mut total_verts = vec![];
                let mut total_inds = vec![];
                let mut cubes = 0;

                for chunk in chunks {
                    for x in 0..CHUNK_SIZE {
                        for y in 0..CHUNK_SIZE {
                            for z in 0..CHUNK_SIZE {
                                if let BlockGen::Opaque(block) = &chunk.voxels[access(x,y,z)] {
                                    let (mut verts, mut inds) = gen_cube(
                                        (chunk.pos.x.as_meters() as f32+x as f32,
                                         chunk.pos.y.as_meters() as f32+y as f32,
                                         chunk.pos.z.as_meters() as f32+z as f32),
                                        cubes*8
                                    );
                                    total_verts.append(&mut verts);
                                    total_inds.append(&mut inds);
                                    cubes += 1;
                                }
                            }
                        }
                    }
                }
                (total_verts, total_inds)
            })
        );

        chunker.update(Translation::default());

        // println!("VERTICES {total_verts:?}");
        let (total_verts, total_inds) = chunker.generate_vertices();

        let (vertex_buffer, vertex_buffer_mem, _, _) = unsafe {
            create_host_buffer(vi.clone(), device.clone(), &total_verts, vk::BufferUsageFlags::VERTEX_BUFFER, true)
        };

        let (index_buffer, index_buffer_mem, _, _) = unsafe {
            create_host_buffer(vi.clone(), device.clone(), &total_inds, vk::BufferUsageFlags::INDEX_BUFFER, true)
        };

        Self {
            vi,
            device,
            chunk_mesh: chunker,
            vbo: (vertex_buffer, vertex_buffer_mem),
            ibo: (index_buffer, index_buffer_mem, total_inds.len() as u32),
            to_render: vec![],
            chunk_update: true,
        }
    }
}

impl Component for Terrain {
    fn render(&self) -> Vec<RenderData> {
        self.to_render.clone()
    }

    fn respond_event(&mut self, event: WorldEvent) -> ComponentEventResponse {
        ComponentEventResponse::default()
    }

    fn update_state(&mut self, state: &mut WorldState) {
        self.to_render.clear();
        if self.chunk_update {
            self.to_render.push(RenderData::RecreateVertexBuffer(
                self.vbo.0, self.vbo.1, RenderDataPurpose::TerrainVertices
            ));
            self.to_render.push(RenderData::RecreateIndexBuffer(
                self.ibo.0, self.ibo.1, self.ibo.2, RenderDataPurpose::TerrainVertices
            ));
            self.chunk_update = false;
        }
    }
}
