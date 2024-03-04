use std::collections::HashMap;
use std::rc::Rc;
use criterion::{Criterion, criterion_group, criterion_main};
use matrixagon2::component::camera::Length3D;
use matrixagon2::component::terrain::{BlockData, MeshType, TextureMapper, TransparencyType};
use matrixagon2::component::terrain::chunk_gen_mf::ChunkGeneratorMF;
use matrixagon2::component::terrain::terrain_gen::TerrainGenerator;
use matrixagon2::chunk_mesh::{ChunkGeneratable, ChunkMesh, ChunkRadius, UpdateChunk};


const BLOCK_INDEX: [BlockData; 8] = [
    BlockData {
        ident: "grass_block",
        texture_id: TextureMapper::Lateral("grass_top", "dirt", "grass_side"),
        mesh: MeshType::Cube,
        transparency: TransparencyType::Opaque,
    },
    BlockData {
        ident: "dirt",
        texture_id: TextureMapper::All("dirt"),
        mesh: MeshType::Cube,
        transparency: TransparencyType::Opaque,
    },
    BlockData {
        ident: "stone",
        texture_id: TextureMapper::All("stone"),
        mesh: MeshType::Cube,
        transparency: TransparencyType::Opaque,
    },
    BlockData {
        ident: "sand",
        texture_id: TextureMapper::All("sand"),
        mesh: MeshType::Cube,
        transparency: TransparencyType::Opaque,
    },
    BlockData {
        ident: "grass",
        texture_id: TextureMapper::All("grass_flora"),
        mesh: MeshType::XCross,
        transparency: TransparencyType::Transparent,
    },
    BlockData {
        ident: "flower",
        texture_id: TextureMapper::All("flower"),
        mesh: MeshType::XCross,
        transparency: TransparencyType::Transparent,
    },
    BlockData {
        ident: "water",
        texture_id: TextureMapper::All("water"),
        mesh: MeshType::Fluid,
        transparency: TransparencyType::Translucent,
    },
    BlockData {
        ident: "air",
        texture_id: TextureMapper::All("null"),
        mesh: MeshType::Empty,
        transparency: TransparencyType::Transparent,
    },
];


pub fn benchmark_chunk_mesh_generation(c: &mut Criterion) {
    c.bench_function(
        "Single MF Chunk @(0,0,0) - Mesh Generation",
        |b| b.iter_with_large_drop(|| {
            let chunk_generator = ChunkGeneratorMF::new(
                Vec::from(BLOCK_INDEX),
                Rc::new(HashMap::from([
                    (String::from("grass_top"), 0),
                ])),
                Rc::new(TerrainGenerator::new())
            );
            chunk_generator.generate_mesh(Length3D::origin());
        })
    );
}

pub fn benchmark_chunk_aggregate_mesh(c: &mut Criterion) {
    c.bench_function(
        "Chunk Mesh Handler 4x4x4 MF - Mesh Generation & Aggregation",
        |b| b.iter_with_large_drop(|| {
            let mut chunk_mesh_mf = ChunkMesh::new(
                Length3D::origin(),
                ChunkRadius(2, 1), Some(ChunkRadius(2, 1)),
                ChunkGeneratorMF::new(
                    Vec::from(BLOCK_INDEX), Rc::new(HashMap::from([
                        (String::from("grass_top"), 0),
                    ])),
                    Rc::new(TerrainGenerator::new())
                ),
            );
            chunk_mesh_mf.update(UpdateChunk::Forced);
        })
    );
}

criterion_group!(
    name = benches;
    config = Criterion::default();
    targets = benchmark_chunk_mesh_generation
);
criterion_group!(
    name = benches_heavy;
    config = Criterion::default().sample_size(10);
    targets = benchmark_chunk_aggregate_mesh
);
criterion_main!(benches, benches_heavy);
