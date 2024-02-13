use std::collections::HashMap;
use uom::si::f32::Length;
use crate::component::camera::{Length3D};
use crate::component::RenderDataPurpose;
use crate::measurement::chux;


pub(crate) trait ChunkGeneratable {
    type P;
    type V;
    type I;
    fn generate_chunk(&self, pos: Length3D) -> Box<[Self::P]>;
    fn generate_mesh(&self, chunks: &HashMap<ChunkPosition, Chunk<Self::P>>) -> Vec<(Vec<Self::V>, Vec<Self::I>, RenderDataPurpose)>;
}


#[derive(Copy, Clone, Default)]
pub(crate) struct ChunkAdjacency {
    pub(crate) top: Option<ChunkPosition>,
    pub(crate) bottom: Option<ChunkPosition>,
    pub(crate) left: Option<ChunkPosition>,
    pub(crate) right: Option<ChunkPosition>,
    pub(crate) front: Option<ChunkPosition>,
    pub(crate) back: Option<ChunkPosition>,
}

#[derive(Copy, Clone, Hash, Debug, Eq, PartialEq)]
pub(crate) struct ChunkPosition {
    x: isize, y: isize, z: isize,
}

impl From<Length3D> for ChunkPosition {
    fn from(value: Length3D) -> Self {
        Self {
            x: value.x.floor::<chux>().get::<chux>() as isize,
            y: value.y.floor::<chux>().get::<chux>() as isize,
            z: value.z.floor::<chux>().get::<chux>() as isize,
        }
    }
}

impl ChunkPosition {
    fn top(self) -> Self { Self { x: self.x, y: self.y+1, z: self.z } }
    fn bottom(self) -> Self { Self { x: self.x, y: self.y-1, z: self.z } }
    fn left(self) -> Self { Self { x: self.x-1, y: self.y, z: self.z } }
    fn right(self) -> Self { Self { x: self.x+1, y: self.y, z: self.z } }
    fn front(self) -> Self { Self { x: self.x, y: self.y, z: self.z+1 } }
    fn back(self) -> Self { Self { x: self.x, y: self.y, z: self.z-1 } }
}

pub(crate) struct ChunkMesh<G: ChunkGeneratable> {
    pub(crate) central_pos: Length3D,
    chunk_size: Length3D,
    chunk_radius: i32,  // border rendering radius
    chunk_update_radius: f32,  // inner updating radius (when the player reaches beyond it, it will update the chunk)
    // inner radius should be more than 0, or else it will keep updating and rebuilding mesh (disaster) and quite useless too

    generator: G,
    chunks: HashMap<ChunkPosition, Chunk<G::P>>,
    chunk_adjacency: Vec<ChunkAdjacency>,
}

impl<G: ChunkGeneratable> ChunkMesh<G> {
    pub(crate) fn new(pos: Length3D, size: Length3D, chunk_radius: u32, inner_radius: u32, generator: G) -> Self {
        Self {
            central_pos: pos,
            chunk_size: size,
            chunk_radius: chunk_radius as i32,
            chunk_update_radius: inner_radius as f32,
            generator,
            chunks: HashMap::new(),
            chunk_adjacency: Vec::new(),
        }
    }

    pub(crate) fn initialize(&mut self) {
        for cx in -self.chunk_radius..self.chunk_radius {
            for cy in -self.chunk_radius..self.chunk_radius {
                for cz in -self.chunk_radius..self.chunk_radius {
                    println!("Initial chunk load [{cx} {cy} {cz}]");
                    self.load_chunk(Length3D::new(
                        Length::new::<chux>(cx as f32)+self.central_pos.x.floor::<chux>(),
                        Length::new::<chux>(cy as f32)+self.central_pos.y.floor::<chux>(),
                        Length::new::<chux>(cz as f32)+self.central_pos.z.floor::<chux>(),
                    ));
                }
            }
        }
    }

    pub(crate) fn swap_generator(&mut self, generator: G) {
        self.generator = generator;
    }

