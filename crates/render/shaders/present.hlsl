#include "common.hlsli"

Texture2D frame : register(t0);
SamplerState samp : register(s0);

float4 main(PSIn i) : SV_Target {
    int st = style_id();
    float4 o = frame.SampleLevel(samp, i.uv, 0);
    float2 px = i.uv * res;
    float edge = min(min(px.x, px.y), min(res.x - px.x, res.y - px.y));
    float soft = clamp(min(res.x, res.y) * 0.08, 6.0, 48.0);
    float mask = smoothstep(0.0, soft, edge);
    if (st == SYNTH) {
        float scan = 0.75 + 0.25 * step(0.5, frac(px.y * 0.5));
        o.rgb *= scan;
    }
    if (st == FLUENT) {
        float2 c = i.uv - 0.5;
        o *= 1.0 - 0.6 * dot(c, c);
    }
    o.rgb = o.rgb / (1.0 + 0.25 * max(o.r, max(o.g, o.b)));
    return finish(o * mask, i.uv);
}
