#version 450

layout(binding = 0) uniform UniformBufferObject {
    mat4 view;
    mat4 proj;
} ubo;

layout(location = 0) in vec3 position;
layout(location = 1) in vec2 uv;
layout(location = 2) in float ind;

layout(location = 0) out float out_ind;
layout(location = 1) out vec2 tex_coord;
layout(location = 2) out float calc_depth;

void main() {
    vec4 vert_pos = ubo.proj * ubo.view * vec4(position, 1.0);
    gl_Position = vert_pos;
    tex_coord = uv;
    out_ind = ind;
    calc_depth = vert_pos.z / vert_pos.w;
}
