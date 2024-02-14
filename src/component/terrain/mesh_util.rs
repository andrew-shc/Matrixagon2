use std::mem;
use crate::component::camera::Length3D;
use crate::component::terrain::{Block, BlockCullType, FaceDir, TextureMapper};
use crate::component::texture::TextureIDMapper;
use crate::measurement::blox;
use crate::shader::chunk::ChunkVertex;


pub(super) trait ChunkMeshUtil {
    fn chunk_size(&self) -> u32;

    fn texture_id_mapper(&self) -> TextureIDMapper;

    fn access(&self, x: u32, y: u32, z: u32) -> usize {
        (y*self.chunk_size()*self.chunk_size()+x*self.chunk_size()+z) as usize
    }

    fn check_block_obscured(block: BlockCullType) -> bool {
        mem::discriminant(&block) == mem::discriminant(&BlockCullType::BorderVisible(Block::default())) ||
            mem::discriminant(&block) == mem::discriminant(&BlockCullType::Obscured)
    }

    fn check_fluid_obscured(block: BlockCullType) -> bool {
        mem::discriminant(&block) == mem::discriminant(&BlockCullType::BorderVisibleFluid(Block::default())) ||
            mem::discriminant(&block) == mem::discriminant(&BlockCullType::ObscuredFluid) ||
            mem::discriminant(&block) == mem::discriminant(&BlockCullType::BorderVisible(Block::default())) ||
            mem::discriminant(&block) == mem::discriminant(&BlockCullType::Obscured)
    }

    fn reverse_access(&self, pos: Length3D, i: f32) -> (f64, f64, f64) {
        let y = (i/(self.chunk_size() as f32*self.chunk_size() as f32)).floor();
        let x = ((i-y*self.chunk_size() as f32*self.chunk_size() as f32)/self.chunk_size() as f32).floor();
        let z = (i-y*self.chunk_size() as f32*self.chunk_size() as f32) % self.chunk_size() as f32;
        (x as f64+pos.x.get::<blox>() as f64, y as f64+pos.y.get::<blox>() as f64, z as f64+pos.z.get::<blox>() as f64)
    }

