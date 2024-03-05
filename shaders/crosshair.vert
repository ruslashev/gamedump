#version 450

layout(push_constant) uniform PushConstants {
    vec3 color;
    vec2 screenRes;
} consts;

layout(location = 0) in vec2 inPosition;

void main() {
    vec2 pos = inPosition + consts.screenRes / 2.0;
    vec2 ndc = 2.0 * pos / consts.screenRes - 1.0;

    gl_Position = vec4(ndc, 0.0, 1.0);
}