    pub(crate) fn update(&mut self, pos: Length3D) -> bool {
        let mut pos_changed = false;
        let mut chunk_changed = false;

        if pos.x.floor::<chux>() < self.central_pos.x-Length::new::<chux>(self.chunk_update_radius) {
            // println!("NEW CHUNK LOAD -X");
            self.central_pos.x -= Length::new::<chux>(1.0);
            pos_changed = true;
        } else if self.central_pos.x+Length::new::<chux>(self.chunk_update_radius-1.0) < pos.x.floor::<chux>() {
            // println!("NEW CHUNK LOAD +X");
            self.central_pos.x += Length::new::<chux>(1.0);
            pos_changed = true;
        }
        if pos.y.floor::<chux>() < self.central_pos.y-Length::new::<chux>(self.chunk_update_radius) {
            // println!("NEW CHUNK LOAD -Y");
            self.central_pos.y -= Length::new::<chux>(1.0);
            pos_changed = true;
        } else if self.central_pos.y+Length::new::<chux>(self.chunk_update_radius-1.0) < pos.y.floor::<chux>() {
            // println!("NEW CHUNK LOAD +Y");
            self.central_pos.y += Length::new::<chux>(1.0);
            pos_changed = true;
        }
        if pos.z.floor::<chux>() < self.central_pos.z-Length::new::<chux>(self.chunk_update_radius) {
            // println!("NEW CHUNK LOAD -Z");
            self.central_pos.z -= Length::new::<chux>(1.0);
            pos_changed = true;
        } else if self.central_pos.z+Length::new::<chux>(self.chunk_update_radius-1.0) < pos.z.floor::<chux>() {
            // println!("NEW CHUNK LOAD +Z");
            self.central_pos.z += Length::new::<chux>(1.0);
            pos_changed = true;
        }

        if pos_changed {  // chunk position changed, update what chunks needs to be loaded
            // println!("CENTRAL POS {:?}", self.central_pos);
            self.reset_chunk_visibility();

            for cx in -self.chunk_radius..self.chunk_radius {
                for cy in -self.chunk_radius..self.chunk_radius {
                    for cz in -self.chunk_radius..self.chunk_radius {
                        let new_chunk_pos = Length3D::new(
                            Length::new::<chux>(cx as f32)+self.central_pos.x,
                            Length::new::<chux>(cy as f32)+self.central_pos.y,
                            Length::new::<chux>(cz as f32)+self.central_pos.z,
                        );

                        if let Some(chunk) = self.chunks.get_mut(&ChunkPosition::from(new_chunk_pos)) {
                            chunk.visible = true;
                            chunk_changed = true;
                        } else {
                            // chunk at new_chunk_pos does not exist (needs to be created)

                            // println!("New chunk loaded [{} {} {}]",
                            //          new_chunk_pos.x.into_format_args(chux, DisplayStyle::Abbreviation),
                            //          new_chunk_pos.y.into_format_args(chux, DisplayStyle::Abbreviation),
                            //          new_chunk_pos.z.into_format_args(chux, DisplayStyle::Abbreviation),
                            // );
                            self.load_chunk(new_chunk_pos);
                            chunk_changed = true;
                        }
                    }
                }
            }

            chunk_changed
        } else {
            false
        }
    }

    fn reset_chunk_visibility(&mut self) {
        for v in self.chunks.values_mut() {
            v.visible = false;
        }
    }

    fn load_chunk(&mut self, pos: Length3D) {
        let hash_pos = ChunkPosition::from(pos);
        // println!("LOAD CHUNK / HASH POS {:?}", hash_pos);

        let mut adj = ChunkAdjacency::default();
        if let Some(c) = self.chunks.get_mut(&hash_pos.top()) {
            adj.top.replace(c.hash_pos);
            c.adjacency.bottom.replace(hash_pos);
        }
        if let Some(c) = self.chunks.get_mut(&hash_pos.bottom()) {
            adj.bottom.replace(c.hash_pos);
            c.adjacency.top.replace(hash_pos);
        }
        if let Some(c) = self.chunks.get_mut(&hash_pos.left()) {
            adj.left.replace(c.hash_pos);
            c.adjacency.right.replace(hash_pos);
        }
        if let Some(c) = self.chunks.get_mut(&hash_pos.right()) {
            adj.right.replace(c.hash_pos);
            c.adjacency.left.replace(hash_pos);
        }
        if let Some(c) = self.chunks.get_mut(&hash_pos.front()) {
            adj.front.replace(c.hash_pos);
            c.adjacency.back.replace(hash_pos);
        }
        if let Some(c) = self.chunks.get_mut(&hash_pos.back()) {
            adj.back.replace(c.hash_pos);
            c.adjacency.front.replace(hash_pos);
        }

        self.chunks.insert(
            hash_pos, Chunk::new(pos, hash_pos, self.generator.generate_chunk(pos), adj)
        );
    }

    fn unload_chunk(&mut self) {

    }

    // generate the entire aggregated vertices/indices
    pub(crate) fn generate_vertices(&mut self) -> Vec<(Vec<G::V>, Vec<G::I>, RenderDataPurpose)> {
        self.generator.generate_mesh(&self.chunks)
    }
}


pub(crate) struct Chunk<P> {
    pub(crate) voxels: Box<[P]>,
    pub(crate) pos: Length3D,  // south-west corner of the chunk TODO
    pub(crate) hash_pos: ChunkPosition,
    pub(crate) adjacency: ChunkAdjacency,
    visible: bool,
}

impl<P> Chunk<P> {
    pub(crate) fn new(pos: Length3D, hash_pos: ChunkPosition, voxels: Box<[P]>, init_adjs: ChunkAdjacency) -> Self {
        Self {
            voxels, pos, visible: true, hash_pos, adjacency: init_adjs,
        }
    }

    pub(crate) fn visible(&self) -> bool {self.visible}
}
