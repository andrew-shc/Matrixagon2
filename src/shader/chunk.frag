#version 450

layout(location = 1) in vec2 tex_coord;
//layout(location = 0) in vec3 DEBUG_pos;

layout(location = 0) out vec4 out_color;  // renderpass: color attachment #0

layout(binding = 1) uniform sampler2D tex_sampler;

//layout (depth_less) out float gl_FragDepth;

void main() {
    out_color = texture(tex_sampler, tex_coord);
//    gl_FragDepth = DEBUG_pos.z/2;
}
