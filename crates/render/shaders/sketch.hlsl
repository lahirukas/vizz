#include "common.hlsli"

Texture2D prev_frame : register(t0);
SamplerState samp : register(s0);

float3 hsv(float h, float s, float v) {
    float3 k = saturate(abs(frac(h + float3(0.0, 2.0 / 3.0, 1.0 / 3.0)) * 6.0 - 3.0) - 1.0);
    return v * lerp(float3(1, 1, 1), k, s);
}

float seg_dist(float2 p, float2 a, float2 b) {
    float2 ab = b - a;
    float h = saturate(dot(p - a, ab) / max(dot(ab, ab), 1e-8));
    return length(p - a - ab * h);
}

float4 main(PSIn i) : SV_Target {
    float scale = min(res.x, res.y);
    float2 p = (i.uv - 0.5) * res / scale * 2.0 * float2(1, -1);
    float px = 2.0 / scale;
    float4 canvas = prev_frame.SampleLevel(samp, i.uv, 0);
    canvas *= exp(-dt * sketch_info.z);
    if (sketch_info.w > 0.5) {
        float d = seg_dist(p, sketch_pos.zw, sketch_pos.xy);
        float w = (0.006 + 0.01 * aux1) * (0.7 + 0.6 * pulse * drive);
        float core = smoothstep(w + px, w - px, d);
        float glow = exp(-d / (w * 3.0));
        float3 ink = hsv(sketch_info.y, 0.65, 1.0);
        canvas.rgb = max(canvas.rgb, ink * (core * 1.2 + glow * 0.45));
        canvas.a = max(canvas.a, saturate(core + glow * 0.5));
    }
    if (aux0 > 0.0) {
        float2 c = trail[7].xy;
        float age = 1.0 - aux0;
        float r = age * 1.1;
        float3 hot = lerp(hsv(sketch_info.y, 0.7, 1.0), float3(1, 1, 1), 0.4);
        float ring = exp(-abs(length(p - c) - r) / (px * 2.5)) * aux0;
        float sparks = 0;
        for (int k = 0; k < 16; k++) {
            float a = k * 0.3927 + frac(k * 0.618) * 0.4;
            float len = 0.6 + 0.5 * frac(k * 0.37);
            float2 sp = c + float2(cos(a), sin(a)) * r * len;
            sparks += exp(-length(p - sp) / (px * 3.0));
        }
        float3 burst = hot * (ring * 0.8 + sparks * 1.4) * aux0;
        canvas.rgb = max(canvas.rgb, burst);
        canvas.a = max(canvas.a, saturate(ring + sparks) * aux0);
    }
    return canvas;
}
