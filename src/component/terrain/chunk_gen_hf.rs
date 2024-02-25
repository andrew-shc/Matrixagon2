use std::collections::HashMap;
use noise::{NoiseFn, Perlin};
use uom::si::f32::Length;
use crate::chunk_mesh::{Chunk, ChunkGeneratable, Position};
use crate::component::camera::Length3D;
use crate::component::RenderDataPurpose;
use crate::component::terrain::{Block, BlockCullType, BlockData, FaceDir, MeshType, TransparencyType};
use crate::component::terrain::mesh_util::ChunkMeshUtil;
use crate::component::texture::TextureIDMapper;
use crate::measurement::{blox, chux, chux_hf};
use crate::shader::chunk::ChunkVertex;

pub(super) struct ChunkGeneratorHF<'b> {
    chunk_size: u32,
    block_ind: Vec<BlockData<'b>>,
    txtr_id_mapper: TextureIDMapper,
    noise: Perlin,
    floral_noise: Perlin,
}

impl<'b> ChunkGeneratorHF<'b> {
    const SEA_LEVEL: f64 = 10.0;
    const SAND_LEVEL: f64 = 13.0;

    pub(super) fn new(block_ind: Vec<BlockData<'b>>, txtr_id_mapper: TextureIDMapper,) -> Self {
        Self {
            chunk_size: Length::new::<<Self as ChunkGeneratable>::B>(1.0).get::<blox>() as u32, block_ind, txtr_id_mapper,
            noise: Perlin::new(50), floral_noise: Perlin::new(23),
        }
    }
}

impl ChunkMeshUtil for ChunkGeneratorHF<'_> {
    fn chunk_size(&self) -> u32 {self.chunk_size}

    fn texture_id_mapper(&self) -> TextureIDMapper {self.txtr_id_mapper.clone()}
}

