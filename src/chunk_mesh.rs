use uom::si::f32::Length;
use crate::component::camera::{Rotation, Length3D};
use crate::component::terrain::BlockGen;
use crate::measurement::chux;

pub(crate) struct ChunkMesh<P, V, I> {
    pos: Length3D,
    chunk_size: Length3D,

    chunk_generator: Box<dyn Fn(Length3D, Length3D) -> Box<[P]>>,
    chunks: Vec<Chunk<P>>,
    vertex_generator: Box<dyn Fn(&Vec<Chunk<P>>) -> (Vec<V>, Vec<I>)>,
}

impl<P, V, I> ChunkMesh<P, V, I> {
    pub(crate) fn new(pos: Length3D, size: Length3D,
                      chunk_generator: Box<dyn Fn(Length3D, Length3D) -> Box<[P]>>, vertex_generator: Box<dyn Fn(&Vec<Chunk<P>>) -> (Vec<V>, Vec<I>)>) -> Self {
        Self {
            pos,
            chunk_size: size,
            chunk_generator,
            chunks: Vec::new(),
            vertex_generator,
        }
    }

    pub(crate) fn update(&mut self, pos: Length3D) {
        self.pos = pos;
        let mut new_pos = pos;
        for cx in -2..2 {
            for cy in -2..2 {
                for cz in -2..2 {
                    self.load_chunk(Length3D::new(Length::new::<chux>(cx as f32),Length::new::<chux>(cy as f32),Length::new::<chux>(cz as f32)));
                }
            }
        }
    }

    fn load_chunk(&mut self, pos: Length3D) {
        self.chunks.push(Chunk::new(pos, (self.chunk_generator)(pos, self.chunk_size)))
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
    pub(crate) pos: Length3D,
}

impl<P> Chunk<P> {
    pub(crate) fn new(pos: Length3D, voxels: Box<[P]>) -> Self {
        Self {
            voxels,
            pos,
        }
    }
}
