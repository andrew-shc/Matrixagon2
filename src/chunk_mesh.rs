use std::collections::HashMap;
use uom::fmt::DisplayStyle;
use uom::si::f32::Length;
use crate::component::camera::{Rotation, Length3D};
use crate::component::terrain::{BlockGen, FaceDir};
use crate::measurement::chux;


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

pub(crate) struct ChunkMesh<P, V, I> {
    pub(crate) central_pos: Length3D,
    chunk_size: Length3D,
    chunk_radius: i32,

    chunk_generator: Box<dyn Fn(Length3D, Length3D) -> Box<[P]>>,
    // chunks: Vec<Chunk<P>>,
    // chunk_adjacency: Vec<ChunkAdjaceny>,
    chunks: HashMap<ChunkPosition, Chunk<P>>,
    chunk_adjacency: Vec<ChunkAdjacency>,
    vertex_generator: Box<dyn Fn(&HashMap<ChunkPosition, Chunk<P>>) -> (Vec<V>, Vec<I>)>,
}

impl<P, V, I> ChunkMesh<P, V, I> {
    pub(crate) fn new(pos: Length3D, size: Length3D, chunk_radius: u32,
                      chunk_generator: Box<dyn Fn(Length3D, Length3D) -> Box<[P]>>,
                      vertex_generator: Box<dyn Fn(&HashMap<ChunkPosition, Chunk<P>>) -> (Vec<V>, Vec<I>)>
    ) -> Self {
        Self {
            central_pos: pos,
            chunk_size: size,
            chunk_radius: chunk_radius as i32,
            chunk_generator,
            chunks: HashMap::new(),
            chunk_adjacency: Vec::new(),
            vertex_generator,
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

    pub(crate) fn update(&mut self, pos: Length3D) -> bool {
        let mut pos_changed = false;
        let mut chunk_changed = false;

        if pos.x.floor::<chux>() < self.central_pos.x-Length::new::<chux>(1.0) {
            println!("NEW CHUNK LOAD -X");
            self.central_pos.x -= Length::new::<chux>(1.0);
            pos_changed = true;
        } else if self.central_pos.x < pos.x.floor::<chux>() {
            println!("NEW CHUNK LOAD +X");
            self.central_pos.x += Length::new::<chux>(1.0);
            pos_changed = true;
        }
        if pos.y.floor::<chux>() < self.central_pos.y-Length::new::<chux>(1.0) {
            println!("NEW CHUNK LOAD -Y");
            self.central_pos.y -= Length::new::<chux>(1.0);
            pos_changed = true;
        } else if self.central_pos.y < pos.y.floor::<chux>() {
            println!("NEW CHUNK LOAD +Y");
            self.central_pos.y += Length::new::<chux>(1.0);
            pos_changed = true;
        }
        if pos.z.floor::<chux>() < self.central_pos.z-Length::new::<chux>(1.0) {
            println!("NEW CHUNK LOAD -Z");
            self.central_pos.z -= Length::new::<chux>(1.0);
            pos_changed = true;
        } else if self.central_pos.z < pos.z.floor::<chux>() {
            println!("NEW CHUNK LOAD +Z");
            self.central_pos.z += Length::new::<chux>(1.0);
            pos_changed = true;
        }

        if pos_changed {  // chunk position changed, update what chunks needs to be loaded
            // println!("CENTRAL POS {:?}", self.central_pos);
            for cx in -self.chunk_radius..self.chunk_radius {
                for cy in -self.chunk_radius..self.chunk_radius {
                    for cz in -self.chunk_radius..self.chunk_radius {
                        let new_chunk_pos = Length3D::new(
                            Length::new::<chux>(cx as f32)+self.central_pos.x,
                            Length::new::<chux>(cy as f32)+self.central_pos.y,
                            Length::new::<chux>(cz as f32)+self.central_pos.z,
                        );

                        // let mut chunk_exists = false;
                        // for chunk in &self.chunks {
                        //     // f chunk.pos.x.get::<chux>().floor() != new_chunk_pos.x.get::<chux>().floor() ||
                        //     //     chunk.pos.y.get::<chux>().floor() != new_chunk_pos.y.get::<chux>().floor() ||
                        //     //     chunk.pos.z.get::<chux>().floor() != new_chunk_pos.z.get::<chux>().floor() {
                        //     if chunk.pos.x == new_chunk_pos.x &&
                        //         chunk.pos.y == new_chunk_pos.y &&
                        //         chunk.pos.z == new_chunk_pos.z {
                        //         chunk_exists = true;
                        //     }
                        // }
                        if let None = self.chunks.get(&ChunkPosition::from(new_chunk_pos)) {
                            // chunk at new_chunk_pos does not exist (needs to be created)

                            println!("New chunk loaded [{} {} {}]",
                                     new_chunk_pos.x.into_format_args(chux, DisplayStyle::Abbreviation),
                                     new_chunk_pos.y.into_format_args(chux, DisplayStyle::Abbreviation),
                                     new_chunk_pos.z.into_format_args(chux, DisplayStyle::Abbreviation),
                            );
                            self.load_chunk(new_chunk_pos);
                            chunk_changed = true;
                        }
                        // if !chunk_exists {
                        //     println!("New chunk loaded [{} {} {}]",
                        //              new_chunk_pos.x.into_format_args(chux, DisplayStyle::Abbreviation),
                        //              new_chunk_pos.y.into_format_args(chux, DisplayStyle::Abbreviation),
                        //              new_chunk_pos.z.into_format_args(chux, DisplayStyle::Abbreviation),
                        //     );
                        //     self.load_chunk(new_chunk_pos);
                        //     chunk_changed = true;
                        // }
                    }
                }
            }

            chunk_changed
        } else {
            false
        }
    }

    fn load_chunk(&mut self, pos: Length3D) {
        let hash_pos = ChunkPosition::from(pos);
        println!("LOAD CHUNK / HASH POS {:?}", hash_pos);

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
            hash_pos, Chunk::new(pos, hash_pos, (self.chunk_generator)(pos, self.chunk_size), adj)
        );
    }

    fn unload_chunk(&mut self) {

    }

    // generate the entire aggregated vertices/indices
    pub(crate) fn generate_vertices(&mut self) -> (Vec<V>, Vec<I>) {
        (self.vertex_generator)(&self.chunks)
    }
}


pub(crate) struct Chunk<P> {
    pub(crate) voxels: Box<[P]>,
    pub(crate) pos: Length3D,  // south-west corner of the chunk TODO
    pub(crate) hash_pos: ChunkPosition,
    pub(crate) adjacency: ChunkAdjacency,
}

impl<P> Chunk<P> {
    pub(crate) fn new(pos: Length3D, hash_pos: ChunkPosition, voxels: Box<[P]>, init_adjs: ChunkAdjacency) -> Self {
        Self {
            voxels, pos, hash_pos, adjacency: init_adjs,
        }
    }
}
