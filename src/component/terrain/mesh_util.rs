use std::mem;
use std::rc::Rc;
use crate::component::terrain::{Block, BlockCullType, BlockData, FaceDir, TextureMapper};
use crate::component::terrain::terrain_gen::TerrainGenerator;
use crate::component::texture::TextureIDMapper;
use crate::shader::chunk::ChunkVertex;


pub(super) trait ChunkMeshUtil<'b> {
    fn chunk_size(&self) -> u32;

    fn texture_id_mapper(&self) -> TextureIDMapper;

    fn block_ind(&self, ind: usize) -> BlockData<'b>;

    fn terrain_gen(&self) -> Rc<TerrainGenerator>;

    fn access(&self, x: u32, y: u32, z: u32) -> usize {
        let size = self.chunk_size();
        (y*size*size+x*size+z) as usize
    }

    fn check_block_obscured(block: BlockCullType) -> bool {
        mem::discriminant(&block) == mem::discriminant(&BlockCullType::BorderVisible0(Block::default())) ||
            mem::discriminant(&block) == mem::discriminant(&BlockCullType::Obscured)
    }

    fn check_fluid_obscured(block: BlockCullType) -> bool {
        mem::discriminant(&block) == mem::discriminant(&BlockCullType::BorderVisibleFluid0(Block::default())) ||
            mem::discriminant(&block) == mem::discriminant(&BlockCullType::ObscuredFluid) ||
            mem::discriminant(&block) == mem::discriminant(&BlockCullType::BorderVisible0(Block::default())) ||
            mem::discriminant(&block) == mem::discriminant(&BlockCullType::Obscured)
    }

    fn voluminous_opaque_cubes_mesh<C>(&self, ofs: (i32, i32, i32), chunk_pos: C) -> [(Vec<ChunkVertex>, Vec<u32>, FaceDir); 6]
        where C: Fn(u32, u32, u32) -> (f32, f32, f32)
    {
        let mut top_verts = vec![];
        let mut top_inds = vec![];
        let mut top_faces = 0;
        let mut bottom_verts = vec![];
        let mut bottom_inds = vec![];
        let mut bottom_faces = 0;
        let mut left_verts = vec![];
        let mut left_inds = vec![];
        let mut left_faces = 0;
        let mut right_verts = vec![];
        let mut right_inds = vec![];
        let mut right_faces = 0;
        let mut front_verts = vec![];
        let mut front_inds = vec![];
        let mut front_faces = 0;
        let mut back_verts = vec![];
        let mut back_inds = vec![];
        let mut back_faces = 0;

        let expanded_size = self.chunk_size()+1;

        let mut xy_grid: Vec<u16> = vec![0u16; (expanded_size*expanded_size) as usize];
        let mut yz_grid: Vec<u16> = vec![0u16; (expanded_size*expanded_size) as usize];
        let mut xz_grid: Vec<u16> = vec![0u16; (expanded_size*expanded_size) as usize];

        // HEIGHT BOUNDS to optimize terrains generation that are commonly one side full of voxels and other side empty
        // - note: the height bounds are increased by one (i.e. hb+1u32) since the mesh fill list algo
        //      needs to check one additional block for the closing face (just like the expanded checking of the chunk size)

        let mut xz_max_height_bounds = vec![0i32; (expanded_size*expanded_size) as usize];
        let mut min_height_bound = expanded_size;
        let mut max_height_bound = 0u32;

        for x in 0..expanded_size {
            for z in 0..expanded_size {
                let hb = self.terrain_gen().opaque_block_height_bound_test((ofs.0+x as i32) as f64, (ofs.2+z as i32) as f64).ceil() as i32;
                xz_max_height_bounds[(x*expanded_size+z) as usize] = hb;
                if hb-ofs.1+1 > max_height_bound as i32 {
                    max_height_bound = (hb-ofs.1+1).clamp(0i32, expanded_size as i32) as u32;
                }
                if hb-ofs.1 < min_height_bound as i32 {
                    min_height_bound = (hb-ofs.1).clamp(0i32, expanded_size as i32) as u32;
                }
            }
        }

        // for x == 0, set cells to start with closed
        for y in 0..min_height_bound {
            let mut xy_cell = &mut xy_grid[(0*expanded_size+y) as usize];
            if *xy_cell%2 == 0 {
                *xy_cell += 1;
            }
        }
        for x in 0..expanded_size {
            // for z == 0, set cells to start with closed
            for y in 0..min_height_bound {
                let mut yz_cell = &mut yz_grid[(y*expanded_size+0) as usize];
                if *yz_cell%2 == 0 {
                    *yz_cell += 1;
                }
            }
            for z in 0..expanded_size {
                // let height = opaque_block_max_height_bounds((x_ofs+x as i32) as f64, (z_ofs+z as i32) as f64).ceil() as isize;
                let hb = xz_max_height_bounds[(x*expanded_size+z) as usize];

                // TODO: multiple height bounds when we add caves, overhangs, trees/models, etc.

                // for y == 0, set cells to start with closed
                let mut xz_cell = &mut xz_grid[(x*expanded_size+z) as usize];
                if *xz_cell%2 == 0 {
                    *xz_cell += 1;
                }

                for y in min_height_bound..max_height_bound {
                    let open = ofs.1+y as i32 >= hb;
                    let mut xy_cell = &mut xy_grid[(x*expanded_size+y) as usize];
                    let mut yz_cell = &mut yz_grid[(y*expanded_size+z) as usize];
                    // let mut xz_cell = &mut xz_grid[(x*expanded_size+z) as usize];

                    let lazy_block_gen = |dx: i32, dy: i32, dz: i32| {
                        self.terrain_gen().get_block((ofs.0+dx+x as i32) as f64, (ofs.1+dy+y as i32) as f64, (ofs.2+dz+z as i32) as f64)
                    };

                    let mut fast_face_gen = |
                        total_verts: &mut Vec<ChunkVertex>, total_inds: &mut Vec<u32>, total_faces: &mut u32,
                        dx, dy, dz, face_dir, txtr_mapping
                    | {
                        let (mut verts, mut inds) = self.gen_face(
                            chunk_pos((x as i32+dx) as u32,(y as i32+dy) as u32,(z as i32+dz) as u32),
                            *total_faces*4, face_dir, txtr_mapping, false
                        );
                        total_verts.append(&mut verts);
                        total_inds.append(&mut inds);
                        *total_faces += 1;
                    };

                    let mut fast_block_face_gen = |block, verts, inds, faces, dx: i32, dy: i32, dz: i32, face_dir| {
                        if let
                            BlockCullType::BorderVisible0(block) |
                            BlockCullType::BorderVisibleFluid0(block) |
                            BlockCullType::AlwaysVisible(block)
                            = &block
                        {
                            let block = self.block_ind(block.0 as usize);
                            let txtr = block.texture_id;

                            // mesh assumed to be (opaque) cube
                            fast_face_gen(verts, inds, faces, dx, dy, dz, face_dir, txtr);
                        }
                    };

                    // prevents rendering the current face direction on the extended chunk size for the other direction's block checking
                    if x < self.chunk_size() && y < self.chunk_size() {
                        if *xy_cell%2 == 1 && open {
                            // current hit cell is set to closed that needs to be opened using previous block index
                            *xy_cell += 1;

                            fast_block_face_gen(
                                lazy_block_gen(0, 0,-1),
                                &mut front_verts, &mut front_inds, &mut front_faces,
                                0, 0,-1, FaceDir::FRONT
                            );
                        } else if *xy_cell%2 == 0 && !open {
                            // current hit cell is set to opened that needs to be closed using current block index
                            *xy_cell += 1;

                            if 0 < z {
                                fast_block_face_gen(
                                    lazy_block_gen(0, 0, 0),
                                    &mut back_verts, &mut back_inds, &mut back_faces,
                                    0, 0, 0, FaceDir::BACK
                                );
                            }
                        }
                    }

                    if y < self.chunk_size() && z < self.chunk_size() {
                        if *yz_cell%2 == 1 && open {
                            // current hit cell is set to closed that needs to be opened using previous block index
                            *yz_cell += 1;

                            fast_block_face_gen(
                                lazy_block_gen(-1, 0, 0),
                                &mut right_verts, &mut right_inds, &mut right_faces,
                                -1, 0, 0, FaceDir::RIGHT
                            );
                        } else if *yz_cell%2 == 0 && !open {
                            // current hit cell is set to opened that needs to be closed using current block index
                            *yz_cell += 1;

                            if x > 0 {
                                fast_block_face_gen(
                                    lazy_block_gen(0, 0, 0),
                                    &mut left_verts, &mut left_inds, &mut left_faces,
                                    0, 0, 0, FaceDir::LEFT
                                );
                            }
                        }
                    }

                    if x < self.chunk_size() && z < self.chunk_size() {
                        if *xz_cell%2 == 1 && open {
                            // current hit cell is set to closed that needs to be opened using previous block index
                            *xz_cell += 1;

                            fast_block_face_gen(
                                lazy_block_gen( 0,-1, 0),
                                &mut top_verts, &mut top_inds, &mut top_faces,
                                0, -1, 0, FaceDir::TOP
                            );
                        } else if *xz_cell%2 == 0 && !open {
                            // current hit cell is set to opened that needs to be closed using current block index
                            *xz_cell += 1;

                            if y > 0 {
                                fast_block_face_gen(
                                    lazy_block_gen(0, 0, 0),
                                    &mut bottom_verts, &mut bottom_inds, &mut bottom_faces,
                                    0, 0, 0, FaceDir::BOTTOM
                                );
                            }
                        }
                    }
                }
                xz_grid[(x*expanded_size+z) as usize] += 1;
            }
        }

        [
            (top_verts, top_inds, FaceDir::TOP),
            (bottom_verts, bottom_inds, FaceDir::BOTTOM),
            (left_verts, left_inds, FaceDir::LEFT),
            (right_verts, right_inds, FaceDir::RIGHT),
            (front_verts, front_inds, FaceDir::FRONT),
            (back_verts, back_inds, FaceDir::BACK),
        ]
    }

    fn sparse_transparent_floral_mesh<C>(&self, ofs: (i32, i32, i32), chunk_pos: C) -> (Vec<ChunkVertex>, Vec<u32>)
        where C: Fn(u32, u32, u32) -> (f32, f32, f32)
    {
        let mut transparent_verts = vec![];
        let mut transparent_inds = vec![];
        let mut transparent_faces = 0;

        for x in 0..self.chunk_size() {
            for z in 0..self.chunk_size() {
                if let Some(y) = self.terrain_gen().floral_existence_bound_test((ofs.0+x as i32) as f64, (ofs.2+z as i32) as f64) {
                    let y = y.ceil();
                    if ofs.1 as f64 <= y && y < ofs.1 as f64+self.chunk_size() as f64 {
                        if let
                            BlockCullType::BorderVisible0(block) |
                            BlockCullType::BorderVisibleFluid0(block) |
                            BlockCullType::AlwaysVisible(block)
                            = self.terrain_gen().get_block((ofs.0+x as i32) as f64, (y as i32) as f64, (ofs.2+z as i32) as f64)
                        {
                            // assumes floral mesh

                            let block = self.block_ind(block.0 as usize);

                            let txtr = block.texture_id;

                            let (mut xcross_verts, mut xcross_inds) = self.gen_xcross(
                                chunk_pos(x, (y as i32-ofs.1) as u32, z), transparent_faces*4, txtr,
                            );
                            transparent_verts.append(&mut xcross_verts);
                            transparent_inds.append(&mut xcross_inds);
                            transparent_faces += 2;
                        }
                    }
                };
            }
        }

        (transparent_verts, transparent_inds)
    }


    // TODO: TEMPORARY
    fn temporary_fluid_mesher<C>(&self, ofs: (i32, i32, i32), chunk_pos: C) -> (Vec<ChunkVertex>, Vec<u32>)
        where C: Fn(u32, u32, u32) -> (f32, f32, f32)
    {
        let mut translucent_verts = vec![];
        let mut translucent_inds = vec![];
        let mut translucent_faces = 0;

        let expanded_size = self.chunk_size()+1;

        let mut xz_grid: Vec<u16> = vec![0u16; (expanded_size*expanded_size) as usize];

        let mut xz_max_height_bounds = vec![None; (expanded_size*expanded_size) as usize];
        let mut min_height_bound = expanded_size;
        let mut max_height_bound = 0u32;

        for x in 0..expanded_size {
            for z in 0..expanded_size {
                if let Some(hb) = self.terrain_gen().fluid_height_existence_bound_test((ofs.0+x as i32) as f64, (ofs.2+z as i32) as f64) {
                    let hb = hb.ceil() as i32;
                    xz_max_height_bounds[(x*expanded_size+z) as usize] = Some(hb);
                    if hb-ofs.1+1 > max_height_bound as i32 {
                        max_height_bound = (hb-ofs.1+1).clamp(0i32, expanded_size as i32) as u32;
                    }
                    if hb-ofs.1 < min_height_bound as i32 {
                        min_height_bound = (hb-ofs.1).clamp(0i32, expanded_size as i32) as u32;
                    }
                }
            }
        }

        for x in 0..expanded_size {
            for z in 0..expanded_size {
                // let height = opaque_block_max_height_bounds((x_ofs+x as i32) as f64, (z_ofs+z as i32) as f64).ceil() as isize;
                // println!("??? {:?} {:?} = {:?}", x,z,  xz_max_height_bounds[(x*expanded_size+z) as usize]);
                if let Some(height) = xz_max_height_bounds[(x*expanded_size+z) as usize] {
                    // TODO
                    // for y == 0, set cells to start with closed
                    let mut xz_cell = &mut xz_grid[(x*expanded_size+z) as usize];
                    if *xz_cell%2 == 0 {
                        *xz_cell += 1;
                    }

                    for y in min_height_bound..max_height_bound {
                        let open = ofs.1+y as i32 >= height;
                        let mut xz_cell = &mut xz_grid[(x*expanded_size+z) as usize];

                        let lazy_block_gen = |dx: i32, dy: i32, dz: i32| {
                            self.terrain_gen().get_block((ofs.0+dx+x as i32) as f64, (ofs.1+dy+y as i32) as f64, (ofs.2+dz+z as i32) as f64)
                        };

                        let mut fast_face_gen = |
                            total_verts: &mut Vec<ChunkVertex>, total_inds: &mut Vec<u32>, total_faces: &mut u32,
                            dx, dy, dz, face_dir, txtr_mapping
                        | {
                                let (mut verts, mut inds) = self.gen_face(
                                    chunk_pos((x as i32+dx) as u32,(y as i32+dy) as u32,(z as i32+dz) as u32),
                                    *total_faces*4, face_dir, txtr_mapping, false
                                );
                                total_verts.append(&mut verts);
                                total_inds.append(&mut inds);
                                *total_faces += 1;
                        };

                        let mut fast_block_face_gen = |block, verts, inds, faces, dx: i32, dy: i32, dz: i32, face_dir| {
                            if let
                                BlockCullType::BorderVisible0(block) |
                                BlockCullType::BorderVisibleFluid0(block) |
                                BlockCullType::AlwaysVisible(block)
                                = &block
                            {
                                let block = self.block_ind(block.0 as usize);
                                let txtr = block.texture_id;

                                // mesh assumed to be (opaque) cube
                                fast_face_gen(verts, inds, faces, dx, dy, dz, face_dir, txtr);
                            }
                        };

                        if x < self.chunk_size() && z < self.chunk_size() {
                            if *xz_cell%2 == 1 && open {
                                // current hit cell is set to closed that needs to be opened using previous block index
                                *xz_cell += 1;

                                fast_block_face_gen(
                                    lazy_block_gen( 0,-1, 0),
                                    &mut translucent_verts, &mut translucent_inds, &mut translucent_faces,
                                    0, -1, 0, FaceDir::TOP
                                );
                            } else if *xz_cell%2 == 0 && !open {
                                // current hit cell is set to opened that needs to be closed using current block index
                                *xz_cell += 1;

                                if y > 0 {
                                    fast_block_face_gen(
                                        lazy_block_gen(0, 0, 0),
                                        &mut translucent_verts, &mut translucent_inds, &mut translucent_faces,
                                        0, 0, 0, FaceDir::BOTTOM
                                    );
                                }
                            }
                        }
                    }
                    xz_grid[(x*expanded_size+z) as usize] += 1;
                }
            }
        }

        (translucent_verts, translucent_inds)
    }

    fn block_culling(&self, voxel: &mut Box<[BlockCullType]>) {
        for x in 1..self.chunk_size()-1 {
            for y in 1..self.chunk_size()-1 {
                for z in 1..self.chunk_size()-1 {
                    match voxel[self.access(x,y,z)] {
                        BlockCullType::BorderVisible0(_) if
                        Self::check_block_obscured(voxel[self.access(x+1,y,z)]) &&
                            Self::check_block_obscured(voxel[self.access(x-1,y,z)]) &&
                            Self::check_block_obscured(voxel[self.access(x,y+1,z)]) &&
                            Self::check_block_obscured(voxel[self.access(x,y-1,z)]) &&
                            Self::check_block_obscured(voxel[self.access(x,y,z+1)]) &&
                            Self::check_block_obscured(voxel[self.access(x,y,z-1)]) => {
                            voxel[self.access(x,y,z)] = BlockCullType::Obscured;
                        }
                        BlockCullType::BorderVisibleFluid0(_) if
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

    fn check_coord_within_chunk(&self, x: i32, y: i32, z: i32) -> bool {
        0 <= x && x < self.chunk_size() as i32 && 0 <= y && y < self.chunk_size() as i32 && 0 <= z && z < self.chunk_size() as i32
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

