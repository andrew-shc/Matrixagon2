use matrixagon2::debug::DebugVisibility;
use matrixagon2::MatrixagonApp;

fn main() {
    let debug_visibility = DebugVisibility {
        vk_setup_output: true,
        vk_swapchain_output: false,
        mtxg_output: true,
        mtxg_render_output: false,
    };
    let mtxg = MatrixagonApp::init(true, debug_visibility, true, true);
    // mtxg.load_shader(StandardRasterizer::new());
    mtxg.run();
}
