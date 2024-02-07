use std::collections::HashMap;
use std::mem;
use noise::{NoiseFn, Perlin};
use crate::chunk_mesh::{Chunk, ChunkPosition, ChunkGeneratable};
use crate::component::camera::Length3D;
use crate::component::RenderDataPurpose;
use crate::component::terrain::{Block, BlockCullType, BlockData, FaceDir, MeshType, TextureMapper};
use crate::component::texture::TextureIDMapper;
use crate::measurement::blox;
use crate::shader::chunk::ChunkVertex;

pub(super) struct BlockGenerator<'b> {
    chunk_size: u32,
    block_ind: Vec<BlockData<'b>>,
    txtr_id_mapper: TextureIDMapper,
    noise: Perlin,
    floral_noise: Perlin,
}

impl<'b> BlockGenerator<'b> {
    pub(crate) fn new(chunk_size: u32, block_ind: Vec<BlockData<'b>>, txtr_id_mapper: TextureIDMapper,) -> Self {
        Self {
            chunk_size, block_ind, txtr_id_mapper, noise: Perlin::new(50), floral_noise: Perlin::new(23),
        }
    }

    fn access(&self, x: u32, y: u32, z: u32) -> usize {
        (y*self.chunk_size*self.chunk_size+x*self.chunk_size+z) as usize
    }

    fn check_block_obscured(block: BlockCullType) -> bool {
        mem::discriminant(&block) == mem::discriminant(&BlockCullType::Opaque(Block::default())) ||
            mem::discriminant(&block) == mem::discriminant(&BlockCullType::Obscured)
    }

    fn check_coord_within_chunk(&self, x: u32, y: u32, z: u32) -> bool {
        0 <= x && x < self.chunk_size && 0 <= y && y < self.chunk_size && 0 <= z && z < self.chunk_size
    }

