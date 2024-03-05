#version 450

layout (input_attachment_index = 0, binding = 0) uniform subpassInput inputColor;
layout (input_attachment_index = 1, binding = 1) uniform subpassInput inputDepth;

layout (location = 0) out vec4 outColor;

float scale(float x, float rmin, float rmax) {
    return (x - rmin) * 1.0 / (rmax - rmin);
}

void main()
{
    bool loadColor = false;

    if (loadColor) {
        outColor.rgb = subpassLoad(inputColor).rgb;
    } else {
        float depth = subpassLoad(inputDepth).r;
        outColor.rgb = vec3(scale(depth, 0.9, 1.0));
    }
}