impl ChunkGeneratable for ChunkGeneratorHF<'_> {
    type A = chux_hf;
    type B = chux;
    type P = BlockCullType;
    type V = ChunkVertex;
    type I = u32;

    fn generate_voxel(&self, pos: Length3D) -> Box<[Self::P]> {
        // println!("GEN VOXEL");
        let mut raw_voxels = Vec::with_capacity((self.chunk_size*self.chunk_size*self.chunk_size) as usize);
        let pos: Position<blox> = pos.into();

        for y in pos.y..pos.y+self.chunk_size() as isize {
            for x in pos.x..pos.x+self.chunk_size() as isize {
                for z in pos.z..pos.z+self.chunk_size() as isize {
                    raw_voxels.push({
                        let (x,y,z) = (x as f64, y as f64, z as f64);

                        let base_level = self.noise.get([x/20.0, z/20.0])*20.0+20.0;
                        let floralness = self.floral_noise.get([x/40.0, z/40.0]);

                        if y > base_level+1.0 {
                            if y <= Self::SEA_LEVEL {
                                BlockCullType::BorderVisibleFluid0(Block(6))
                            } else {
                                BlockCullType::Empty
                            }
                        } else if y > base_level {
                            if y <= Self::SEA_LEVEL {
                                BlockCullType::BorderVisibleFluid0(Block(6))
                            } else if 0.8 < floralness && floralness < 0.9 {
                                if 0.84 < floralness && floralness < 0.86 {
                                    BlockCullType::AlwaysVisible(Block(5))
                                } else {
                                    BlockCullType::AlwaysVisible(Block(4))
                                }
                            } else {
                                BlockCullType::Empty
                            }
                        } else if y < Self::SAND_LEVEL {
                            BlockCullType::BorderVisible0(Block(3))
                        } else if y > base_level-1.0 {
                            BlockCullType::BorderVisible0(Block(2))  // TODO: LOD DEBUG
                        } else if y > base_level-3.0 {
                            BlockCullType::BorderVisible0(Block(1))
                        } else {
                            BlockCullType::BorderVisible0(Block(2))
                        }

                        // if y > 40 {
                        //     BlockCullType::Empty
                        // } else {
                        //     BlockCullType::BorderVisible(Block(2))
                        // }
                    })
                }
            }
        }
        let mut voxel = raw_voxels.into_boxed_slice();

        self.block_culling(&mut voxel);

        voxel
    }

    fn generate_mesh(&self, pos: Length3D, voxels: &[Self::P])
        -> Vec<(Vec<Self::V>, Vec<Self::I>, RenderDataPurpose)>
    {
        // println!("GEN CHUNK MESH");
        let mut opaque_verts = vec![];
        let mut opaque_inds = vec![];
        let mut opaque_faces = 0;
        let mut transparent_verts = vec![];
        let mut transparent_inds = vec![];
        let mut transparent_faces = 0;
        let mut translucent_verts = vec![];
        let mut translucent_inds = vec![];
        let mut translucent_faces = 0;

        let chunk_pos = |x: u32, y: u32, z: u32| (
            pos.x.get::<blox>()+x as f32,
            pos.y.get::<blox>()+y as f32,
            -pos.z.get::<blox>()-z as f32
        );

        for x in 0..self.chunk_size {
            for y in 0..self.chunk_size {
                for z in 0..self.chunk_size {
                    let mut local_checked_gen_face = |
                        total_verts: &mut Vec<ChunkVertex>, total_inds: &mut Vec<u32>, total_faces: &mut u32,
                        dx, dy, dz, face_dir, txtr_mapping, fluid: bool, border_always_clear: bool| {
                        if self.check_coord_within_chunk(x as i32+dx,y as i32+dy,z as i32+dz) {
                            // inner face mesh culling
                            let block_cull = voxels[self.access((x as i32+dx) as u32,(y as i32+dy) as u32,(z as i32+dz) as u32)];
                            if (!fluid && !Self::check_block_obscured(block_cull)) || (fluid && !Self::check_fluid_obscured(block_cull)) {
                                // if delta face coord is in chunk and not obscured
                                let (mut verts, mut inds) = self.gen_face(
                                    chunk_pos(x,y,z), *total_faces*4, face_dir, txtr_mapping, fluid
                                );
                                total_verts.append(&mut verts);
                                total_inds.append(&mut inds);
                                *total_faces += 1;
                            }
                        }
                    };

                    if let
                        BlockCullType::BorderVisible0(block) |
                        BlockCullType::BorderVisibleFluid0(block) |
                        BlockCullType::AlwaysVisible(block)
                        = &voxels[self.access(x,y,z)]
                    {
                        let block = self.block_ind[block.0 as usize];

                        let txtr = block.texture_id;

                        let (verts, inds, faces) = match block.transparency {
                            TransparencyType::Opaque => {(&mut opaque_verts, &mut opaque_inds, &mut opaque_faces)}
                            TransparencyType::Transparent => {(&mut transparent_verts, &mut transparent_inds, &mut transparent_faces)}
                            TransparencyType::Translucent => {(&mut translucent_verts, &mut translucent_inds, &mut translucent_faces)}
                        };

                        match block.mesh {
                            MeshType::Cube => {
                                local_checked_gen_face(verts, inds, faces, 0, 0, 1, FaceDir::FRONT, txtr, false, false);
                                local_checked_gen_face(verts, inds, faces, 1, 0, 0, FaceDir::RIGHT, txtr, false, false);
                                local_checked_gen_face(verts, inds, faces, 0, 0, -1, FaceDir::BACK, txtr, false, false);
                                local_checked_gen_face(verts, inds, faces, -1, 0, 0, FaceDir::LEFT, txtr, false, false);
                                local_checked_gen_face(verts, inds, faces, 0, 1, 0, FaceDir::TOP, txtr, false, false);
                                local_checked_gen_face(verts, inds, faces, 0, -1, 0, FaceDir::BOTTOM, txtr, false, false);
                            }
                            MeshType::XCross => {
                                let (mut xcross_verts, mut xcross_inds) = self.gen_xcross(
                                    chunk_pos(x,y,z), *faces*4, txtr,
                                );
                                verts.append(&mut xcross_verts);
                                inds.append(&mut xcross_inds);
                                *faces += 2;
                            }
                            MeshType::Fluid => {
                                local_checked_gen_face(verts, inds, faces, 0, 0, 1, FaceDir::FRONT, txtr, true, true);
                                local_checked_gen_face(verts, inds, faces, 1, 0, 0, FaceDir::RIGHT, txtr, true, true);
                                local_checked_gen_face(verts, inds, faces, 0, 0, -1, FaceDir::BACK, txtr, true, true);
                                local_checked_gen_face(verts, inds, faces, -1, 0, 0, FaceDir::LEFT, txtr, true, true);
                                local_checked_gen_face(verts, inds, faces, 0, 1, 0, FaceDir::TOP, txtr, true, true);
                                local_checked_gen_face(verts, inds, faces, 0, -1, 0, FaceDir::BOTTOM, txtr, true, true);
                            }
                        }
                    }
                }
            }
        }

        vec![
            (opaque_verts, opaque_inds, RenderDataPurpose::TerrainOpaque),
            (transparent_verts, transparent_inds, RenderDataPurpose::TerrainTransparent),
            (translucent_verts, translucent_inds, RenderDataPurpose::TerrainTranslucent),
        ]
    }

    fn aggregate_mesh(&self, chunks: &HashMap<Position<Self::B>, Chunk<Self::P, Self::V, Self::I, Self::B>>)
        -> Vec<(Vec<Self::V>, Vec<Self::I>, RenderDataPurpose)>
    {
        println!("GEN AGGREGATED MESH");

        let mut opaque_verts = vec![];
        let mut opaque_inds = vec![];
        let mut opaque_faces = 0;
        let mut transparent_verts = vec![];
        let mut transparent_inds = vec![];
        let mut transparent_faces = 0;
        let mut translucent_verts = vec![];
        let mut translucent_inds = vec![];
        let mut translucent_faces = 0;

        for chunk in chunks.values().filter(|c| c.visible()) {
            for (vert, raw_ind, purpose) in chunk.mesh.iter() {
                match purpose {
                    RenderDataPurpose::TerrainOpaque => {
                        let mut ind = raw_ind.clone().iter().map(|i| i+opaque_faces*4).collect();
                        opaque_faces += vert.len() as u32/4;  // 4 vertices in each face

                        opaque_verts.append(&mut vert.clone());
                        opaque_inds.append(&mut ind);
                    }
                    RenderDataPurpose::TerrainTransparent => {
                        let mut ind = raw_ind.clone().iter().map(|i| i+transparent_faces*4).collect();
                        transparent_faces += vert.len() as u32/4;  // 4 vertices in each face

                        transparent_verts.append(&mut vert.clone());
                        transparent_inds.append(&mut ind);
                    }
                    RenderDataPurpose::TerrainTranslucent => {
                        let mut ind = raw_ind.clone().iter().map(|i| i+translucent_faces*4).collect();
                        translucent_faces += vert.len() as u32/4;  // 4 vertices in each face

                        translucent_verts.append(&mut vert.clone());
                        translucent_inds.append(&mut ind);
                    }
                    _ => {}
                }
            }

            let chunk_pos = |x: u32, y: u32, z: u32| (
                chunk.pos.x.get::<blox>()+x as f32,
                chunk.pos.y.get::<blox>()+y as f32,
                -chunk.pos.z.get::<blox>()-z as f32
            );

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
                        let mut local_checked_gen_face = |
                            total_verts: &mut Vec<ChunkVertex>, total_inds: &mut Vec<u32>, total_faces: &mut u32,
                            dx, dy, dz, face_dir, txtr_mapping, fluid: bool, border_always_clear: bool| {
                            // if (dx == -1 && x == 0) || (dy == -1 && y == 0) || (dz == 1 && z == self.chunk_size-1) ||
                            //     ((dx == 1 && x == self.chunk_size-1) || (dy == 1 && y == self.chunk_size-1) || (dz == -1 && z == 0)) {
                            // }
                            if !self.check_coord_within_chunk(x as i32+dx,y as i32+dy,z as i32+dz) {
                                // chunk border mesh culling (more like checking whether any exposed border faces that needs to be shown/added)
                                if !cull_border_face(x, y, z, face_dir) && !border_always_clear {
                                    let (mut verts, mut inds) = self.gen_face(
                                        chunk_pos(x,y,z), *total_faces*4, face_dir, txtr_mapping, fluid
                                    );
                                    total_verts.append(&mut verts);
                                    total_inds.append(&mut inds);
                                    *total_faces += 1;
                                }
                            }
                        };

                        if let BlockCullType::BorderVisible0(block) | BlockCullType::BorderVisibleFluid0(block) | BlockCullType::AlwaysVisible(block)
                            = &chunk.voxels[self.access(x,y,z)] {
                            let block = self.block_ind[block.0 as usize];

                            let txtr = block.texture_id;

                            let (verts, inds, faces) = match block.transparency {
                                TransparencyType::Opaque => {(&mut opaque_verts, &mut opaque_inds, &mut opaque_faces)}
                                TransparencyType::Transparent => {(&mut transparent_verts, &mut transparent_inds, &mut transparent_faces)}
                                TransparencyType::Translucent => {(&mut translucent_verts, &mut translucent_inds, &mut translucent_faces)}
                            };

                            match block.mesh {
                                MeshType::Cube => {
                                    local_checked_gen_face(verts, inds, faces, 0, 0, 1, FaceDir::FRONT, txtr, false, false);
                                    local_checked_gen_face(verts, inds, faces, 1, 0, 0, FaceDir::RIGHT, txtr, false, false);
                                    local_checked_gen_face(verts, inds, faces, 0, 0, -1, FaceDir::BACK, txtr, false, false);
                                    local_checked_gen_face(verts, inds, faces, -1, 0, 0, FaceDir::LEFT, txtr, false, false);
                                    local_checked_gen_face(verts, inds, faces, 0, 1, 0, FaceDir::TOP, txtr, false, false);
                                    local_checked_gen_face(verts, inds, faces, 0, -1, 0, FaceDir::BOTTOM, txtr, false, false);
                                }
                                MeshType::Fluid => {
                                    local_checked_gen_face(verts, inds, faces, 0, 0, 1, FaceDir::FRONT, txtr, true, true);
                                    local_checked_gen_face(verts, inds, faces, 1, 0, 0, FaceDir::RIGHT, txtr, true, true);
                                    local_checked_gen_face(verts, inds, faces, 0, 0, -1, FaceDir::BACK, txtr, true, true);
                                    local_checked_gen_face(verts, inds, faces, -1, 0, 0, FaceDir::LEFT, txtr, true, true);
                                    local_checked_gen_face(verts, inds, faces, 0, 1, 0, FaceDir::TOP, txtr, true, true);
                                    local_checked_gen_face(verts, inds, faces, 0, -1, 0, FaceDir::BOTTOM, txtr, true, true);
                                }
                                _ => {}
                            }
                        }
                    }
                }
            }
        }

        vec![
            (opaque_verts, opaque_inds, RenderDataPurpose::TerrainOpaque),
            (transparent_verts, transparent_inds, RenderDataPurpose::TerrainTransparent),
            (translucent_verts, translucent_inds, RenderDataPurpose::TerrainTranslucent),
        ]
    }
}

