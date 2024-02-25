use std::collections::HashMap;
use std::rc::Rc;
use noise::{NoiseFn, Perlin, Simplex};
use uom::si::f32::Length;
use crate::chunk_mesh::{Chunk, ChunkGeneratable, Position};
use crate::component::camera::Length3D;
use crate::component::RenderDataPurpose;
use crate::component::terrain::{Block, BlockCullType, BlockData, FaceDir, MeshType, TransparencyType};
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
    const SEA_LEVEL: f64 = 10.0;
    const SAND_LEVEL: f64 = 13.0;

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
    type P = BlockCullType;
    type V = ChunkVertex;
    type I = u32;

    fn generate_voxel(&self, pos: Length3D) -> Box<[Self::P]> {
        // println!("GEN VOXEL? [MF]");

        let mut raw_voxels = vec![];

        let mut voxel = raw_voxels.into_boxed_slice();

        voxel
    }

    fn generate_mesh(&self, pos: Length3D, voxels: &[Self::P]) -> Vec<(Vec<Self::V>, Vec<Self::I>, Option<FaceDir>, RenderDataPurpose)> {
        // println!("[MF] GEN CHUNK MESH: {:?}", pos);
        // let mut opaque_verts = vec![];
        // let mut opaque_inds = vec![];
        // let mut opaque_faces = 0;
        // let mut transparent_verts = vec![];
        // let mut transparent_inds = vec![];
        // let mut transparent_faces = 0;
        // let mut translucent_verts = vec![];
        // let mut translucent_inds = vec![];
        // let mut translucent_faces = 0;

        // let expanded_size = self.chunk_size+1;

        // let terrain_generator = |x: f64, y: f64, z: f64| {
        //     let base_level = self.noise.get([x/20.0, z/20.0])*20.0+20.0;
        //     let floralness = self.floral_noise.get([x/40.0, z/40.0]);
        //
        //     if y >= base_level+1.0 {
        //         if y <= Self::SEA_LEVEL {
        //             BlockCullType::BorderVisibleFluid0(Block(6))
        //         } else {
        //             BlockCullType::Empty
        //         }
        //     } else if y >= base_level {
        //         if y <= Self::SEA_LEVEL {
        //             BlockCullType::BorderVisibleFluid0(Block(6))
        //         } else if 0.8 <= floralness && floralness <= 0.9 {
        //             if 0.84 <= floralness && floralness <= 0.86 {
        //                 BlockCullType::AlwaysVisible(Block(5))
        //             } else {
        //                 BlockCullType::AlwaysVisible(Block(4))
        //             }
        //         } else {
        //             BlockCullType::Empty
        //         }
        //     } else if y <= Self::SAND_LEVEL {
        //         BlockCullType::BorderVisible0(Block(3))
        //     } else if y >= base_level-1.0 {
        //         BlockCullType::BorderVisible0(Block(0))
        //     } else if y >= base_level-3.0 {
        //         BlockCullType::BorderVisible0(Block(1))
        //     } else {
        //         BlockCullType::BorderVisible0(Block(2))
        //     }
        // };
        // let opaque_block_max_height_bounds = |x: f64, z: f64| {
        //     let base_level = self.noise.get([x/20.0, z/20.0])*20.0+20.0;
        //
        //     base_level
        // };

        // let chunk_pos = |x: u32, y: u32, z: u32| (
        //     pos.x.get::<blox>()+x as f32,
        //     pos.y.get::<blox>()+y as f32,
        //     -pos.z.get::<blox>()-z as f32
        // );
        // let x_ofs = pos.x.get::<blox>().ceil() as i32;
        // let y_ofs = pos.y.get::<blox>().ceil() as i32;
        // let z_ofs = pos.z.get::<blox>().ceil() as i32;
        //
        // let mut xy_grid: Vec<u16> = vec![0u16; (expanded_size*expanded_size) as usize];
        // let mut yz_grid: Vec<u16> = vec![0u16; (expanded_size*expanded_size) as usize];
        // let mut xz_grid: Vec<u16> = vec![0u16; (expanded_size*expanded_size) as usize];
        //
        // // HEIGHT BOUNDS to optimize terrains generation that are commonly one side full of voxels and other side empty
        // // - note: the height bounds are increased by one (i.e. expanded_size-1u32+1u32) since the mesh fill list algo
        // //      needs to check one additional block (just like the expanded checking of the chunk size)
        //
        // let mut xz_max_height_bounds = vec![0; (expanded_size*expanded_size) as usize];
        // let mut min_height_bound = expanded_size;
        // let mut max_height_bound = 0u32;
        //
        // for x in 0..expanded_size {
        //     for z in 0..expanded_size {
        //         let hb = self.terrain_gen.opaque_block_height_bound_test((x_ofs+x as i32) as f64, (z_ofs+z as i32) as f64).ceil() as isize;
        //         xz_max_height_bounds[(x*expanded_size+z) as usize] = hb;
        //         if hb > max_height_bound as isize+y_ofs as isize {
        //             max_height_bound = (hb.clamp(y_ofs as isize, y_ofs as isize+expanded_size as isize)-y_ofs as isize) as u32;
        //         }
        //         if hb < min_height_bound as isize+y_ofs as isize {
        //             min_height_bound = (hb.clamp(y_ofs as isize, y_ofs as isize+expanded_size as isize)-y_ofs as isize) as u32;
        //         }
        //     }
        // }
        //
        // // incremented max height bound to do the final block check vertically, for those faces on the top edge
        // max_height_bound = ((max_height_bound as isize+1).clamp(y_ofs as isize, y_ofs as isize+expanded_size as isize)-y_ofs as isize) as u32;
        //
        // // println!("HB MIN: {:?} MAX: {:?}", min_height_bound, max_height_bound);
        //
        // // for x == 0, set cells to start with closed
        // for y in 0..min_height_bound {
        //     let mut xy_cell = &mut xy_grid[(0*expanded_size+y) as usize];
        //     if *xy_cell%2 == 0 {
        //         *xy_cell += 1;
        //     }
        // }
        // for x in 0..expanded_size {
        //     // for z == 0, set cells to start with closed
        //     for y in 0..min_height_bound {
        //         let mut yz_cell = &mut yz_grid[(y*expanded_size+0) as usize];
        //         if *yz_cell%2 == 0 {
        //             *yz_cell += 1;
        //         }
        //     }
        //     for z in 0..expanded_size {
        //         // let height = opaque_block_max_height_bounds((x_ofs+x as i32) as f64, (z_ofs+z as i32) as f64).ceil() as isize;
        //         let height = xz_max_height_bounds[(x*expanded_size+z) as usize];
        //
        //         // TODO: multiple height bounds when we add caves, overhangs, trees/models, etc.
        //
        //         // for y == 0, set cells to start with closed
        //         let mut xz_cell = &mut xz_grid[(x*expanded_size+z) as usize];
        //         if *xz_cell%2 == 0 {
        //             *xz_cell += 1;
        //         }
        //
        //         for y in min_height_bound..max_height_bound {
        //             let open = y_ofs as isize+y as isize >= height;
        //             let mut xy_cell = &mut xy_grid[(x*expanded_size+y) as usize];
        //             let mut yz_cell = &mut yz_grid[(y*expanded_size+z) as usize];
        //             let mut xz_cell = &mut xz_grid[(x*expanded_size+z) as usize];
        //
        //             let lazy_block_gen = |dx: i32, dy: i32, dz: i32| {
        //                 self.terrain_gen.get_block((x_ofs+dx+x as i32) as f64, (y_ofs+dy+y as i32) as f64, (z_ofs+dz+z as i32) as f64)
        //             };
        //
        //             let mut fast_face_gen = |
        //                 total_verts: &mut Vec<ChunkVertex>, total_inds: &mut Vec<u32>, total_faces: &mut u32,
        //                 dx, dy, dz, face_dir, txtr_mapping
        //             | {
        //                 if self.check_coord_within_chunk(x as i32+dx,y as i32+dy,z as i32+dz) {
        //                     let (mut verts, mut inds) = self.gen_face(
        //                         chunk_pos((x as i32+dx) as u32,(y as i32+dy) as u32,(z as i32+dz) as u32),
        //                         *total_faces*4, face_dir, txtr_mapping, false
        //                     );
        //                     total_verts.append(&mut verts);
        //                     total_inds.append(&mut inds);
        //                     *total_faces += 1;
        //                 }
        //             };
        //
        //             let mut fast_block_face_gen = |block, dx: i32, dy: i32, dz: i32, face_dir| {
        //                 if let
        //                     BlockCullType::BorderVisible0(block) |
        //                     BlockCullType::BorderVisibleFluid0(block) |
        //                     BlockCullType::AlwaysVisible(block)
        //                     = &block
        //                 {
        //                     let block = self.block_ind[block.0 as usize];
        //                     let txtr = block.texture_id;
        //
        //                     // mesh assumed to be (opaque) cube
        //                     fast_face_gen(&mut opaque_verts, &mut opaque_inds, &mut opaque_faces, dx, dy, dz, face_dir, txtr);
        //                 }
        //             };
        //
        //             if *xy_cell%2 == 1 && open {
        //                 // current hit cell is set to closed that needs to be opened at the previous block index
        //                 *xy_cell += 1;
        //                 fast_block_face_gen(lazy_block_gen(0, 0,-1), 0, 0,-1, FaceDir::FRONT);
        //             } else if *xy_cell%2 == 0 && !open {
        //                 // current hit cell is set to opened that needs to be closed at the current block index
        //                 *xy_cell += 1;
        //
        //                 if z > 0 || (z == 0 && !Self::check_block_obscured(lazy_block_gen(0, 0,-1))) {
        //                     fast_block_face_gen(lazy_block_gen(0, 0, 0), 0, 0, 0, FaceDir::BACK);
        //                 }
        //             }
        //
        //             if *yz_cell%2 == 1 && open {
        //                 // current hit cell is set to closed that needs to be opened at the previous block index
        //                 *yz_cell += 1;
        //                 fast_block_face_gen(lazy_block_gen(-1, 0, 0), -1, 0, 0, FaceDir::RIGHT);
        //             } else if *yz_cell%2 == 0 && !open {
        //                 // current hit cell is set to opened that needs to be closed at the current block index
        //                 *yz_cell += 1;
        //
        //                 if x > 0 || (x == 0 && !Self::check_block_obscured(lazy_block_gen(-1, 0, 0))) {
        //                     fast_block_face_gen(lazy_block_gen(0, 0, 0), 0, 0, 0, FaceDir::LEFT);
        //                 }
        //             }
        //
        //             if *xz_cell%2 == 1 && open {
        //                 // current hit cell is set to closed that needs to be opened at the previous block index
        //                 *xz_cell += 1;
        //                 fast_block_face_gen(lazy_block_gen( 0,-1, 0),  0, -1, 0, FaceDir::TOP);
        //             } else if *xz_cell%2 == 0 && !open {
        //                 // current hit cell is set to opened that needs to be closed at the current block index
        //                 *xz_cell += 1;
        //
        //                 if y > 0 || (y == 0 && !Self::check_block_obscured(lazy_block_gen( 0,-1, 0))) {
        //                     fast_block_face_gen(lazy_block_gen(0, 0, 0), 0, 0, 0, FaceDir::BOTTOM);
        //                 }
        //             }
        //         }
        //         xz_grid[(x*expanded_size+z) as usize] += 1;
        //     }
        // }

        // for x in 0..self.chunk_size {
        //     for y in 0..self.chunk_size {
        //         for z in 0..self.chunk_size {
        //             let mut local_checked_gen_face = |
        //                 total_verts: &mut Vec<ChunkVertex>, total_inds: &mut Vec<u32>, total_faces: &mut u32,
        //                 dx, dy, dz, face_dir, txtr_mapping, fluid: bool, border_always_clear: bool| {
        //                 if self.check_coord_within_chunk(x as i32+dx,y as i32+dy,z as i32+dz) {
        //                     // inner face mesh culling
        //                     let block_cull = voxels[self.access((x as i32+dx) as u32,(y as i32+dy) as u32,(z as i32+dz) as u32)];
        //                     if (!fluid && !Self::check_block_obscured(block_cull)) || (fluid && !Self::check_fluid_obscured(block_cull)) {
        //                         // if delta face coord is in chunk and not obscured
        //                         let (mut verts, mut inds) = self.gen_face(
        //                             chunk_pos(x,y,z), *total_faces*4, face_dir, txtr_mapping, fluid
        //                         );
        //                         total_verts.append(&mut verts);
        //                         total_inds.append(&mut inds);
        //                         *total_faces += 1;
        //                     }
        //                 }
        //             };
        //
        //             if let
        //                 BlockCullType::BorderVisible0(block) |
        //                 BlockCullType::BorderVisibleFluid0(block) |
        //                 BlockCullType::AlwaysVisible(block)
        //                 = &voxels[self.access(x,y,z)]
        //             {
        //                 let block = self.block_ind[block.0 as usize];
        //
        //                 let txtr = block.texture_id;
        //
        //                 let (verts, inds, faces) = match block.transparency {
        //                     TransparencyType::Opaque => {(&mut opaque_verts, &mut opaque_inds, &mut opaque_faces)}
        //                     TransparencyType::Transparent => {(&mut transparent_verts, &mut transparent_inds, &mut transparent_faces)}
        //                     TransparencyType::Translucent => {(&mut translucent_verts, &mut translucent_inds, &mut translucent_faces)}
        //                 };
        //
        //                 match block.mesh {
        //                     MeshType::Cube => {
        //                         local_checked_gen_face(verts, inds, faces, 0, 0, 1, FaceDir::FRONT, txtr, false, false);
        //                         local_checked_gen_face(verts, inds, faces, 1, 0, 0, FaceDir::RIGHT, txtr, false, false);
        //                         local_checked_gen_face(verts, inds, faces, 0, 0, -1, FaceDir::BACK, txtr, false, false);
        //                         local_checked_gen_face(verts, inds, faces, -1, 0, 0, FaceDir::LEFT, txtr, false, false);
        //                         local_checked_gen_face(verts, inds, faces, 0, 1, 0, FaceDir::TOP, txtr, false, false);
        //                         local_checked_gen_face(verts, inds, faces, 0, -1, 0, FaceDir::BOTTOM, txtr, false, false);
        //                     }
        //                     MeshType::XCross => {
        //                         let (mut xcross_verts, mut xcross_inds) = self.gen_xcross(
        //                             chunk_pos(x,y,z), *faces*4, txtr,
        //                         );
        //                         verts.append(&mut xcross_verts);
        //                         inds.append(&mut xcross_inds);
        //                         *faces += 2;
        //                     }
        //                     MeshType::Fluid => {
        //                         local_checked_gen_face(verts, inds, faces, 0, 0, 1, FaceDir::FRONT, txtr, true, true);
        //                         local_checked_gen_face(verts, inds, faces, 1, 0, 0, FaceDir::RIGHT, txtr, true, true);
        //                         local_checked_gen_face(verts, inds, faces, 0, 0, -1, FaceDir::BACK, txtr, true, true);
        //                         local_checked_gen_face(verts, inds, faces, -1, 0, 0, FaceDir::LEFT, txtr, true, true);
        //                         local_checked_gen_face(verts, inds, faces, 0, 1, 0, FaceDir::TOP, txtr, true, true);
        //                         local_checked_gen_face(verts, inds, faces, 0, -1, 0, FaceDir::BOTTOM, txtr, true, true);
        //                     }
        //                 }
        //             }
        //         }
        //     }
        // }

        let opaque_cube_mesh = self.voluminous_opaque_blocks_mesh(
            (pos.x.get::<blox>().ceil() as i32, pos.y.get::<blox>().ceil() as i32, pos.z.get::<blox>().ceil() as i32),
            |x: u32, y: u32, z: u32| (
                pos.x.get::<blox>()+x as f32,
                pos.y.get::<blox>()+y as f32,
                -pos.z.get::<blox>()-z as f32
            )
        );

        let mut all_mesh = Vec::new();

        for (v, i, f) in opaque_cube_mesh {
            all_mesh.push((v, i, Some(f), RenderDataPurpose::TerrainOpaque))
        }

        all_mesh
    }

    fn aggregate_mesh(&self,
                      central_pos: Length3D,
                      chunks: &HashMap<Position<Self::B>, Chunk<Self::P, Self::V, Self::I, Self::B>>
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

            // let chunk_pos = |x: u32, y: u32, z: u32| (
            //     chunk.pos.x.get::<blox>()+x as f32,
            //     chunk.pos.y.get::<blox>()+y as f32,
            //     -chunk.pos.z.get::<blox>()-z as f32
            // );
            //
            // let cull_border_face = |x, y, z, face_dir: FaceDir| {
            //     match face_dir {
            //         FaceDir::FRONT => {
            //             if let Some(ref hpos) = chunk.adjacency.front {
            //                 let adj_block = chunks.get(hpos).unwrap().voxels[self.access(x,y,0)];
            //                 Self::check_block_obscured(adj_block)
            //             } else { true }
            //             // if theres no chunk, then it probably means the player can't see it anyways
            //             // no need to render the whole face at the border
            //         }
            //         FaceDir::RIGHT => {
            //             if let Some(ref hpos) = chunk.adjacency.right {
            //                 let adj_block = chunks.get(hpos).unwrap().voxels[self.access(0,y,z)];
            //                 Self::check_block_obscured(adj_block)
            //             } else { true }
            //         }
            //         FaceDir::BACK => {
            //             if let Some(ref hpos) = chunk.adjacency.back {
            //                 let adj_block = chunks.get(hpos).unwrap().voxels[self.access(x,y,self.chunk_size-1)];
            //                 Self::check_block_obscured(adj_block)
            //             } else { true }
            //         }
            //         FaceDir::LEFT => {
            //             if let Some(ref hpos) = chunk.adjacency.left {
            //                 let adj_block = chunks.get(hpos).unwrap().voxels[self.access(self.chunk_size-1,y,z)];
            //                 Self::check_block_obscured(adj_block)
            //             } else { true }
            //         }
            //         FaceDir::TOP => {
            //             if let Some(ref hpos) = chunk.adjacency.top {
            //                 let adj_block = chunks.get(hpos).unwrap().voxels[self.access(x,0,z)];
            //                 Self::check_block_obscured(adj_block)
            //             } else { true }
            //         }
            //         FaceDir::BOTTOM => {
            //             if let Some(ref hpos) = chunk.adjacency.bottom {
            //                 let adj_block = chunks.get(hpos).unwrap().voxels[self.access(x,self.chunk_size-1,z)];
            //                 Self::check_block_obscured(adj_block)
            //             } else { true }
            //         }
            //     }
            // };
            //
            // for x in 0..self.chunk_size {
            //     for y in 0..self.chunk_size {
            //         for z in 0..self.chunk_size {
            //             let mut local_checked_gen_face = |
            //                 total_verts: &mut Vec<ChunkVertex>, total_inds: &mut Vec<u32>, total_faces: &mut u32,
            //                 dx, dy, dz, face_dir, txtr_mapping, fluid: bool, border_always_clear: bool| {
            //                 // if (dx == -1 && x == 0) || (dy == -1 && y == 0) || (dz == 1 && z == self.chunk_size-1) ||
            //                 //     ((dx == 1 && x == self.chunk_size-1) || (dy == 1 && y == self.chunk_size-1) || (dz == -1 && z == 0)) {
            //                 // }
            //                 if !self.check_coord_within_chunk(x as i32+dx,y as i32+dy,z as i32+dz) {
            //                     // chunk border mesh culling (more like checking whether any exposed border faces that needs to be shown/added)
            //                     if !cull_border_face(x, y, z, face_dir) && !border_always_clear {
            //                         let (mut verts, mut inds) = self.gen_face(
            //                             chunk_pos(x,y,z), *total_faces*4, face_dir, txtr_mapping, fluid
            //                         );
            //                         total_verts.append(&mut verts);
            //                         total_inds.append(&mut inds);
            //                         *total_faces += 1;
            //                     }
            //                 }
            //             };
            //
            //             if let BlockCullType::BorderVisible0(block) | BlockCullType::BorderVisibleFluid0(block) | BlockCullType::AlwaysVisible(block)
            //                 = &chunk.voxels[self.access(x,y,z)] {
            //                 let block = self.block_ind[block.0 as usize];
            //
            //                 let txtr = block.texture_id;
            //
            //                 let (verts, inds, faces) = match block.transparency {
            //                     TransparencyType::Opaque => {(&mut opaque_verts, &mut opaque_inds, &mut opaque_faces)}
            //                     TransparencyType::Transparent => {(&mut transparent_verts, &mut transparent_inds, &mut transparent_faces)}
            //                     TransparencyType::Translucent => {(&mut translucent_verts, &mut translucent_inds, &mut translucent_faces)}
            //                 };
            //
            //                 match block.mesh {
            //                     MeshType::Cube => {
            //                         local_checked_gen_face(verts, inds, faces, 0, 0, 1, FaceDir::FRONT, txtr, false, false);
            //                         local_checked_gen_face(verts, inds, faces, 1, 0, 0, FaceDir::RIGHT, txtr, false, false);
            //                         local_checked_gen_face(verts, inds, faces, 0, 0, -1, FaceDir::BACK, txtr, false, false);
            //                         local_checked_gen_face(verts, inds, faces, -1, 0, 0, FaceDir::LEFT, txtr, false, false);
            //                         local_checked_gen_face(verts, inds, faces, 0, 1, 0, FaceDir::TOP, txtr, false, false);
            //                         local_checked_gen_face(verts, inds, faces, 0, -1, 0, FaceDir::BOTTOM, txtr, false, false);
            //                     }
            //                     MeshType::Fluid => {
            //                         local_checked_gen_face(verts, inds, faces, 0, 0, 1, FaceDir::FRONT, txtr, true, true);
            //                         local_checked_gen_face(verts, inds, faces, 1, 0, 0, FaceDir::RIGHT, txtr, true, true);
            //                         local_checked_gen_face(verts, inds, faces, 0, 0, -1, FaceDir::BACK, txtr, true, true);
            //                         local_checked_gen_face(verts, inds, faces, -1, 0, 0, FaceDir::LEFT, txtr, true, true);
            //                         local_checked_gen_face(verts, inds, faces, 0, 1, 0, FaceDir::TOP, txtr, true, true);
            //                         local_checked_gen_face(verts, inds, faces, 0, -1, 0, FaceDir::BOTTOM, txtr, true, true);
            //                     }
            //                     _ => {}
            //                 }
            //             }
            //         }
            //     }
            // }
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
