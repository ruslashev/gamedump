#version 450

layout(push_constant) uniform PushConstants {
    mat4 inv;
    vec3 pos;
    vec2 res;
} consts;

layout(location = 0) out vec4 outColor;

// https://www.shadertoy.com/view/NlXXWN

vec3 hash33(vec3 p3) {
    p3 = fract(p3 * vec3(0.1031, 0.1030, 0.0973));
    p3 += dot(p3, p3.yxz + 33.33);
    return fract((p3.xxy + p3.yxx) * p3.zyx);
}

float starField(vec3 rd) {
    rd *= 150.0;

    float col = 0.0;

    for (int i = 0; i < 4; i++) {
        vec3 cellUVs = floor(rd + float(i * 1199));
        vec3 hash = (hash33(cellUVs) * 2.0 - 1.0) * 0.8;
        float hashMagnitude = 1.0 - length(hash);
        vec3 UVgrid = fract(rd) - 0.5;
        float radius = clamp(hashMagnitude - 0.5, 0.0, 1.0);
        float radialGradient = length(UVgrid - hash) / radius;
        radialGradient = clamp(1.0 - radialGradient, 0.0, 1.0);
        radialGradient *= radialGradient;
        col += radialGradient;
    }

    return col;
}

void main() {
    vec2 clip = 2.0 * gl_FragCoord.xy / consts.res - 1.0;
    vec4 pixel = vec4(clip, 1.0, 1.0);

    vec4 mult = consts.inv * pixel;

    mult /= mult.w;

    vec3 rayDir = normalize(mult.xyz - consts.pos);

    outColor = vec4(vec3(starField(rayDir)), 1.0);
}
