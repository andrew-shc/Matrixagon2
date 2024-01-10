#version 450

layout(location = 0) in vec2 tex_coord;
layout(location = 1) in vec3 in_color;


layout(location = 0) out vec4 out_color;  // renderpass: color attachment #2

layout(set = 1, binding = 0) uniform sampler2D font_sampler;
layout(input_attachment_index = 0, set = 1, binding = 1) uniform subpassInput inputColor;

void main() {
//    out_color = vec4(subpassLoad(inputColor).rgb, 1.0);
//    out_color = vec4(1.0, 1.0, 0.0, 0.0);
//    out_color = vec4(in_color, 1.0);
    vec4 font_color = texture(font_sampler, tex_coord);
    vec3 solid_color = in_color;
    vec3 ui_color = mix(solid_color.rgb, font_color.rgb, 1-font_color.a);
    out_color = vec4(ui_color, 0.8);
//    out_color = vec4(mix(texture(ui_sampler, tex_coord).rgb, in_color, texture(ui_sampler, tex_coord).a), 0.5);
}