    fn gen_face(&self, loc: (f32, f32, f32), ind_ofs: u32, face: FaceDir, txtr_mapping: TextureMapper) -> (Vec<ChunkVertex>, Vec<u32>) {
        let txtr_mapper = |name: &str| *self.txtr_id_mapper.get(name).unwrap_or(&0) as f32;

        let (v, i) = match face {
            FaceDir::FRONT => {
                let txtr = txtr_mapper(txtr_mapping.front());

                (
                    vec![
                        ChunkVertex { pos: [loc.0+1.0, loc.1+0.0, -loc.2+0.0], uv: [1.0, 1.0], txtr },
                        ChunkVertex { pos: [loc.0+0.0, loc.1+1.0, -loc.2+0.0], uv: [0.0, 0.0], txtr },
                        ChunkVertex { pos: [loc.0+0.0, loc.1+0.0, -loc.2+0.0], uv: [0.0, 1.0], txtr },
                        ChunkVertex { pos: [loc.0+1.0, loc.1+1.0, -loc.2+0.0], uv: [1.0, 0.0], txtr },
                    ],
                    vec![0,1,2,3,1,0]
                )
            }
            FaceDir::RIGHT => {
                let txtr = txtr_mapper(txtr_mapping.right());

                (
                    vec![
                        ChunkVertex { pos: [loc.0+1.0, loc.1+0.0, -loc.2+0.0], uv: [1.0, 1.0], txtr },
                        ChunkVertex { pos: [loc.0+1.0, loc.1+1.0, -loc.2+0.0], uv: [1.0, 0.0], txtr },
                        ChunkVertex { pos: [loc.0+1.0, loc.1+0.0, -loc.2-1.0], uv: [0.0, 1.0], txtr },
                        ChunkVertex { pos: [loc.0+1.0, loc.1+1.0, -loc.2-1.0], uv: [0.0, 0.0], txtr },
                    ],
                    vec![0,2,1,3,1,2]
                )}
            FaceDir::BACK => {
                let txtr = txtr_mapper(txtr_mapping.back());

                (
                    vec![
                        ChunkVertex { pos: [loc.0+0.0, loc.1+0.0, -loc.2-1.0], uv: [0.0, 1.0], txtr },
                        ChunkVertex { pos: [loc.0+1.0, loc.1+0.0, -loc.2-1.0], uv: [1.0, 1.0], txtr },
                        ChunkVertex { pos: [loc.0+0.0, loc.1+1.0, -loc.2-1.0], uv: [0.0, 0.0], txtr },
                        ChunkVertex { pos: [loc.0+1.0, loc.1+1.0, -loc.2-1.0], uv: [1.0, 0.0], txtr },
                    ],
                    vec![1,0,3,2,3,0]
                )}
            FaceDir::LEFT => {
                let txtr = txtr_mapper(txtr_mapping.left());

                (
                    vec![
                        ChunkVertex { pos: [loc.0+0.0, loc.1+0.0, -loc.2+0.0], uv: [1.0, 1.0], txtr },
                        ChunkVertex { pos: [loc.0+0.0, loc.1+1.0, -loc.2+0.0], uv: [1.0, 0.0], txtr },
                        ChunkVertex { pos: [loc.0+0.0, loc.1+0.0, -loc.2-1.0], uv: [0.0, 1.0], txtr },
                        ChunkVertex { pos: [loc.0+0.0, loc.1+1.0, -loc.2-1.0], uv: [0.0, 0.0], txtr },
                    ],
                    vec![2,0,3,1,3,0]
                )}
            FaceDir::TOP => {
                let txtr = txtr_mapper(txtr_mapping.top());

                (
                    vec![
                        ChunkVertex { pos: [loc.0+0.0, loc.1+1.0, -loc.2+0.0], uv: [1.0, 1.0], txtr },
                        ChunkVertex { pos: [loc.0+1.0, loc.1+1.0, -loc.2+0.0], uv: [0.0, 1.0], txtr },
                        ChunkVertex { pos: [loc.0+0.0, loc.1+1.0, -loc.2-1.0], uv: [1.0, 0.0], txtr },
                        ChunkVertex { pos: [loc.0+1.0, loc.1+1.0, -loc.2-1.0], uv: [0.0, 0.0], txtr },
                    ],
                    vec![0,1,2,3,2,1]
                )}
            FaceDir::BOTTOM => {
                let txtr = txtr_mapper(txtr_mapping.bottom());

                (
                    vec![
                        ChunkVertex { pos: [loc.0+0.0, loc.1+0.0, -loc.2+0.0], uv: [0.0, 1.0], txtr },
                        ChunkVertex { pos: [loc.0+1.0, loc.1+0.0, -loc.2+0.0], uv: [1.0, 1.0], txtr },
                        ChunkVertex { pos: [loc.0+0.0, loc.1+0.0, -loc.2-1.0], uv: [0.0, 0.0], txtr },
                        ChunkVertex { pos: [loc.0+1.0, loc.1+0.0, -loc.2-1.0], uv: [1.0, 0.0], txtr },
                    ],
                    vec![1,0,3,2,3,0]
                )}
        };
        let i = i.into_iter()
            .map(|ind| ind+ind_ofs)
            .collect();
        (v,i)
    }

