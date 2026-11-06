#include "common.hlsli"

Texture2D frame : register(t0);
SamplerState samp : register(s0);

float2 fold(int mode, float2 uv, float aspect) {
    float2 p = (uv - 0.5) * float2(aspect, 1.0);
    if (mode == 1) p.x = abs(p.x);
    else if (mode == 2) p = abs(p);
    else if (mode == 3) {
        float a = atan2(p.y, p.x);
        float r = length(p);
        float seg = TAU / 6.0;
        a = abs(fmod(a + TAU * 4.0, seg) - seg * 0.5);
        p = float2(cos(a), sin(a)) * r;
    }
    return p / float2(aspect, 1.0) + 0.5;
}

float4 main(PSIn i) : SV_Target {
    float aspect = res.x / res.y;
    float4 f5 = flux[5], f6 = flux[6], f3 = flux[3];
    float3 a = frame.SampleLevel(samp, fold((int)(f5.z + 0.5), i.uv, aspect), 0).rgb;
    float3 b = frame.SampleLevel(samp, fold((int)(f5.w + 0.5), i.uv, aspect), 0).rgb;
    float3 col = lerp(a, b, f6.x);
    float2 c = i.uv - 0.5;
    float3 echo = frame.SampleLevel(samp, 0.5 + c / (1.0 + f6.y * 0.5), 0).rgb;
    col = max(col, echo * f6.y * 0.6);
    col *= f3.w;
    col = lerp(col, abs(sin(col * 3.14159 * 0.8)), f6.z);
    col = col / (1.0 + col * 0.4);
    col *= 1.0 - 0.35 * dot(c, c) * 2.0;

    float2 px = i.uv * res;
    float edge = min(min(px.x, px.y), min(res.x - px.x, res.y - px.y));
    float mask = res.x * res.y > 600000.0 ? 1.0 : smoothstep(0.0, clamp(min(res.x, res.y) * 0.06, 6.0, 32.0), edge);
    float lum = max(col.r, max(col.g, col.b));
    float alpha = res.x * res.y > 600000.0 ? 1.0 : saturate(lum * 1.5 + 0.55);
    return finish(float4(col * mask, alpha * mask), i.uv);
}
