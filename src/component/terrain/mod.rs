mod block_gen;
// mod mesh_gen;

use std::collections::HashMap;
use std::rc::Rc;
use std::mem;
use ash::{Device, vk};
use noise::{NoiseFn, Perlin};
use uom::si::f32::Length;
use crate::chunk_mesh::{Chunk, ChunkMesh, ChunkPosition};
use crate::component::{Component, RenderData, RenderDataPurpose};
use crate::component::camera::Length3D;
use crate::component::terrain::block_gen::BlockGenerator;
use crate::component::texture::TextureIDMapper;
use crate::handler::VulkanInstance;
use crate::measurement::{blox, chux};
use crate::shader::chunk::ChunkVertex;
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
pub enum TextureMapper<'s> {
    All(&'s str),
    Lateral(&'s str, &'s str, &'s str),  // top, bottom, lateral
    Unique(&'s str, &'s str, &'s str, &'s str, &'s str, &'s str),  // top, bottom, E (right), S (front), W (left), N (back)
}

impl<'s> TextureMapper<'s> {
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
    ident: &'s str,
    texture_id: TextureMapper<'s>,
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

impl Terrain<'_> {
    pub(crate) fn new(vi: Rc<VulkanInstance>, device: Rc<Device>) -> Self {
        let block_ind = Vec::from(&[
            BlockData {
                ident: "grass",
                texture_id: TextureMapper::Lateral("grass_top", "dirt", "grass_side"),
            },
        ]);

        let chunk_size = Length::new::<chux>(1.0).get::<blox>() as u32;
        // let access = move |x,y,z| (y*chunk_size*chunk_size+x*chunk_size+z) as usize;
        // let test_block_obscured = |block| {
        //     mem::discriminant(&block) == mem::discriminant(&BlockGen::Opaque(Block::default())) ||
        //         mem::discriminant(&block) == mem::discriminant(&BlockGen::Obscured)
        // };
        // let test_coord_in_chunk = move |x: u32, y: u32, z: u32| {
        //     0 <= x && x < chunk_size && 0 <= y && y < chunk_size && 0 <= z && z < chunk_size
        // };
        // terrain generation TODO: (no side effect for future optimization)
        let noise = Perlin::new(50);

        // let mut chunker = ChunkMesh::new(
        //     Length3D {
        //         x: Length::new::<blox>(0.0),
        //         y: Length::new::<blox>(0.0),
        //         z: Length::new::<blox>(0.0),
        //     },
        //     Length3D {
        //         x: Length::new::<chux>(1.0),
        //         y: Length::new::<chux>(1.0),
        //         z: Length::new::<chux>(1.0),
        //     },
        //     2,
        //     // Box::new(move |pos: Length3D, size: Length3D| {
        //     //
        //     // }),
        //     // Box::new(move |chunks: &HashMap<ChunkPosition, Chunk<BlockGen>>| {
        //     //
        //     // })
        //     Box::new(Self::terrain_generator),
        //     Box::new(Self::mesh_generator)
        // );

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

    // pub(crate) fn terrain_generator(&self, pos: Length3D, size: Length3D) -> Box<[BlockCullType]> {
    //     let coord = |i: f32| {
    //         let y = (i/(self.chunk_size as f32*self.chunk_size as f32)).floor();
    //         let x = ((i-y*self.chunk_size as f32*self.chunk_size as f32)/self.chunk_size as f32).floor();
    //         let z = (i-y*self.chunk_size as f32*self.chunk_size as f32) % self.chunk_size as f32;
    //         (x+pos.x.get::<blox>(), y+pos.y.get::<blox>(), z+pos.z.get::<blox>())
    //     };
    //     let mut voxel = (0..(size.x.get::<blox>()*size.y.get::<blox>()*size.z.get::<blox>()) as u32)
    //         .into_iter()
    //         .map(|i| {
    //             let (x,y,z) = coord(i as f32);
    //
    //             if y > (x/20.0).sin()*10.0+(z/20.0).sin()*10.0  {
    //                 BlockCullType::Empty
    //             } else {
    //                 BlockCullType::Opaque(Block(0))
    //             }
    //             // if y as f64 > noise.get([x, z])  {
    //             //     BlockGen::Empty
    //             // } else {
    //             //     BlockGen::Opaque(Block(0))
    //             // }
    //         })
    //         .collect::<Box<[BlockCullType]>>();
    //
    //     for x in 1..size.x.get::<blox>() as u32-1 {
    //         for y in 1..size.y.get::<blox>() as u32-1 {
    //             for z in 1..size.z.get::<blox>() as u32-1 {
    //                 if  Self::check_block_obscured(voxel[self.access(x+1,y,z)]) &&
    //                     Self::check_block_obscured(voxel[self.access(x-1,y,z)]) &&
    //                     Self::check_block_obscured(voxel[self.access(x,y+1,z)]) &&
    //                     Self::check_block_obscured(voxel[self.access(x,y-1,z)]) &&
    //                     Self::check_block_obscured(voxel[self.access(x,y,z+1)]) &&
    //                     Self::check_block_obscured(voxel[self.access(x,y,z-1)]) {
    //                     voxel[self.access(x,y,z)] = BlockCullType::Obscured;
    //                 }
    //             }
    //         }
    //     }
    //
    //     voxel
    // }
    //
    // pub(crate) fn mesh_generator(&self, chunks: &HashMap<ChunkPosition, Chunk<BlockCullType>>) -> (Vec<ChunkVertex>, Vec<u32>) {
    //     let mut total_verts = vec![];
    //     let mut total_inds = vec![];
    //     let mut faces = 0;
    //
    //     for chunk in chunks.values() {
    //         let mut local_gen_face = |x: u32, y: u32, z: u32, cube_face_dir: FaceDir, txtr_mapping: TextureMapper, txtr_id_mapping: &TextureIDMapper| {
    //             let (mut verts, mut inds) = gen_face(
    //                 (chunk.pos.x.get::<blox>()+x as f32,
    //                  chunk.pos.y.get::<blox>()+y as f32,
    //                  -chunk.pos.z.get::<blox>()-z as f32),
    //                 faces*4,
    //                 cube_face_dir,
    //                 txtr_mapping,
    //                 txtr_id_mapping,
    //             );
    //             total_verts.append(&mut verts);
    //             total_inds.append(&mut inds);
    //             faces += 1;
    //         };
    //
    //         let cull_border_face = |x, y, z, face_dir: FaceDir| {
    //             match face_dir {
    //                 FaceDir::FRONT => {
    //                     if let Some(ref hpos) = chunk.adjacency.front {
    //                         let adj_block = chunks.get(hpos).unwrap().voxels[self.access(x,y,0)];
    //                         Self::check_block_obscured(adj_block)
    //                     } else { true }
    //                     // if theres no chunk, then it probably means the player can't see it anyways
    //                     // no need to render the whole face at the border
    //                 }
    //                 FaceDir::RIGHT => {
    //                     if let Some(ref hpos) = chunk.adjacency.right {
    //                         let adj_block = chunks.get(hpos).unwrap().voxels[self.access(0,y,z)];
    //                         Self::check_block_obscured(adj_block)
    //                     } else { true }
    //                 }
    //                 FaceDir::BACK => {
    //                     if let Some(ref hpos) = chunk.adjacency.back {
    //                         let adj_block = chunks.get(hpos).unwrap().voxels[self.access(x,y,self.chunk_size-1)];
    //                         Self::check_block_obscured(adj_block)
    //                     } else { true }
    //                 }
    //                 FaceDir::LEFT => {
    //                     if let Some(ref hpos) = chunk.adjacency.left {
    //                         let adj_block = chunks.get(hpos).unwrap().voxels[self.access(self.chunk_size-1,y,z)];
    //                         Self::check_block_obscured(adj_block)
    //                     } else { true }
    //                 }
    //                 FaceDir::TOP => {
    //                     if let Some(ref hpos) = chunk.adjacency.top {
    //                         let adj_block = chunks.get(hpos).unwrap().voxels[self.access(x,0,z)];
    //                         Self::check_block_obscured(adj_block)
    //                     } else { true }
    //                 }
    //                 FaceDir::BOTTOM => {
    //                     if let Some(ref hpos) = chunk.adjacency.bottom {
    //                         let adj_block = chunks.get(hpos).unwrap().voxels[self.access(x,self.chunk_size-1,z)];
    //                         Self::check_block_obscured(adj_block)
    //                     } else { true }
    //                 }
    //             }
    //         };
    //
    //         for x in 0..self.chunk_size {
    //             for y in 0..self.chunk_size {
    //                 for z in 0..self.chunk_size {
    //                     let mut local_checked_gen_face = |dx, dy, dz, face_dir, txtr_mapping| {
    //                         if (dx == -1 && x == 0) || (dy == -1 && y == 0) || (dz == 1 && z == self.chunk_size-1) {
    //                             if !cull_border_face(x, y, z, face_dir) {
    //                                 local_gen_face(x,y,z,face_dir,txtr_mapping,self.txtr_mapper.as_ref().unwrap());
    //                             }
    //                         } else if (dx == 1 && x == self.chunk_size-1) || (dy == 1 && y == self.chunk_size-1) || (dz == -1 && z == 0) {
    //                             if !cull_border_face(x, y, z, face_dir) {
    //                                 local_gen_face(x,y,z,face_dir,txtr_mapping,self.txtr_mapper.as_ref().unwrap());
    //                             }
    //                         } else if self.check_coord_within_chunk((x as i32+dx) as u32,(y as i32+dy) as u32,(z as i32+dz) as u32) {
    //                             if !Self::check_block_obscured(chunk.voxels[self.access((x as i32+dx) as u32,(y as i32+dy) as u32,(z as i32+dz) as u32)]) {
    //                                 // if delta face coord is in chunk and not obscured
    //                                 local_gen_face(x,y,z,face_dir,txtr_mapping,self.txtr_mapper.as_ref().unwrap());
    //                             }
    //                         }
    //                     };
    //
    //                     if let BlockCullType::Opaque(block) = &chunk.voxels[self.access(x, y, z)] {
    //                         let txtr = self.block_ind[block.0 as usize].texture_id;
    //
    //                         local_checked_gen_face(0, 0, 1, FaceDir::FRONT, txtr);
    //                         local_checked_gen_face(1, 0, 0, FaceDir::RIGHT, txtr);
    //                         local_checked_gen_face(0, 0, -1, FaceDir::BACK, txtr);
    //                         local_checked_gen_face(-1, 0, 0, FaceDir::LEFT, txtr);
    //                         local_checked_gen_face(0, 1, 0, FaceDir::TOP, txtr);
    //                         local_checked_gen_face(0, -1, 0, FaceDir::BOTTOM, txtr);
    //                     }
    //                 }
    //             }
    //         }
    //     }
    //     (total_verts, total_inds)
    // }

    // fn access(&self, x: u32, y: u32, z: u32) -> usize {
    //     (y*self.chunk_size*self.chunk_size+x*self.chunk_size+z) as usize
    // }
    //
    // fn check_block_obscured(block: BlockCullType) -> bool {
    //     mem::discriminant(&block) == mem::discriminant(&BlockCullType::Opaque(Block::default())) ||
    //         mem::discriminant(&block) == mem::discriminant(&BlockCullType::Obscured)
    // }
    //
    // fn check_coord_within_chunk(&self, x: u32, y: u32, z: u32) -> bool {
    //     0 <= x && x < self.chunk_size && 0 <= y && y < self.chunk_size && 0 <= z && z < self.chunk_size
    // }
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
                    // Box::new(move |pos: Length3D, size: Length3D| {
                    //
                    // }),
                    // Box::new(move |chunks: &HashMap<ChunkPosition, Chunk<BlockGen>>| {
                    //
                    // })
                    // Box::new({
                    //     let terrain = self.clone();
                    //     |pos, size| {
                    //         terrain.terrain_generator(pos, size)  // TODO: create a separate trait interfaced generator struct
                    //     }
                    // }),
                    // Box::new({
                    //     let terrain = self.clone();
                    //     |chunks| {
                    //         terrain.mesh_generator(chunks)
                    //     }
                    // })
                    block_generator,
                );

                chunk_mesher.initialize();

                // self.chunk_update = true
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
                let (total_verts, total_inds) = chunk_mesh.generate_vertices();

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
}