    fn gen_xcross(&self, loc: (f32, f32, f32), ind_ofs: u32, txtr_mapping: TextureMapper) -> (Vec<ChunkVertex>, Vec<u32>) {
        let txtr_mapper = |name: &str| *self.txtr_id_mapper.get(name).unwrap_or(&0) as f32;
        let txtr = txtr_mapper(txtr_mapping.default());

        let v = [
            // -x +z to +x -z
            ChunkVertex { pos: [loc.0+0.0, loc.1+0.0, -loc.2+0.0], uv: [1.0, 1.0], txtr },
            ChunkVertex { pos: [loc.0+0.0, loc.1+1.0, -loc.2+0.0], uv: [1.0, 0.0], txtr },
            ChunkVertex { pos: [loc.0+1.0, loc.1+0.0, -loc.2-1.0], uv: [0.0, 1.0], txtr },
            ChunkVertex { pos: [loc.0+1.0, loc.1+1.0, -loc.2-1.0], uv: [0.0, 0.0], txtr },

            // +x +z to -x -z
            ChunkVertex { pos: [loc.0+1.0, loc.1+0.0, -loc.2+0.0], uv: [1.0, 1.0], txtr },
            ChunkVertex { pos: [loc.0+1.0, loc.1+1.0, -loc.2+0.0], uv: [1.0, 0.0], txtr },
            ChunkVertex { pos: [loc.0+0.0, loc.1+0.0, -loc.2-1.0], uv: [0.0, 1.0], txtr },
            ChunkVertex { pos: [loc.0+0.0, loc.1+1.0, -loc.2-1.0], uv: [0.0, 0.0], txtr },
        ];
        let i = [
            0,1,2,2,1,3,
            4,5,6,6,5,7,
        ];

        let i = i.into_iter()
            .map(|ind| ind+ind_ofs)
            .collect();
        (v.to_vec(),i)
    }
}

