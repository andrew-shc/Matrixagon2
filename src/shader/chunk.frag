#version 450

layout(location = 0) in float txtr_ind;
layout(location = 1) in vec2 tex_coord;

layout(location = 0) out vec4 out_color;  // renderpass: color attachment #0

layout(binding = 1) uniform sampler2DArray tex_sampler;

void main() {
    out_color = texture(tex_sampler, vec3(tex_coord, txtr_ind));
}