    fn block_culling(&self, voxel: &mut Box<[BlockCullType]>) {
        for x in 1..self.chunk_size() -1 {
            for y in 1..self.chunk_size() -1 {
                for z in 1..self.chunk_size() -1 {
                    match voxel[self.access(x,y,z)] {
                        BlockCullType::BorderVisible(_) if
                        Self::check_block_obscured(voxel[self.access(x+1,y,z)]) &&
                            Self::check_block_obscured(voxel[self.access(x-1,y,z)]) &&
                            Self::check_block_obscured(voxel[self.access(x,y+1,z)]) &&
                            Self::check_block_obscured(voxel[self.access(x,y-1,z)]) &&
                            Self::check_block_obscured(voxel[self.access(x,y,z+1)]) &&
                            Self::check_block_obscured(voxel[self.access(x,y,z-1)]) => {
                            voxel[self.access(x,y,z)] = BlockCullType::Obscured;
                        }
                        BlockCullType::BorderVisibleFluid(_) if
                        Self::check_fluid_obscured(voxel[self.access(x+1,y,z)]) &&
                            Self::check_fluid_obscured(voxel[self.access(x-1,y,z)]) &&
                            Self::check_fluid_obscured(voxel[self.access(x,y+1,z)]) &&
                            Self::check_fluid_obscured(voxel[self.access(x,y-1,z)]) &&
                            Self::check_fluid_obscured(voxel[self.access(x,y,z+1)]) &&
                            Self::check_fluid_obscured(voxel[self.access(x,y,z-1)]) => {
                            voxel[self.access(x,y,z)] = BlockCullType::ObscuredFluid;
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    fn check_coord_within_chunk(&self, x: u32, y: u32, z: u32) -> bool {
        0 <= x && x < self.chunk_size() && 0 <= y && y < self.chunk_size() && 0 <= z && z < self.chunk_size()
    }

    fn gen_face(&self, loc: (f32, f32, f32), ind_ofs: u32, face: FaceDir, txtr_mapping: TextureMapper, fluid: bool) -> (Vec<ChunkVertex>, Vec<u32>) {
        let txtr_mapper = |name: &str| *self.texture_id_mapper().get(name).unwrap_or(&0) as f32;

        // TODO: encode indent height into the shader itself
        let hgt = if fluid {
            0.9
        } else {
            1.0
        };

        let (v, i) = match face {
            FaceDir::FRONT => {
                let txtr = txtr_mapper(txtr_mapping.front());

                (
                    vec![
                        ChunkVertex { pos: [loc.0+1.0, loc.1+0.0, -loc.2+0.0], uv: [1.0, 1.0], txtr },
                        ChunkVertex { pos: [loc.0+0.0, loc.1+hgt, -loc.2+0.0], uv: [0.0, 0.0], txtr },
                        ChunkVertex { pos: [loc.0+0.0, loc.1+0.0, -loc.2+0.0], uv: [0.0, 1.0], txtr },
                        ChunkVertex { pos: [loc.0+1.0, loc.1+hgt, -loc.2+0.0], uv: [1.0, 0.0], txtr },
                    ],
                    vec![0,1,2,3,1,0]
                )
            }
            FaceDir::RIGHT => {
                let txtr = txtr_mapper(txtr_mapping.right());

                (
                    vec![
                        ChunkVertex { pos: [loc.0+1.0, loc.1+0.0, -loc.2+0.0], uv: [1.0, 1.0], txtr },
                        ChunkVertex { pos: [loc.0+1.0, loc.1+hgt, -loc.2+0.0], uv: [1.0, 0.0], txtr },
                        ChunkVertex { pos: [loc.0+1.0, loc.1+0.0, -loc.2-1.0], uv: [0.0, 1.0], txtr },
                        ChunkVertex { pos: [loc.0+1.0, loc.1+hgt, -loc.2-1.0], uv: [0.0, 0.0], txtr },
                    ],
                    vec![0,2,1,3,1,2]
                )}
            FaceDir::BACK => {
                let txtr = txtr_mapper(txtr_mapping.back());

                (
                    vec![
                        ChunkVertex { pos: [loc.0+0.0, loc.1+0.0, -loc.2-1.0], uv: [0.0, 1.0], txtr },
                        ChunkVertex { pos: [loc.0+1.0, loc.1+0.0, -loc.2-1.0], uv: [1.0, 1.0], txtr },
                        ChunkVertex { pos: [loc.0+0.0, loc.1+hgt, -loc.2-1.0], uv: [0.0, 0.0], txtr },
                        ChunkVertex { pos: [loc.0+1.0, loc.1+hgt, -loc.2-1.0], uv: [1.0, 0.0], txtr },
                    ],
                    vec![1,0,3,2,3,0]
                )}
            FaceDir::LEFT => {
                let txtr = txtr_mapper(txtr_mapping.left());

                (
                    vec![
                        ChunkVertex { pos: [loc.0+0.0, loc.1+0.0, -loc.2+0.0], uv: [1.0, 1.0], txtr },
                        ChunkVertex { pos: [loc.0+0.0, loc.1+hgt, -loc.2+0.0], uv: [1.0, 0.0], txtr },
                        ChunkVertex { pos: [loc.0+0.0, loc.1+0.0, -loc.2-1.0], uv: [0.0, 1.0], txtr },
                        ChunkVertex { pos: [loc.0+0.0, loc.1+hgt, -loc.2-1.0], uv: [0.0, 0.0], txtr },
                    ],
                    vec![2,0,3,1,3,0]
                )}
            FaceDir::TOP => {
                let txtr = txtr_mapper(txtr_mapping.top());

                (
                    vec![
                        ChunkVertex { pos: [loc.0+0.0, loc.1+hgt, -loc.2+0.0], uv: [1.0, 1.0], txtr },
                        ChunkVertex { pos: [loc.0+1.0, loc.1+hgt, -loc.2+0.0], uv: [0.0, 1.0], txtr },
                        ChunkVertex { pos: [loc.0+0.0, loc.1+hgt, -loc.2-1.0], uv: [1.0, 0.0], txtr },
                        ChunkVertex { pos: [loc.0+1.0, loc.1+hgt, -loc.2-1.0], uv: [0.0, 0.0], txtr },
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
        let txtr_mapper = |name: &str| *self.texture_id_mapper().get(name).unwrap_or(&0) as f32;
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

