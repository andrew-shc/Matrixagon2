use std::rc::Rc;
use std::mem;
use ash::{Device, vk};
use uom::si::f32::Length;
use crate::chunk_mesh::{Chunk, ChunkMesh};
use crate::component::{Component, ComponentEventResponse, RenderData, RenderDataPurpose};
use crate::component::camera::Length3D;
use crate::handler::VulkanInstance;
use crate::measurement::{blox, chux};
use crate::shader::chunk::ChunkVertex;
use crate::util::create_host_buffer;
use crate::world::{WorldEvent, WorldState};


#[derive(Copy, Clone)]
pub(crate) enum CubeFaceDir {
    FRONT,
    RIGHT,
    BACK,
    LEFT,
    TOP,
    BOTTOM
}

fn gen_face(loc: (f32, f32, f32), ind_ofs: u32, face: CubeFaceDir) -> (Vec<ChunkVertex>, Vec<u32>) {
    let (v, i) = match face {
        CubeFaceDir::FRONT => {(
            vec![
                ChunkVertex { pos: [loc.0+0.0, loc.1+0.0, -loc.2+0.0], uv: [0.0, 1.0] },
                ChunkVertex { pos: [loc.0+1.0, loc.1+0.0, -loc.2+0.0], uv: [1.0, 1.0] },
                ChunkVertex { pos: [loc.0+0.0, loc.1+1.0, -loc.2+0.0], uv: [0.0, 0.0] },
                ChunkVertex { pos: [loc.0+1.0, loc.1+1.0, -loc.2+0.0], uv: [1.0, 0.0] },
            ],
            vec![
                0,1,2,3,2,1
            ]
        )}
        CubeFaceDir::RIGHT => {(
            vec![
                ChunkVertex { pos: [loc.0+1.0, loc.1+0.0, -loc.2+0.0], uv: [1.0, 1.0] },
                ChunkVertex { pos: [loc.0+1.0, loc.1+1.0, -loc.2+0.0], uv: [1.0, 0.0] },
                ChunkVertex { pos: [loc.0+1.0, loc.1+0.0, -loc.2-1.0], uv: [0.0, 1.0] },
                ChunkVertex { pos: [loc.0+1.0, loc.1+1.0, -loc.2-1.0], uv: [0.0, 0.0] },
            ],
            vec![
                0,2,1,3,1,2
            ]
        )}
        CubeFaceDir::BACK => {(
            vec![
                ChunkVertex { pos: [loc.0+0.0, loc.1+0.0, -loc.2-1.0], uv: [0.0, 1.0] },
                ChunkVertex { pos: [loc.0+1.0, loc.1+0.0, -loc.2-1.0], uv: [1.0, 1.0] },
                ChunkVertex { pos: [loc.0+0.0, loc.1+1.0, -loc.2-1.0], uv: [0.0, 0.0] },
                ChunkVertex { pos: [loc.0+1.0, loc.1+1.0, -loc.2-1.0], uv: [1.0, 0.0] },
            ],
            vec![
                1,0,3,2,3,0
            ]
        )}
        CubeFaceDir::LEFT => {(
            vec![
                ChunkVertex { pos: [loc.0+0.0, loc.1+0.0, -loc.2+0.0], uv: [1.0, 1.0] },
                ChunkVertex { pos: [loc.0+0.0, loc.1+1.0, -loc.2+0.0], uv: [1.0, 0.0] },
                ChunkVertex { pos: [loc.0+0.0, loc.1+0.0, -loc.2-1.0], uv: [0.0, 1.0] },
                ChunkVertex { pos: [loc.0+0.0, loc.1+1.0, -loc.2-1.0], uv: [0.0, 0.0] },
            ],
            vec![
                2,0,3,1,3,0
            ]
        )}
        CubeFaceDir::TOP => {(
            vec![
                ChunkVertex { pos: [loc.0+0.0, loc.1+1.0, -loc.2+0.0], uv: [1.0, 1.0] },
                ChunkVertex { pos: [loc.0+1.0, loc.1+1.0, -loc.2+0.0], uv: [0.0, 1.0] },
                ChunkVertex { pos: [loc.0+0.0, loc.1+1.0, -loc.2-1.0], uv: [1.0, 0.0] },
                ChunkVertex { pos: [loc.0+1.0, loc.1+1.0, -loc.2-1.0], uv: [0.0, 0.0] },
            ],
            vec![
                0,1,2,3,2,1
            ]
        )}
        CubeFaceDir::BOTTOM => {(
            vec![
                ChunkVertex { pos: [loc.0+0.0, loc.1+0.0, -loc.2+0.0], uv: [0.0, 1.0] },
                ChunkVertex { pos: [loc.0+1.0, loc.1+0.0, -loc.2+0.0], uv: [1.0, 1.0] },
                ChunkVertex { pos: [loc.0+0.0, loc.1+0.0, -loc.2-1.0], uv: [0.0, 0.0] },
                ChunkVertex { pos: [loc.0+1.0, loc.1+0.0, -loc.2-1.0], uv: [1.0, 0.0] },
            ],
            vec![
                1,0,3,2,3,0
            ]
        )}
    };
    let i = i.into_iter()
        .map(|ind| ind+ind_ofs)
        .collect();
    (v,i)
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
        let chunk_size = Length::new::<chux>(1.0).get::<blox>() as u32;
        let access = move |x,y,z| (y*chunk_size*chunk_size+x*chunk_size+z) as usize;
        let test_block_obscured = |block| {
            mem::discriminant(&block) == mem::discriminant(&BlockGen::Opaque(Block::default())) ||
                mem::discriminant(&block) == mem::discriminant(&BlockGen::Obscured)
        };
        let test_coord_in_chunk = move |x: u32, y: u32, z: u32| {
            0 <= x && x < chunk_size && 0 <= y && y < chunk_size && 0 <= z && z < chunk_size
        };

        let mut chunker = ChunkMesh::new(
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
            Box::new(move |pos: Length3D, size: Length3D| {
                let mut voxel = (0..(size.x.get::<blox>()*size.y.get::<blox>()*size.z.get::<blox>()) as u32)
                    .into_iter()
                    .map(|i| {
                        BlockGen::Opaque(Block(0))
                    })
                    .collect::<Box<[BlockGen]>>();

                for x in 1..size.x.get::<blox>() as u32-1 {
                    for y in 1..size.y.get::<blox>() as u32-1 {
                        for z in 1..size.z.get::<blox>() as u32-1 {
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
            Box::new(move |chunks: &Vec<Chunk<BlockGen>>| {
                let mut total_verts = vec![];
                let mut total_inds = vec![];
                let mut faces = 0;

                for chunk in chunks {
                    let mut local_gen_face = |x: u32, y: u32, z: u32, cube_face_dir: CubeFaceDir| {
                        let (mut verts, mut inds) = gen_face(
                            (chunk.pos.x.get::<blox>()+x as f32,
                             chunk.pos.y.get::<blox>()+y as f32,
                             chunk.pos.z.get::<blox>()+z as f32),
                            faces*4,
                            cube_face_dir,
                        );
                        total_verts.append(&mut verts);
                        total_inds.append(&mut inds);
                        faces += 1;
                    };

                    for x in 0..chunk_size {
                        for y in 0..chunk_size {
                            for z in 0..chunk_size {
                                if let BlockGen::Opaque(block) = &chunk.voxels[access(x,y,z)] {
                                    // let (mut verts, mut inds) = gen_cube(
                                    //     (chunk.pos.x.get::<blox>()+x as f32,
                                    //      chunk.pos.y.get::<blox>()+y as f32,
                                    //      chunk.pos.z.get::<blox>()+z as f32),
                                    //     cubes*8
                                    // );
                                    let mut local_checked_gen_face = |dx, dy, dz, face_dir| {
                                        if (dx == -1 && x == 0) || (dy == -1 && y == 0) || (dz == -1 && z == 0) {
                                            // if delta face coord is not in chunk (other side)
                                            local_gen_face(x,y,z,face_dir);
                                            return;
                                        }
                                        if test_coord_in_chunk((x as i32+dx) as u32,(y as i32+dy) as u32,(z as i32+dz) as u32) {
                                            if !test_block_obscured(chunk.voxels[access((x as i32+dx) as u32,(y as i32+dy) as u32,(z as i32+dz) as u32)]) {
                                                // if delta face coord is in chunk and not obscured
                                                local_gen_face(x,y,z,face_dir);
                                            }
                                        } else {
                                            // if delta face coord is not in chunk
                                            local_gen_face(x,y,z,face_dir);
                                        }
                                    };

                                    local_checked_gen_face( 0, 0, -1, CubeFaceDir::FRONT);
                                    local_checked_gen_face( 1, 0, 0, CubeFaceDir::RIGHT);
                                    local_checked_gen_face( 0, 0,1, CubeFaceDir::BACK);
                                    local_checked_gen_face(-1, 0, 0, CubeFaceDir::LEFT);
                                    local_checked_gen_face( 0, 1, 0, CubeFaceDir::TOP);
                                    local_checked_gen_face( 0,-1, 0, CubeFaceDir::BOTTOM);

                                    // if test_coord_in_chunk(x,y+1,z) {
                                    //     if !test_block_obscured(chunk.voxels[access(x,y,z+1)]) {
                                    //         local_gen_face(x,y,z,CubeFaceDir::FRONT);
                                    //     }
                                    // } else {
                                    //     local_gen_face(x,y,z,CubeFaceDir::FRONT);
                                    // }
                                    // if test_coord_in_chunk(x,y+1,z) {
                                    //     if !test_block_obscured(chunk.voxels[access(x+1,y,z)]) {
                                    //         local_gen_face(x,y,z,CubeFaceDir::RIGHT);
                                    //     }
                                    // } else {
                                    //     local_gen_face(x,y,z,CubeFaceDir::RIGHT);
                                    // }
                                    // if test_coord_in_chunk(x,y+1,z) {
                                    //     if !test_block_obscured(chunk.voxels[access(x,y,z-1)]) {
                                    //         local_gen_face(x,y,z,CubeFaceDir::BACK);
                                    //     }
                                    // } else {
                                    //     local_gen_face(x,y,z,CubeFaceDir::BACK);
                                    // }
                                    // if test_coord_in_chunk(x,y+1,z) {
                                    //     if !test_block_obscured(chunk.voxels[access(x-1,y,z)]) {
                                    //         local_gen_face(x,y,z,CubeFaceDir::LEFT);
                                    //     }
                                    // } else {
                                    //     local_gen_face(x,y,z,CubeFaceDir::LEFT);
                                    // }
                                    // if test_coord_in_chunk(x,y+1,z) {
                                    //     if !test_block_obscured(chunk.voxels[access(x,y+1,z)]) {
                                    //         local_gen_face(x,y,z,CubeFaceDir::TOP);
                                    //     }
                                    // } else {
                                    //     local_gen_face(x,y,z,CubeFaceDir::TOP);
                                    // }
                                    // if test_coord_in_chunk(x,y-1,z) {
                                    //     if !test_block_obscured(chunk.voxels[access(x,y-1,z)]) {
                                    //         local_gen_face(x,y,z,CubeFaceDir::BOTTOM);
                                    //     }
                                    // } else {
                                    //     local_gen_face(x,y,z,CubeFaceDir::BOTTOM);
                                    // }
                                }
                            }
                        }
                    }
                }
                (total_verts, total_inds)
            })
        );

        chunker.update(Length3D::default());

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
