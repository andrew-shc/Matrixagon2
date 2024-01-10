#version 450

layout(location = 0) in vec2 pos;
layout(location = 1) in vec2 uv;
layout(location = 2) in vec3 color;

layout(location = 0) out vec2 tex_coord;
layout(location = 1) out vec3 out_color;

void main() {
    gl_Position = vec4(pos.x*.002-1.0, pos.y*.002*1.6-1.0, 0.0, 1.0);
    tex_coord = uv;
    out_color = color;
}
