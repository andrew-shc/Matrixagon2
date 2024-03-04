use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::marker::PhantomData;
use uom::num_traits::Float;
use uom::si::f32::Length;
use uom::si::Unit;
use crate::component::camera::{Length3D};
use crate::component::RenderDataPurpose;
use crate::component::terrain::FaceDir;


pub(crate) trait BlockLengthUnit: uom::si::length::Unit + uom::Conversion<f32, T = f32> {}
impl<T> BlockLengthUnit for T where T: uom::si::length::Unit + uom::Conversion<f32, T = f32> {}


pub trait ChunkGeneratable {
    type A: BlockLengthUnit;  // border outer radius
    type B: BlockLengthUnit;  // empty inner radius
    type V;
    type I;
    fn generate_mesh(&self, pos: Length3D) -> Vec<(Vec<Self::V>, Vec<Self::I>, Option<FaceDir>, RenderDataPurpose)>;
    fn aggregate_mesh(&self, central_pos: Length3D, chunks: &HashMap<Position<Self::B>, Chunk<Self::V, Self::I, Self::B>>)
        -> Vec<(Vec<Self::V>, Vec<Self::I>, RenderDataPurpose)>;
}


#[derive(Copy, Clone)]
pub(crate) struct ChunkAdjacency<M: BlockLengthUnit> {
    pub(crate) top: Option<Position<M>>,
    pub(crate) bottom: Option<Position<M>>,
    pub(crate) left: Option<Position<M>>,
    pub(crate) right: Option<Position<M>>,
    pub(crate) front: Option<Position<M>>,
    pub(crate) back: Option<Position<M>>,
}
impl<M: BlockLengthUnit> Default for ChunkAdjacency<M> {
    fn default() -> Self {Self {top: None, bottom: None, left: None, right: None, front: None, back: None}}
}

#[derive(Copy, Clone, Debug)]
pub(crate) struct Position<M: BlockLengthUnit> {
    pub(crate) x: isize, pub(crate) y: isize, pub(crate) z: isize, _measure: PhantomData<M>,
}

impl<M: BlockLengthUnit> PartialEq<Self> for Position<M> {
    fn eq(&self, other: &Self) -> bool {self.x == other.x && self.y == other.y && self.z == other.z}
}
impl<M: BlockLengthUnit> Eq for Position<M> {}
impl<M: BlockLengthUnit> Hash for Position<M> {
    fn hash<H: Hasher>(&self, state: &mut H) {self.x.hash(state); self.y.hash(state); self.z.hash(state);}
}
impl<M: BlockLengthUnit> Default for Position<M> {
    fn default() -> Self {Self {x: isize::default(), y: isize::default(), z: isize::default(), _measure: PhantomData}}
}


impl<M: BlockLengthUnit> From<Length3D> for Position<M> {
    fn from(value: Length3D) -> Self {
        Self {
            x: value.x.floor::<M>().get::<M>() as isize,
            y: value.y.floor::<M>().get::<M>() as isize,
            z: value.z.floor::<M>().get::<M>() as isize,
            _measure: PhantomData,
        }
    }
}

impl<M: BlockLengthUnit> Position<M> {
    fn top(self) -> Self { Self { x: self.x, y: self.y+1, z: self.z, _measure: PhantomData} }
    fn bottom(self) -> Self { Self { x: self.x, y: self.y-1, z: self.z, _measure: PhantomData } }
    fn left(self) -> Self { Self { x: self.x-1, y: self.y, z: self.z, _measure: PhantomData } }
    fn right(self) -> Self { Self { x: self.x+1, y: self.y, z: self.z, _measure: PhantomData } }
    fn front(self) -> Self { Self { x: self.x, y: self.y, z: self.z+1, _measure: PhantomData } }
    fn back(self) -> Self { Self { x: self.x, y: self.y, z: self.z-1, _measure: PhantomData } }
}


pub enum UpdateChunk {
    NewPos(Length3D),
    Forced
}

// border_radius, update_radius
#[derive(Copy, Clone)]
pub struct ChunkRadius(pub u32, pub u32);


pub struct ChunkMesh<G: ChunkGeneratable> {
    pub(crate) central_pos: Length3D,
    inner_central_pos: Length3D,
    chunk_size: Length3D,
    chunk_outer_radius: i32,  // border rendering radius
    chunk_outer_update_radius: f32,  // inner updating radius (when the player reaches beyond it, it will update the chunk)
    subchunk_outer_radius: i32,  // border rendering radius (in subchunk unit)
    chunk_inner_radius: Option<f32>,  // inside rendering radius that should not be rendered
    // inner radius should be more than 0, or else it will keep updating and rebuilding mesh (disaster) and quite useless too
    chunk_inner_update_radius: Option<f32>,

