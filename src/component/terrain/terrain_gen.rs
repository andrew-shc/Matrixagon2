use noise::{NoiseFn, Perlin};
use crate::component::terrain::{Block, BlockCullType};



pub(super) struct TerrainGenerator {
    noise: Perlin,
    floral_noise: Perlin,
}

impl TerrainGenerator {
    const SEA_LEVEL: f64 = 10.0;
    const SAND_LEVEL: f64 = 13.0;

    pub(super) fn new() -> Self {
        Self {
            noise: Perlin::new(50), floral_noise: Perlin::new(23),
        }
    }

    pub(super) fn get_block(&self, x: f64, y: f64, z: f64) -> BlockCullType {
        let base_level = self.noise.get([x/20.0, z/20.0])*20.0+20.0;
        let floralness = self.floral_noise.get([x/40.0, z/40.0]);

        if y >= base_level+1.0 {
            if y <= Self::SEA_LEVEL {
                BlockCullType::BorderVisibleFluid0(Block(6))
            } else {
                BlockCullType::Empty
            }
        } else if y >= base_level {
            if y <= Self::SEA_LEVEL {
                BlockCullType::BorderVisibleFluid0(Block(6))
            } else if 0.8 <= floralness && floralness <= 0.9 {
                if 0.84 <= floralness && floralness <= 0.86 {
                    BlockCullType::AlwaysVisible(Block(5))
                } else {
                    BlockCullType::AlwaysVisible(Block(4))
                }
            } else {
                BlockCullType::Empty
            }
        } else if y <= Self::SAND_LEVEL {
            BlockCullType::BorderVisible0(Block(3))
        } else if y >= base_level-1.0 {
            BlockCullType::BorderVisible0(Block(0))
        } else if y >= base_level-3.0 {
            BlockCullType::BorderVisible0(Block(1))
        } else {
            BlockCullType::BorderVisible0(Block(2))
        }
    }

    // opaque block height-NBT
    pub(super) fn opaque_block_height_bound_test(&self, x: f64, z: f64) -> f64 {
        let base_level = self.noise.get([x/20.0, z/20.0])*20.0+20.0;

        base_level
    }

    // floral block placement-NBT

    // TODO: FLUID NBTs ARE TEMPORARY (FOR FUTURE BETTER FLUID GENERATION, RENDERING, & NEW SIM)
    // fluid block placement-NBT
    pub(super) fn fluid_height_existence_bound_test(&self, x: f64, z: f64) -> f64 {
        let base_level = self.noise.get([x/20.0, z/20.0])*20.0+20.0;

        base_level
    }
}
