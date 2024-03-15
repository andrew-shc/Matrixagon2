# Matrixagon 2
An experimental open-world voxel renderer using Vulkan with focus on interesting novel techniques in computer graphics, reinforcement learning, and weather simulation.

![splash image](doc/splash_image.png)

## Status
Will take a break from implementing this project for now

## Current Features (Dec '23 - Feb '24)
### "Mesh Fill List Algorithm"
- Text
### Terrain Chunking
- Text
### Shaders
- Fluid temporal animation
- Renderpass macro (w/ Debug UI)

## TODO: Remaining MTXG2 Core Features needing to be implemented
(Will be put on hiatus)
- Rendering optimization
  - Useful frustum culling (instead of rebuilding mesh each time the player rotates)
    - Need refactoring in how shader handles individual chunk VBO
  - Facial culling (for far away regions that rarely gets shown on all six sides, remove sides that will never get seen until the camera approaches it closer)
  - Mesh culling/transformation
    - Floral mesh (e.g., grasses, flowers) should be removed at afar
    - Fluid mesh (e.g., water) should be transformed to opaque, static mesh at afar
  - Structural culling (significant/something I want to spend time on)
    - Some efficient way to reduce rendering loads while keeping the original mesh structure as much intact as possible
- Meshing optimization
  - Multithreaded chunk generation
- More realistic terrain
  - Overhangs/caves/etc. to make it more interesting vertically
    - 3D Perlin/Simplex?
    - Displacement map?
  - Macro-generation (i.e., continents)
  - Natural biome & land generation
    - Heightmap (multi-octave Perlin/Simplex noise)
    - Temporary & humidity map (multi-octave low frequency Perlin noise)
    - Tree placement map (Poisson disc distribution)
- Misc.
  - Enhance & fix Debug UI shader
    - Incorporate interactivity
      - Disable/enable components
      - Select a single-shader program from a dropdown
    - Make debug ui shader separate from the main shader
      - Will need to be on a separate renderpass
    - Fix the logical pixel size not correctly mapping
    - Proper aspect ratio values
  - Cleanup codebase for more flexible & extensible rendering application
    - Parameters for the main application (of the required components)
    - Extensible `enum WorldEvent` and `enum RenderDataPurpose`
  - Profiling
## Potential Future Plans on GPU Rendering
(They are very open goals on aspects that I'm purely interested in)
- MTXG2RL (Reinforcement Learning / Model experimentation on reinforcement learning agents)
- MTXG2GP (Graphics Programming / Formal experimentation on graphical techniques)
- MTXG2WS (Weather Simulation / Spatio(temporal?) experimentation on fluid, erosion, etc.)
