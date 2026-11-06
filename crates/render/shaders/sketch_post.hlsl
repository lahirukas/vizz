#include "common.hlsli"

Texture2D frame : register(t0);
SamplerState samp : register(s0);

float3 hsv(float h, float s, float v) {
    float3 k = saturate(abs(frac(h + float3(0.0, 2.0 / 3.0, 1.0 / 3.0)) * 6.0 - 3.0) - 1.0);
    return v * lerp(float3(1, 1, 1), k, s);
}

float tri(float2 p, float size) {
    float2 q = float2(abs(p.y), p.x);
    return max(q.x * 0.866 + q.y * 0.5, -q.y) - size * 0.5;
}

float4 main(PSIn i) : SV_Target {
    float scale = min(res.x, res.y);
    float2 p = (i.uv - 0.5) * res / scale * 2.0 * float2(1, -1);
    float px = 2.0 / scale;
    float4 o = frame.SampleLevel(samp, i.uv, 0);
    float2 texel = 1.0 / res;
    float3 bloom = 0;
    for (int k = 0; k < 6; k++) {
        float a = k * 1.0472;
        bloom += frame.SampleLevel(samp, i.uv + float2(cos(a), sin(a)) * texel * 5.0, 0).rgb;
    }
    o.rgb += bloom * 0.06;

    float2 head = sketch_pos.xy;
    float ang = sketch_info.x;
    float2 d = p - head;
    float c = cos(-ang), s = sin(-ang);
    float2 local = float2(d.x * c - d.y * s, d.x * s + d.y * c);
    float3 ink = hsv(sketch_info.y, 0.55, 1.0);
    float vis = saturate(aux1 * 2.0 + 0.15);
    float arrow = smoothstep(px, -px, tri(local - float2(0.01, 0.0), 0.06));
    float halo = exp(-length(d) * 22.0) * (0.4 + 0.6 * aux1);
    o.rgb += ink * halo * vis + lerp(ink, float3(1, 1, 1), 0.6) * arrow * vis;
    o.a = max(o.a, saturate(arrow * vis + halo * vis * 0.6));

    if (aux0 > 0.0) {
        float2 bc = trail[7].xy;
        float2 bd = p - bc;
        float flash = exp(-length(bd) * 6.0) * aux0 * aux0;
        o.rgb += lerp(ink, float3(1, 1, 1), 0.6) * flash;
        o.a = max(o.a, saturate(flash));
        float r = (1.0 - aux0) * 1.1;
        float ring = exp(-abs(length(bd) - r) * 60.0) * aux0;
        float spark = 0;
        for (int k = 0; k < 10; k++) {
            float a = k * 0.6283 + aux0 * 2.0;
            float2 sp = bc + float2(cos(a), sin(a)) * r * (0.8 + 0.4 * frac(k * 0.37));
            spark += exp(-length(p - sp) * 90.0);
        }
        o.rgb += lerp(ink, float3(1, 1, 1), 0.5) * (ring + spark * 1.5) * aux0;
        o.a = max(o.a, saturate((ring + spark) * aux0));
    }
    float lum = max(o.r, max(o.g, o.b));
    o.a = max(o.a, saturate(lum * 0.8));
    return finish(o, i.uv);
}
