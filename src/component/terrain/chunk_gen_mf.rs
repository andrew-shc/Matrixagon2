use std::collections::HashMap;
use std::rc::Rc;
use noise::{NoiseFn, Perlin};
use uom::si::f32::Length;
use crate::chunk_mesh::{Chunk, ChunkGeneratable, Position};
use crate::component::camera::Length3D;
use crate::component::RenderDataPurpose;
use crate::component::terrain::{Block, BlockCullType, BlockData, FaceDir};
use crate::component::terrain::mesh_util::ChunkMeshUtil;
use crate::component::terrain::terrain_gen::TerrainGenerator;
use crate::component::texture::TextureIDMapper;
use crate::measurement::{blox, chux_hf, chux_mf};
use crate::shader::chunk::ChunkVertex;

pub(super) struct ChunkGeneratorMF<'b>  {
    chunk_size: u32,
    block_ind: Vec<BlockData<'b>>,
    txtr_id_mapper: TextureIDMapper,
    terrain_gen: Rc<TerrainGenerator>,
}

impl<'b> ChunkGeneratorMF<'b> {
    pub(super) fn new(block_ind: Vec<BlockData<'b>>, txtr_id_mapper: TextureIDMapper, terrain_gen: Rc<TerrainGenerator>) -> Self {
        Self {
            chunk_size: Length::new::<<Self as ChunkGeneratable>::B>(1.0).get::<blox>() as u32, block_ind, txtr_id_mapper,
            terrain_gen
        }
    }
}

impl<'b> ChunkMeshUtil<'b> for ChunkGeneratorMF<'b> {
    fn chunk_size(&self) -> u32 {self.chunk_size}

    fn texture_id_mapper(&self) -> TextureIDMapper {self.txtr_id_mapper.clone()}

    fn block_ind(&self, ind: usize) -> BlockData<'b> {
        self.block_ind[ind]
    }

    fn terrain_gen(&self) -> Rc<TerrainGenerator> {
        self.terrain_gen.clone()
    }
}

