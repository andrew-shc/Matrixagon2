#version 450

layout(binding = 0) uniform UniformBufferObject {
    mat4 view;
    mat4 proj;
} ubo;

layout(location = 0) in vec3 position;
layout(location = 1) in vec2 uv;

layout(location = 1) out vec2 tex_coord;
//layout(location = 0) out vec3 DEBUG_pos;

void main() {
    gl_Position = ubo.proj * ubo.view * vec4(position, 1.0);
//    gl_Position = vec4(position, 1.0);
    tex_coord = uv;
//    DEBUG_pos = position;
}
