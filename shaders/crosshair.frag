#version 450

layout(push_constant) uniform PushConstants {
    vec3 color;
    vec2 screenRes;
} consts;

layout(location = 0) out vec4 outColor;

void main() {
    outColor = vec4(consts.color, 1.0);
}