    generator: G,
    chunks: HashMap<Position<G::B>, Chunk<G::V, G::I, G::B>>,
    chunk_adjacency: Vec<ChunkAdjacency<G::B>>,
}

impl<G: ChunkGeneratable> ChunkMesh<G> {
    pub fn new(pos: Length3D, outer: ChunkRadius, inner: Option<ChunkRadius>, generator: G) -> Self {
        Self {
            central_pos: pos,
            inner_central_pos: pos,
            chunk_size: Length3D {
                x: Length::new::<G::B>(1.0),
                y: Length::new::<G::B>(1.0),
                z: Length::new::<G::B>(1.0),
            },
            chunk_outer_radius: outer.0 as i32,
            chunk_outer_update_radius: outer.1 as f32,
            subchunk_outer_radius: Length::new::<G::A>(outer.0 as f32).get::<G::B>() as i32,
            chunk_inner_radius: inner.map(| ChunkRadius(border, _) | border as f32),
            chunk_inner_update_radius: inner.map(| ChunkRadius(_, update) | update as f32),
            generator,
            chunks: HashMap::new(),
            chunk_adjacency: Vec::new(),
        }
    }

    pub(crate) fn swap_generator(&mut self, generator: G) {
        self.generator = generator;
    }

    pub fn update(&mut self, mode: UpdateChunk) -> bool {
        let mut pos_changed = false;
        let mut inner_chunk_update = false;
        let mut chunk_changed = false;

        if let UpdateChunk::NewPos(pos) = mode {
            // INNER RADIUS CHUNK UPDATE

            if let Some(chunk_inner_update_radius) = self.chunk_inner_update_radius {
                inner_chunk_update |= Self::check_and_update_axis::<G::B>(&mut self.inner_central_pos.x, &pos.x, chunk_inner_update_radius);
                inner_chunk_update |= Self::check_and_update_axis::<G::B>(&mut self.inner_central_pos.y, &pos.y, chunk_inner_update_radius);
                inner_chunk_update |= Self::check_and_update_axis::<G::B>(&mut self.inner_central_pos.z, &pos.z, chunk_inner_update_radius);
            }

            // BORDER RADIUS CHUNK UPDATE

            pos_changed |= Self::check_and_update_axis::<G::A>(&mut self.central_pos.x, &pos.x, self.chunk_outer_update_radius);
            pos_changed |= Self::check_and_update_axis::<G::A>(&mut self.central_pos.y, &pos.y, self.chunk_outer_update_radius);
            pos_changed |= Self::check_and_update_axis::<G::A>(&mut self.central_pos.z, &pos.z, self.chunk_outer_update_radius);
        } else {
            inner_chunk_update = true;
            pos_changed = true;
        }

        if pos_changed || inner_chunk_update {  // chunk position changed, update what chunks needs to be loaded
            // println!("CENTRAL POS {:?}", self.central_pos);
            if pos_changed {
                self.reset_chunk_visibility();
            }

            for cx in -self.subchunk_outer_radius..self.subchunk_outer_radius {
                for cy in -self.subchunk_outer_radius..self.subchunk_outer_radius {
                    for cz in -self.subchunk_outer_radius..self.subchunk_outer_radius {
                        let chunk_pos = Length3D::new(
                            Length::new::<G::B>(cx as f32)+self.central_pos.x,
                            Length::new::<G::B>(cy as f32)+self.central_pos.y,
                            Length::new::<G::B>(cz as f32)+self.central_pos.z,
                        );

                        if pos_changed {

                            if let Some(chunk) = self.chunks.get_mut(&Position::from(chunk_pos)) {
                                if !chunk.visible {
                                    chunk.visible = true;
                                    chunk_changed = true;
                                }
                            } else {
                                // chunk at new_chunk_pos does not exist (needs to be created)

                                if chunk_pos.x.get::<G::A>() % 1.0 == 0.0 &&
                                    chunk_pos.y.get::<G::A>() % 2.0 == 0.0 &&
                                    chunk_pos.z.get::<G::A>() % 2.0 == 0.0 {
                                    println!("New chunk loaded [{} {} {} <{}>]",
                                             chunk_pos.x.get::<G::A>(),
                                             chunk_pos.y.get::<G::A>(),
                                             chunk_pos.z.get::<G::A>(),
                                             G::A::abbreviation(),
                                    );
                                }

                                self.load_chunk(chunk_pos);
                                chunk_changed = true;
                            }
                        }
                        // in the niche case when forced to start, inner chunk sets EXISTING inner chunks to invisible
                        //  hence, it needs to be after it is generated only in this niche case
                        if inner_chunk_update {
                            if let Some(chunk) = self.chunks.get_mut(&Position::from(chunk_pos)) {
                                if Self::check_inside_radius::<G::B>(&self.inner_central_pos.x, &chunk.pos.x, self.chunk_inner_radius) &&
                                    Self::check_inside_radius::<G::B>(&self.inner_central_pos.y, &chunk.pos.y, self.chunk_inner_radius) &&
                                    Self::check_inside_radius::<G::B>(&self.inner_central_pos.z, &chunk.pos.z, self.chunk_inner_radius) {
                                    // chunk inside inner radius (needs to be 'removed')
                                    chunk.visible = false;
                                    chunk_changed = true;
                                } else if !chunk.visible {
                                    // not visible but not inside the inner radius? (needs to be added again)
                                    // assumes all the chunks are generated since inner radius is inside the border radius
                                    //  where all the chunk generation happens
                                    // if not, it will just result in empty chunks somehow inside rest of the chunks
                                    chunk.visible = true;
                                    chunk_changed = true;
                                }
                            }
                        }
                    }
                }
            }

            if pos_changed {
                println!("CHUNK NEED UPDATE: BORDER");
            }
            if inner_chunk_update {
                println!("CHUNK NEED UPDATE: INNER");
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
        let hash_pos = Position::from(pos);
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

        let mesh = self.generator.generate_mesh(pos);

        self.chunks.insert(
            hash_pos, Chunk::new(pos, hash_pos, adj, mesh)
        );
    }

    fn unload_chunk(&mut self) {

    }

    // generate the entire aggregated vertices/indices
    pub(crate) fn generate_vertices(&mut self) -> Vec<(Vec<G::V>, Vec<G::I>, RenderDataPurpose)> {
        self.generator.aggregate_mesh(self.central_pos, &self.chunks)
    }

    // checks outward
    fn check_and_update_axis<M: BlockLengthUnit>(central_chunk_axis: &mut Length, new_point_axis: &Length, update_radius: f32) -> bool {
        if new_point_axis.floor::<M>() < *central_chunk_axis-Length::new::<M>(update_radius) {
            *central_chunk_axis -= Length::new::<M>(1.0);
            true
        } else if *central_chunk_axis+Length::new::<G::B>(update_radius-1.0) < new_point_axis.floor::<M>() {
            *central_chunk_axis += Length::new::<M>(1.0);
            true
        } else {
            false
        }
    }

    // checks inward
    fn check_inside_radius<M: BlockLengthUnit>(central_chunk_axis: &Length, existing_chunk_axis: &Length, chunk_radius: Option<f32>) -> bool {
        if let Some(chunk_radius) = chunk_radius {
            *central_chunk_axis-Length::new::<M>(chunk_radius+1.0) < *existing_chunk_axis &&
                *existing_chunk_axis < *central_chunk_axis+Length::new::<M>(chunk_radius)
        } else {
            false
        }
    }
}


pub(crate) struct Chunk<V, I, M: BlockLengthUnit> {
    pub(crate) pos: Length3D,  // south-west corner of the chunk TODO
    pub(crate) hash_pos: Position<M>,
    pub(crate) adjacency: ChunkAdjacency<M>,
    pub(crate) mesh: Vec<(Vec<V>, Vec<I>, Option<FaceDir>, RenderDataPurpose)>,
    visible: bool,
}

impl<V, I, M: BlockLengthUnit> Chunk<V, I, M> {
    pub(crate) fn new(
        pos: Length3D,
        hash_pos: Position<M>,
        init_adjs: ChunkAdjacency<M>,
        mesh: Vec<(Vec<V>, Vec<I>, Option<FaceDir>, RenderDataPurpose)>,
    ) -> Self {
        Self {
            pos, hash_pos, adjacency: init_adjs, mesh, visible: true,
        }
    }

    pub(crate) fn visible(&self) -> bool {self.visible}
}
