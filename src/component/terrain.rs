use std::collections::HashMap;
use std::rc::Rc;
use std::mem;
use ash::{Device, vk};
use uom::si::f32::Length;
use crate::chunk_mesh::{Chunk, ChunkMesh, ChunkPosition};
use crate::component::{Component, ComponentEventResponse, RenderData, RenderDataPurpose};
use crate::component::camera::Length3D;
use crate::handler::VulkanInstance;
use crate::measurement::{blox, chux};
use crate::shader::chunk::ChunkVertex;
use crate::util::create_host_buffer;
use crate::world::{WorldEvent, WorldState};


#[derive(Copy, Clone)]
pub(crate) enum FaceDir {
    FRONT,
    RIGHT,
    BACK,
    LEFT,
    TOP,
    BOTTOM
}

fn gen_face(loc: (f32, f32, f32), ind_ofs: u32, face: FaceDir) -> (Vec<ChunkVertex>, Vec<u32>) {
    let (v, i) = match face {
        FaceDir::FRONT => {(
            vec![
                ChunkVertex { pos: [loc.0+0.0, loc.1+0.0, -loc.2+0.0], uv: [0.0, 1.0] },
                ChunkVertex { pos: [loc.0+1.0, loc.1+0.0, -loc.2+0.0], uv: [1.0, 1.0] },
                ChunkVertex { pos: [loc.0+0.0, loc.1+1.0, -loc.2+0.0], uv: [0.0, 0.0] },
                ChunkVertex { pos: [loc.0+1.0, loc.1+1.0, -loc.2+0.0], uv: [1.0, 0.0] },
            ],
            vec![0,1,2,3,2,1]
        )}
        FaceDir::RIGHT => {(
            vec![
                ChunkVertex { pos: [loc.0+1.0, loc.1+0.0, -loc.2+0.0], uv: [1.0, 1.0] },
                ChunkVertex { pos: [loc.0+1.0, loc.1+1.0, -loc.2+0.0], uv: [1.0, 0.0] },
                ChunkVertex { pos: [loc.0+1.0, loc.1+0.0, -loc.2-1.0], uv: [0.0, 1.0] },
                ChunkVertex { pos: [loc.0+1.0, loc.1+1.0, -loc.2-1.0], uv: [0.0, 0.0] },
            ],
            vec![0,2,1,3,1,2]
        )}
        FaceDir::BACK => {(
            vec![
                ChunkVertex { pos: [loc.0+0.0, loc.1+0.0, -loc.2-1.0], uv: [0.0, 1.0] },
                ChunkVertex { pos: [loc.0+1.0, loc.1+0.0, -loc.2-1.0], uv: [1.0, 1.0] },
                ChunkVertex { pos: [loc.0+0.0, loc.1+1.0, -loc.2-1.0], uv: [0.0, 0.0] },
                ChunkVertex { pos: [loc.0+1.0, loc.1+1.0, -loc.2-1.0], uv: [1.0, 0.0] },
            ],
            vec![1,0,3,2,3,0]
        )}
        FaceDir::LEFT => {(
            vec![
                ChunkVertex { pos: [loc.0+0.0, loc.1+0.0, -loc.2+0.0], uv: [1.0, 1.0] },
                ChunkVertex { pos: [loc.0+0.0, loc.1+1.0, -loc.2+0.0], uv: [1.0, 0.0] },
                ChunkVertex { pos: [loc.0+0.0, loc.1+0.0, -loc.2-1.0], uv: [0.0, 1.0] },
                ChunkVertex { pos: [loc.0+0.0, loc.1+1.0, -loc.2-1.0], uv: [0.0, 0.0] },
            ],
            vec![2,0,3,1,3,0]
        )}
        FaceDir::TOP => {(
            vec![
                ChunkVertex { pos: [loc.0+0.0, loc.1+1.0, -loc.2+0.0], uv: [1.0, 1.0] },
                ChunkVertex { pos: [loc.0+1.0, loc.1+1.0, -loc.2+0.0], uv: [0.0, 1.0] },
                ChunkVertex { pos: [loc.0+0.0, loc.1+1.0, -loc.2-1.0], uv: [1.0, 0.0] },
                ChunkVertex { pos: [loc.0+1.0, loc.1+1.0, -loc.2-1.0], uv: [0.0, 0.0] },
            ],
            vec![0,1,2,3,2,1]
        )}
        FaceDir::BOTTOM => {(
            vec![
                ChunkVertex { pos: [loc.0+0.0, loc.1+0.0, -loc.2+0.0], uv: [0.0, 1.0] },
                ChunkVertex { pos: [loc.0+1.0, loc.1+0.0, -loc.2+0.0], uv: [1.0, 1.0] },
                ChunkVertex { pos: [loc.0+0.0, loc.1+0.0, -loc.2-1.0], uv: [0.0, 0.0] },
                ChunkVertex { pos: [loc.0+1.0, loc.1+0.0, -loc.2-1.0], uv: [1.0, 0.0] },
            ],
            vec![1,0,3,2,3,0]
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
            2,
            Box::new(move |pos: Length3D, size: Length3D| {
                let coord = |i: f32| {
                    let y = (i/(chunk_size as f32*chunk_size as f32)).floor();
                    let x = ((i-y*chunk_size as f32*chunk_size as f32)/chunk_size as f32).floor();
                    let z = (i-y*chunk_size as f32*chunk_size as f32) % chunk_size as f32;
                    (x+pos.x.get::<blox>(), y+pos.y.get::<blox>(), z+pos.z.get::<blox>())
                };
                let mut voxel = (0..(size.x.get::<blox>()*size.y.get::<blox>()*size.z.get::<blox>()) as u32)
                    .into_iter()
                    .map(|i| {
                        let (x,y,z) = coord(i as f32);

                        if y > (x/20.0).sin()*10.0+(z/20.0).sin()*10.0  {
                            BlockGen::Empty
                        } else {
                            BlockGen::Opaque(Block(0))
                        }
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
            Box::new(move |chunks: &HashMap<ChunkPosition, Chunk<BlockGen>>| {
                let mut total_verts = vec![];
                let mut total_inds = vec![];
                let mut faces = 0;

                for chunk in chunks.values() {
                    let mut local_gen_face = |x: u32, y: u32, z: u32, cube_face_dir: FaceDir| {
                        let (mut verts, mut inds) = gen_face(
                            (chunk.pos.x.get::<blox>()+x as f32,
                             chunk.pos.y.get::<blox>()+y as f32,
                             -chunk.pos.z.get::<blox>()-z as f32),
                            faces*4,
                            cube_face_dir,
                        );
                        total_verts.append(&mut verts);
                        total_inds.append(&mut inds);
                        faces += 1;
                    };

                    let mut cull_border_face = |x, y, z, face_dir: FaceDir| {
                        match face_dir {
                            FaceDir::FRONT => {
                                if let Some(ref hpos) = chunk.adjacency.front {
                                    let adj_block = chunks.get(hpos).unwrap().voxels[access(x,y,0)];
                                    test_block_obscured(adj_block)
                                } else { true }
                                // if theres no chunk, then it probably means the player can't see it anyways
                                // no need to render the whole face at the border
                            }
                            FaceDir::RIGHT => {
                                if let Some(ref hpos) = chunk.adjacency.right {
                                    let adj_block = chunks.get(hpos).unwrap().voxels[access(0,y,z)];
                                    test_block_obscured(adj_block)
                                } else { true }
                            }
                            FaceDir::BACK => {
                                if let Some(ref hpos) = chunk.adjacency.back {
                                    let adj_block = chunks.get(hpos).unwrap().voxels[access(x,y,chunk_size-1)];
                                    test_block_obscured(adj_block)
                                } else { true }
                            }
                            FaceDir::LEFT => {
                                if let Some(ref hpos) = chunk.adjacency.left {
                                    let adj_block = chunks.get(hpos).unwrap().voxels[access(chunk_size-1,y,z)];
                                    test_block_obscured(adj_block)
                                } else { true }
                            }
                            FaceDir::TOP => {
                                if let Some(ref hpos) = chunk.adjacency.top {
                                    let adj_block = chunks.get(hpos).unwrap().voxels[access(x,0,z)];
                                    test_block_obscured(adj_block)
                                } else { true }
                            }
                            FaceDir::BOTTOM => {
                                if let Some(ref hpos) = chunk.adjacency.bottom {
                                    let adj_block = chunks.get(hpos).unwrap().voxels[access(x,chunk_size-1,z)];
                                    test_block_obscured(adj_block)
                                } else { true }
                            }
                        }
                    };

                    for x in 0..chunk_size {
                        for y in 0..chunk_size {
                            for z in 0..chunk_size {
                                let mut local_checked_gen_face = |dx, dy, dz, face_dir| {
                                    if (dx == -1 && x == 0) || (dy == -1 && y == 0) || (dz == 1 && z == chunk_size-1) {
                                        // if delta face coord is not in chunk (other side)
                                        // border_culling_face(x,y,z,face_dir);  // TODO: border culling
                                        if !cull_border_face(x, y, z, face_dir) {
                                            local_gen_face(x,y,z,face_dir);
                                        }
                                    } else if (dx == 1 && x == chunk_size-1) || (dy == 1 && y == chunk_size-1) || (dz == -1 && z == 0) {
                                        if !cull_border_face(x, y, z, face_dir) {
                                            local_gen_face(x,y,z,face_dir);
                                        }
                                    } else if test_coord_in_chunk((x as i32+dx) as u32,(y as i32+dy) as u32,(z as i32+dz) as u32) {
                                        if !test_block_obscured(chunk.voxels[access((x as i32+dx) as u32,(y as i32+dy) as u32,(z as i32+dz) as u32)]) {
                                            // if delta face coord is in chunk and not obscured
                                            local_gen_face(x,y,z,face_dir);
                                        }
                                    } else {
                                        // if delta face coord is not in chunk (other side)
                                        // border_culling_face(x,y,z,face_dir);  // TODO: border culling
                                        // if !cull_border_face(x, y, z, face_dir) {
                                        //     local_gen_face(x,y,z,face_dir);
                                        // }
                                    }
                                };

                                // if y == chunk_size-1 {
                                //     if let Some(pos) = chunk.adjacency.top {
                                //         if let Some(adj_chunk) = chunks.get(&pos) {
                                //             if test_block_obscured(adj_chunk.voxels[access(x,y,z)]) {
                                //
                                //             }
                                //         } else {
                                //             println!("WARNING: CHUNK ADJACENCY REFERS TO NON-EXISTENT CHUNK");
                                //         }
                                //     }
                                // } else if y == 0 {
                                //     if let Some(pos) = chunk.adjacency.bottom {
                                //         if let Some(adj_chunk) = chunks.get(&pos) {
                                //             if test_block_obscured(adj_chunk.voxels[access(x,y,z)]) {
                                //
                                //             }
                                //         } else {
                                //             println!("WARNING: CHUNK ADJACENCY REFERS TO NON-EXISTENT CHUNK");
                                //         }
                                //     }
                                // } else if let BlockGen::Opaque(block) = &chunk.voxels[access(x,y,z)] {
                                //
                                // }

                                if let BlockGen::Opaque(block) = &chunk.voxels[access(x,y,z)] {
                                    local_checked_gen_face(0, 0, 1, FaceDir::FRONT);
                                    local_checked_gen_face(1, 0, 0, FaceDir::RIGHT);
                                    local_checked_gen_face(0, 0, -1, FaceDir::BACK);
                                    local_checked_gen_face(-1, 0, 0, FaceDir::LEFT);
                                    local_checked_gen_face(0, 1, 0, FaceDir::TOP);
                                    local_checked_gen_face(0, -1, 0, FaceDir::BOTTOM);
                                }
                            }
                        }
                    }
                }

                // for chunk in chunks {
                //     let mut local_gen_face = |x: u32, y: u32, z: u32, cube_face_dir: FaceDir| {
                //         let (mut verts, mut inds) = gen_face(
                //             (chunk.pos.x.get::<blox>()+x as f32,
                //              chunk.pos.y.get::<blox>()+y as f32,
                //              -chunk.pos.z.get::<blox>()-z as f32),
                //             faces*4,
                //             cube_face_dir,
                //         );
                //         total_verts.append(&mut verts);
                //         total_inds.append(&mut inds);
                //         faces += 1;
                //     };
                //
                //     for x in 0..chunk_size {
                //         for y in 0..chunk_size {
                //             for z in 0..chunk_size {
                //                 if let BlockGen::Opaque(block) = &chunk.voxels[access(x,y,z)] {
                //                     let mut local_checked_gen_face = |dx, dy, dz, face_dir| {
                //                         if (dx == -1 && x == 0) || (dy == -1 && y == 0) || (dz == -1 && z == 0) {
                //                             // if delta face coord is not in chunk (other side)
                //                             local_gen_face(x,y,z,face_dir);
                //                             return;
                //                         }
                //                         if test_coord_in_chunk((x as i32+dx) as u32,(y as i32+dy) as u32,(z as i32+dz) as u32) {
                //                             if !test_block_obscured(chunk.voxels[access((x as i32+dx) as u32,(y as i32+dy) as u32,(z as i32+dz) as u32)]) {
                //                                 // if delta face coord is in chunk and not obscured
                //                                 local_gen_face(x,y,z,face_dir);
                //                             }
                //                         } else {
                //                             // if delta face coord is not in chunk
                //                             local_gen_face(x,y,z,face_dir);
                //                         }
                //                     };
                //
                //                     local_checked_gen_face(0, 0, -1, FaceDir::FRONT);
                //                     local_checked_gen_face(1, 0, 0, FaceDir::RIGHT);
                //                     local_checked_gen_face(0, 0, 1, FaceDir::BACK);
                //                     local_checked_gen_face(-1, 0, 0, FaceDir::LEFT);
                //                     local_checked_gen_face(0, 1, 0, FaceDir::TOP);
                //                     local_checked_gen_face(0, -1, 0, FaceDir::BOTTOM);
                //                 }
                //             }
                //         }
                //     }
                // }
                (total_verts, total_inds)
            })
        );

        chunker.initialize();

        Self {
            vi,
            device,
            chunk_mesh: chunker,
            to_render: vec![],
            chunk_update: true,
        }
    }
}

impl Component for Terrain {
    fn render(&self) -> Vec<RenderData> {
        // println!("RENDER() {}", self.to_render.len());
        self.to_render.clone()
    }

    fn respond_event(&mut self, event: WorldEvent) -> ComponentEventResponse {
        match event {
            WorldEvent::UserPosition(pos) => {
                self.chunk_update = self.chunk_update || self.chunk_mesh.update(pos);
            }
            _ => {}
        }

        ComponentEventResponse::default()
    }

    fn update_state(&mut self, state: &mut WorldState) {
        self.to_render.clear();
        if self.chunk_update {
            let (total_verts, total_inds) = self.chunk_mesh.generate_vertices();

            let (vertex_buffer, vertex_buffer_mem, _, _) = unsafe {
                create_host_buffer(self.vi.clone(), self.device.clone(), &total_verts, vk::BufferUsageFlags::VERTEX_BUFFER, true)
            };
            let (index_buffer, index_buffer_mem, _, _) = unsafe {
                create_host_buffer(self.vi.clone(), self.device.clone(), &total_inds, vk::BufferUsageFlags::INDEX_BUFFER, true)
            };

            self.to_render.push(RenderData::RecreateVertexBuffer(
                vertex_buffer, vertex_buffer_mem, RenderDataPurpose::TerrainVertices
            ));
            self.to_render.push(RenderData::RecreateIndexBuffer(
                index_buffer, index_buffer_mem, total_inds.len() as u32, RenderDataPurpose::TerrainVertices
            ));
            self.chunk_update = false;
        }
    }
}
