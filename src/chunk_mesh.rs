use crate::component::camera::{Rotation, Translation};
use crate::component::terrain::BlockGen;

pub(crate) struct ChunkMesh<P, V, I> {
    pos: Translation,
    chunk_size: Translation,

    chunk_generator: Box<dyn Fn(Translation, Translation) -> Box<[P]>>,
    chunks: Vec<Chunk<P>>,
    vertex_generator: Box<dyn Fn(&Vec<Chunk<P>>) -> (Vec<V>, Vec<I>)>,
}

impl<P, V, I> ChunkMesh<P, V, I> {
    pub(crate) fn new(pos: Translation, size: Translation,
                      chunk_generator: Box<dyn Fn(Translation, Translation) -> Box<[P]>>, vertex_generator: Box<dyn Fn(&Vec<Chunk<P>>) -> (Vec<V>, Vec<I>)>) -> Self {
        Self {
            pos,
            chunk_size: size,
            chunk_generator,
            chunks: Vec::new(),
            vertex_generator,
        }
    }

    pub(crate) fn update(&mut self, pos: Translation) {
        self.pos = pos;
        self.load_chunk(pos);
    }

    fn load_chunk(&mut self, pos: Translation) {
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
    pub(crate) pos: Translation,
}

impl<P> Chunk<P> {
    pub(crate) fn new(pos: Translation, voxels: Box<[P]>) -> Self {
        Self {
            voxels,
            pos,
        }
    }
}