impl ChunkGeneratable for BlockGenerator<'_> {
    type P = BlockCullType;
    type V = ChunkVertex;
    type I = u32;

    fn generate_chunk(&self, pos: Length3D) -> Box<[Self::P]> {
        let coord = |i: f32| {
            let y = (i/(self.chunk_size as f32*self.chunk_size as f32)).floor();
            let x = ((i-y*self.chunk_size as f32*self.chunk_size as f32)/self.chunk_size as f32).floor();
            let z = (i-y*self.chunk_size as f32*self.chunk_size as f32) % self.chunk_size as f32;
            (x as f64+pos.x.get::<blox>() as f64, y as f64+pos.y.get::<blox>() as f64, z as f64+pos.z.get::<blox>() as f64)
        };
        let mut voxel = (0..self.chunk_size*self.chunk_size*self.chunk_size)
            .into_iter()
            .map(|i| {
                let (x,y,z) = coord(i as f32);
                // TERRAIN GENERATION (NO SIDE EFFECT)

                let base_level = self.noise.get([x/20.0, z/20.0])*20.0+20.0;
                let floralness = self.floral_noise.get([x/40.0, z/40.0]);
                let sea_level = 10.0;

                if y > base_level+1.0 {
                    BlockCullType::Empty
                } else if y > base_level {
                    if 0.8 < floralness && floralness < 0.9 {
                        BlockCullType::Transparent(Block(4))
                    } else {
                        BlockCullType::Empty
                    }
                } else if y < sea_level {
                    BlockCullType::Opaque(Block(3))
                } else if y > base_level-1.0 {
                    BlockCullType::Opaque(Block(0))
                } else if y > base_level-3.0 {
                    BlockCullType::Opaque(Block(1))
                } else {
                    BlockCullType::Opaque(Block(2))
                }

                // if y > (x/20.0).sin()*10.0+(z/20.0).sin()*10.0  {
                //     BlockCullType::Empty
                // } else {
                //     BlockCullType::Opaque(Block(0))
                // }
                // if y as f64 > noise.get([x, z])  {
                //     BlockGen::Empty
                // } else {
                //     BlockGen::Opaque(Block(0))
                // }
            })
            .collect::<Box<[BlockCullType]>>();

        for x in 1..self.chunk_size-1 {
            for y in 1..self.chunk_size-1 {
                for z in 1..self.chunk_size-1 {
                    if  Self::check_block_obscured(voxel[self.access(x+1,y,z)]) &&
                        Self::check_block_obscured(voxel[self.access(x-1,y,z)]) &&
                        Self::check_block_obscured(voxel[self.access(x,y+1,z)]) &&
                        Self::check_block_obscured(voxel[self.access(x,y-1,z)]) &&
                        Self::check_block_obscured(voxel[self.access(x,y,z+1)]) &&
                        Self::check_block_obscured(voxel[self.access(x,y,z-1)]) {
                        voxel[self.access(x,y,z)] = BlockCullType::Obscured;
                    }
                }
            }
        }

        voxel
    }

    fn generate_mesh(&self, chunks: &HashMap<ChunkPosition, Chunk<Self::P>>) -> Vec<(Vec<Self::V>, Vec<Self::I>, RenderDataPurpose)> {
        let mut total_verts = vec![];
        let mut total_inds = vec![];
        let mut faces = 0;
        let mut transparent_total_verts = vec![];
        let mut transparent_total_inds = vec![];
        let mut transparent_faces = 0;

        for chunk in chunks.values() {
            let chunk_pos = |x: u32, y: u32, z: u32| (
                chunk.pos.x.get::<blox>()+x as f32,
                chunk.pos.y.get::<blox>()+y as f32,
                -chunk.pos.z.get::<blox>()-z as f32
            );

            // let mut local_gen_xcross = |face_count: u32, x: u32, y: u32, z: u32, txtr_mapping: TextureMapper| {
            //     let (mut verts, mut inds) = self.gen_xcross(
            //         (chunk.pos.x.get::<blox>()+x as f32,
            //          chunk.pos.y.get::<blox>()+y as f32,
            //          -chunk.pos.z.get::<blox>()-z as f32),
            //         face_count*4,
            //         txtr_mapping,
            //     );
            //     // total_verts.append(&mut verts);
            //     // total_inds.append(&mut inds);
            //     // faces += 2;
            //
            //     (verts, inds, 2)
            // };
            //
            // let mut local_gen_face = |face_count: u32, x: u32, y: u32, z: u32, cube_face_dir: FaceDir, txtr_mapping: TextureMapper| {
            //     let (mut verts, mut inds) = self.gen_face(
            //         (chunk.pos.x.get::<blox>()+x as f32,
            //          chunk.pos.y.get::<blox>()+y as f32,
            //          -chunk.pos.z.get::<blox>()-z as f32),
            //         faces*4,
            //         cube_face_dir,
            //         txtr_mapping,
            //     );
            //     // total_verts.append(&mut verts);
            //     // total_inds.append(&mut inds);
            //     // faces += 1;
            //
            //     (verts, inds, 1)
            // };

            let cull_border_face = |x, y, z, face_dir: FaceDir| {
                match face_dir {
                    FaceDir::FRONT => {
                        if let Some(ref hpos) = chunk.adjacency.front {
                            let adj_block = chunks.get(hpos).unwrap().voxels[self.access(x,y,0)];
                            Self::check_block_obscured(adj_block)
                        } else { true }
                        // if theres no chunk, then it probably means the player can't see it anyways
                        // no need to render the whole face at the border
                    }
                    FaceDir::RIGHT => {
                        if let Some(ref hpos) = chunk.adjacency.right {
                            let adj_block = chunks.get(hpos).unwrap().voxels[self.access(0,y,z)];
                            Self::check_block_obscured(adj_block)
                        } else { true }
                    }
                    FaceDir::BACK => {
                        if let Some(ref hpos) = chunk.adjacency.back {
                            let adj_block = chunks.get(hpos).unwrap().voxels[self.access(x,y,self.chunk_size-1)];
                            Self::check_block_obscured(adj_block)
                        } else { true }
                    }
                    FaceDir::LEFT => {
                        if let Some(ref hpos) = chunk.adjacency.left {
                            let adj_block = chunks.get(hpos).unwrap().voxels[self.access(self.chunk_size-1,y,z)];
                            Self::check_block_obscured(adj_block)
                        } else { true }
                    }
                    FaceDir::TOP => {
                        if let Some(ref hpos) = chunk.adjacency.top {
                            let adj_block = chunks.get(hpos).unwrap().voxels[self.access(x,0,z)];
                            Self::check_block_obscured(adj_block)
                        } else { true }
                    }
                    FaceDir::BOTTOM => {
                        if let Some(ref hpos) = chunk.adjacency.bottom {
                            let adj_block = chunks.get(hpos).unwrap().voxels[self.access(x,self.chunk_size-1,z)];
                            Self::check_block_obscured(adj_block)
                        } else { true }
                    }
                }
            };

            for x in 0..self.chunk_size {
                for y in 0..self.chunk_size {
                    for z in 0..self.chunk_size {
                        let mut local_checked_gen_face = |dx, dy, dz, face_dir, txtr_mapping| {
                            if (dx == -1 && x == 0) || (dy == -1 && y == 0) || (dz == 1 && z == self.chunk_size-1) {
                                if !cull_border_face(x, y, z, face_dir) {
                                    // let (mut verts, mut inds, add_face) = local_gen_face(x, y, z, face_dir, txtr_mapping);
                                    let (mut verts, mut inds) = self.gen_face(
                                        chunk_pos(x,y,z), faces*4, face_dir, txtr_mapping,
                                    );
                                    total_verts.append(&mut verts);
                                    total_inds.append(&mut inds);
                                    faces += 1;
                                }
                            } else if (dx == 1 && x == self.chunk_size-1) || (dy == 1 && y == self.chunk_size-1) || (dz == -1 && z == 0) {
                                if !cull_border_face(x, y, z, face_dir) {
                                    // let (mut verts, mut inds, add_face) = local_gen_face(x, y, z, face_dir, txtr_mapping);
                                    let (mut verts, mut inds) = self.gen_face(
                                        chunk_pos(x,y,z), faces*4, face_dir, txtr_mapping,
                                    );
                                    total_verts.append(&mut verts);
                                    total_inds.append(&mut inds);
                                    faces += 1;
                                }
                            } else if self.check_coord_within_chunk((x as i32+dx) as u32,(y as i32+dy) as u32,(z as i32+dz) as u32) {
                                if !Self::check_block_obscured(chunk.voxels[self.access((x as i32+dx) as u32,(y as i32+dy) as u32,(z as i32+dz) as u32)]) {
                                    // if delta face coord is in chunk and not obscured
                                    // let (mut verts, mut inds, add_face) = local_gen_face(x, y, z, face_dir, txtr_mapping);
                                    let (mut verts, mut inds) = self.gen_face(
                                        chunk_pos(x,y,z), faces*4, face_dir, txtr_mapping,
                                    );
                                    total_verts.append(&mut verts);
                                    total_inds.append(&mut inds);
                                    faces += 1;
                                }
                            }
                        };

                        match &chunk.voxels[self.access(x, y, z)] {
                            BlockCullType::Opaque(block)
                            if matches!(self.block_ind[block.0 as usize].mesh, MeshType::Cube) => {
                                let txtr = self.block_ind[block.0 as usize].texture_id;

                                local_checked_gen_face(0, 0, 1, FaceDir::FRONT, txtr);
                                local_checked_gen_face(1, 0, 0, FaceDir::RIGHT, txtr);
                                local_checked_gen_face(0, 0, -1, FaceDir::BACK, txtr);
                                local_checked_gen_face(-1, 0, 0, FaceDir::LEFT, txtr);
                                local_checked_gen_face(0, 1, 0, FaceDir::TOP, txtr);
                                local_checked_gen_face(0, -1, 0, FaceDir::BOTTOM, txtr);
                            }
                            BlockCullType::Transparent(block)  // assuming XCross MeshType never culls as Opaque
                            if matches!(self.block_ind[block.0 as usize].mesh, MeshType::XCross) => {
                                let txtr = self.block_ind[block.0 as usize].texture_id;

                                // let (mut verts, mut inds, add_face) = local_gen_xcross(x, y, z, txtr);
                                let (mut verts, mut inds) = self.gen_xcross(
                                    chunk_pos(x,y,z), transparent_faces*4, txtr,
                                );
                                transparent_total_verts.append(&mut verts);
                                transparent_total_inds.append(&mut inds);
                                transparent_faces += 2;
                            }
                            _ => {}
                        }
                    }
                }
            }
        }

        vec![
            (total_verts, total_inds, RenderDataPurpose::TerrainVertices),
            (transparent_total_verts, transparent_total_inds, RenderDataPurpose::TransparentVertices),
        ]
    }
}