impl ChunkGeneratable for ChunkGeneratorMF<'_> {
    type A = chux_mf;
    type B = chux_hf;
    type V = ChunkVertex;
    type I = u32;

    fn generate_mesh(&self, pos: Length3D)
        -> Vec<(Vec<Self::V>, Vec<Self::I>, Option<FaceDir>, RenderDataPurpose)>
    {
        let ofs = (pos.x.get::<blox>().ceil() as i32, pos.y.get::<blox>().ceil() as i32, pos.z.get::<blox>().ceil() as i32);
        let chunk_pos = |x: u32, y: u32, z: u32| (
            pos.x.get::<blox>()+x as f32,
            pos.y.get::<blox>()+y as f32,
            -pos.z.get::<blox>()-z as f32
        );

        let opaque_cube_mesh = self.voluminous_opaque_cubes_mesh(ofs, chunk_pos);
        let transparent_floral_mesh = self.sparse_transparent_floral_mesh(ofs, chunk_pos);
        let translucent_fluid_mesh = self.temporary_fluid_mesher(ofs, chunk_pos);

        let mut all_mesh = Vec::new();

        for (v, i, f) in opaque_cube_mesh {
            all_mesh.push((v, i, Some(f), RenderDataPurpose::TerrainOpaque))
        }
        all_mesh.push((transparent_floral_mesh.0, transparent_floral_mesh.1, None, RenderDataPurpose::TerrainTransparent));
        all_mesh.push((translucent_fluid_mesh.0, translucent_fluid_mesh.1, None, RenderDataPurpose::TerrainTranslucent));

        all_mesh
    }

    fn aggregate_mesh(&self,
                      central_pos: Length3D,
                      chunks: &HashMap<Position<Self::B>, Chunk<Self::V, Self::I, Self::B>>
    ) -> Vec<(Vec<Self::V>, Vec<Self::I>, RenderDataPurpose)>
    {
        println!("[MF] GEN AGGREGATED MESH");

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
            for (vert, raw_ind, _face, purpose) in chunk.mesh.iter() {
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
        }

        vec![
            (opaque_verts, opaque_inds, RenderDataPurpose::TerrainOpaque),
            (transparent_verts, transparent_inds, RenderDataPurpose::TerrainTransparent),
            (translucent_verts, translucent_inds, RenderDataPurpose::TerrainTranslucent),
        ]
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_terrain_generator() {
        let noise = Perlin::new(35);
        let floral_noise = Perlin::new(23);
        const SEA_LEVEL: f64 = 10.0;
        const SAND_LEVEL: f64 = 13.0;

        let terrain_generator = |x: f64, y: f64, z: f64| {
            let base_level = noise.get([x/20.0, z/20.0])*20.0+20.0;
            let floralness = floral_noise.get([x/40.0, z/40.0]);

            if y > base_level+1.0 {
                if y <= SEA_LEVEL {
                    BlockCullType::BorderVisibleFluid0(Block(6))
                } else {
                    BlockCullType::Empty
                }
            } else if y > base_level {
                if y <= SEA_LEVEL {
                    BlockCullType::BorderVisibleFluid0(Block(6))
                } else if 0.3 < floralness && floralness < 0.9 {
                    if 0.84 < floralness && floralness < 0.86 {
                        BlockCullType::AlwaysVisible(Block(5))
                    } else {
                        BlockCullType::AlwaysVisible(Block(4))
                    }
                } else {
                    BlockCullType::Empty
                }
            } else if y < SAND_LEVEL {
                BlockCullType::BorderVisible0(Block(3))
            } else if y > base_level-1.0 {
                BlockCullType::BorderVisible0(Block(3))  // TODO: LOD DEBUG
            } else if y > base_level-3.0 {
                BlockCullType::BorderVisible0(Block(1))
            } else {
                BlockCullType::BorderVisible0(Block(2))
            }
        };
        let max_height_bounds = |x: f64, z: f64| {
            let base_level = noise.get([x/20.0, z/20.0])*20.0+20.0;
            let floralness = floral_noise.get([x/40.0, z/40.0]);

            if 0.3 < floralness && floralness < 0.9 {
                base_level+1.0
            } else {
                base_level
            }
        };

        const CHUNK_SIZE: usize = 64;

        let mut xy_grid = vec![vec![]; CHUNK_SIZE*CHUNK_SIZE];
        let mut yz_grid = vec![vec![]; CHUNK_SIZE*CHUNK_SIZE];
        let mut xz_grid = vec![vec![]; CHUNK_SIZE*CHUNK_SIZE];

        let mut yz_grid_truth = vec![BlockCullType::Empty; CHUNK_SIZE*CHUNK_SIZE];
        // let mut first = true;

        let mut xz_max_height_bounds = vec![0; CHUNK_SIZE*CHUNK_SIZE];
        let mut min_height_bound = CHUNK_SIZE as usize;
        let mut max_height_bound = 0usize;

        for x in 0..CHUNK_SIZE {
            for z in 0..CHUNK_SIZE {
                let hb = max_height_bounds(x as f64, z as f64).ceil() as isize;
                xz_max_height_bounds[x*CHUNK_SIZE+z] = hb;
                if hb > max_height_bound as isize {
                    max_height_bound = (hb as usize).clamp(0, CHUNK_SIZE);
                }
                if hb < min_height_bound as isize {
                    min_height_bound = (hb as usize).clamp(0, CHUNK_SIZE);
                }
            }
        }

        println!("MAX/MIN HEIGHT BOUND: {:?} {:?}", max_height_bound, min_height_bound);

        for x in 0..CHUNK_SIZE {
            for z in 0..CHUNK_SIZE {
                // let height = max_height_bounds(x as f64, z as f64).ceil() as isize;
                let height = xz_max_height_bounds[x*CHUNK_SIZE+z];
                // TODO: multiple height bounds when we add caves, overhangs, trees/models, etc.
                for y in min_height_bound..max_height_bound {
                    let open = y as isize >= height;
                    let mut xy_cell = &mut xy_grid[x*CHUNK_SIZE+y];
                    let mut yz_cell = &mut yz_grid[y*CHUNK_SIZE+z];

                    if x == 0 {  // TODO: test code
                        yz_grid_truth[y*CHUNK_SIZE+z] = terrain_generator(0.0, y as f64, z as f64);
                    }

                    if xy_cell.len()%2 == 1 && open {
                        // current hit cell is set to closed that needs to be opened at the current block index
                        xy_cell.push(z as u16);
                    } else if xy_cell.len()%2 == 0 && !open {
                        // current hit cell is set to opened that needs to be closed at the current block index
                        xy_cell.push(z as u16);
                    }
                    if yz_cell.len()%2 == 1 && open {
                        // current hit cell is set to closed that needs to be opened at the current block index
                        yz_cell.push(x as u16);
                    } else if yz_cell.len()%2 == 0 && !open {
                        // current hit cell is set to opened that needs to be closed at the current block index
                        yz_cell.push(x as u16);
                    }
                }
                if x == 0 {
                    for y in 0..min_height_bound {
                        // TODO: test code
                        yz_grid_truth[y*CHUNK_SIZE+z] = terrain_generator(0.0, y as f64, z as f64);

                        let mut xy_cell = &mut xy_grid[x*CHUNK_SIZE+y];
                        if xy_cell.len()%2 == 0 {
                            // current hit cell is set to opened that needs to be closed at the current block index
                            xy_cell.push(z as u16);
                        }
                    }
                }
                if z == 0 {
                    for y in 0..min_height_bound {
                        let mut yz_cell = &mut yz_grid[y * CHUNK_SIZE + z];
                        if yz_cell.len()%2 == 0 {
                            // current hit cell is set to opened that needs to be closed at the current block index
                            yz_cell.push(x as u16);
                        }
                    }
                }
                xz_grid[x*CHUNK_SIZE+z].push(height as u16);
                //
                // let mut y = (CHUNK_SIZE-1) as isize;
                // let height = max_height_bounds(x as f64, z as f64).ceil() as isize;
                // while y >= 0 {
                //     let open = y >= height;
                //     let mut xy_cell = &mut xy_grid[x*CHUNK_SIZE+y as usize];
                //     let mut yz_cell = &mut yz_grid[(y as usize)*CHUNK_SIZE+z];
                //
                //     if x == 0 {
                //         yz_grid_truth[(y as usize)*CHUNK_SIZE+z] = terrain_generator(0.0, y as f64, z as f64);
                //     }
                //
                //     if xy_cell.len()%2 == 1 {
                //         // currently set to closed
                //         if open {
                //             // append to set to open at the current block index
                //             xy_cell.push(z as u16);
                //         } else {}
                //     } else {
                //         // currently set to opened
                //         if open {} else {
                //             // append to set to closed at the current block index
                //             xy_cell.push(z as u16);
                //         }
                //     }
                //     if yz_cell.len()%2 == 1 {
                //         // currently set to closed
                //         if open {
                //             // append to set to open at the current block index
                //             yz_cell.push(x as u16);
                //         } else {}
                //     } else {
                //         // currently set to opened
                //         if open {} else {
                //             // append to set to closed at the current block index
                //             yz_cell.push(x as u16);
                //         }
                //     }
                //
                //     y -= 1;
                // }
                // xz_grid[x*CHUNK_SIZE+z].push(height as u16);
            }
        }

        println!("TRUTH:");
        for y in 0..CHUNK_SIZE {
            print!("Y={:>2}: ", CHUNK_SIZE-1-y);
            for z in 0..CHUNK_SIZE {
                print!("{}",
                    match yz_grid_truth[(CHUNK_SIZE-1-y)*CHUNK_SIZE+z] {
                        BlockCullType::Empty => {"__"}
                        BlockCullType::AlwaysVisible(Block(i)) => {"AA"}
                        BlockCullType::BorderVisible0(Block(i)) => {"BB"}
                        BlockCullType::BorderVisible1(Block(i)) => {"BB"}
                        BlockCullType::BorderVisible2(Block(i)) => {"BB"}
                        BlockCullType::BorderVisible3(Block(i)) => {"BB"}
                        BlockCullType::BorderVisibleFluid0(Block(i)) => {"bb"}
                        BlockCullType::BorderVisibleFluid1(Block(i)) => {"bb"}
                        BlockCullType::BorderVisibleFluid2(Block(i)) => {"bb"}
                        BlockCullType::BorderVisibleFluid3(Block(i)) => {"bb"}
                        BlockCullType::Obscured => {"##"}
                        BlockCullType::ObscuredFluid => {"%%"}
                    }
                );
            }
            println!("");
        }
        print!("    : ");
        for z in 0..CHUNK_SIZE {
            print!("{:>2}", z);
        }
        println!("");

        println!("HIT LIST:");
        for y in 0..CHUNK_SIZE {
            print!("Y={:>2}: ", CHUNK_SIZE-1-y);
            for hit_content in &xy_grid[0*CHUNK_SIZE+(CHUNK_SIZE-1-y)] {
                print!("{}, ", hit_content);
            }
            println!("");
        }
    }
}
