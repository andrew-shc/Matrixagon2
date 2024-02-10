#version 450

layout(set = 0, binding = 0) uniform UniformBufferObject {
    mat4 view;
    mat4 proj;
} mvp;

layout(location = 0) in vec3 position;
layout(location = 1) in vec2 uv;
layout(location = 2) in float ind;

layout(location = 0) out float out_ind;
layout(location = 1) out vec2 tex_coord;

void main() {
    gl_Position = mvp.proj * mvp.view * vec4(position, 1.0);
    tex_coord = uv;
    out_ind = ind;
}
