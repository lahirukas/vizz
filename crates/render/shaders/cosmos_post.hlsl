#include "common.hlsli"

Texture2D frame : register(t0);
SamplerState samp : register(s0);

float3 bright(float2 uv) {
    float3 c = frame.SampleLevel(samp, uv, 0).rgb;
    float b = max(c.r, max(c.g, c.b));
    return c * smoothstep(0.35, 1.2, b);
}

float4 main(PSIn i) : SV_Target {
    float2 uv = i.uv;
    float2 c = uv - 0.5;
    float ab = 0.0015 + 0.006 * jump + 0.002 * pulse * drive;
    float blur = 0.008 * saturate(aux2 - 0.8) + 0.05 * jump + 0.006 * pulse * drive;
    float3 col = 0;
    for (int m = 0; m < 6; m++) {
        float2 o = -c * blur * (m / 5.0);
        col.r += frame.SampleLevel(samp, uv + o + c * ab, 0).r;
        col.g += frame.SampleLevel(samp, uv + o, 0).g;
        col.b += frame.SampleLevel(samp, uv + o - c * ab, 0).b;
    }
    col /= 6.0;

    float2 texel = 1.0 / res;
    float3 bloom = 0;
    int taps = 4 + 4 * min(quality_id(), 2);
    for (int k = 0; k < 12; k++) {
        if (k >= taps) break;
        float a = k * TAU / taps + 0.39;
        float2 o = float2(cos(a), sin(a));
        bloom += (bright(uv + o * texel * 6.0) * 0.6 + bright(uv + o * texel * 16.0) * 0.4) * 8.0 / taps;
    }
    col += bloom * (0.2 + 0.15 * pulse * drive);

    col *= 1.0 - 0.55 * dot(c, c) * 2.0;
    col *= 1.15;
    col = saturate(col * (2.51 * col + 0.03) / (col * (2.43 * col + 0.59) + 0.14));
    float grain = (hash(uv * res + frac(time) * 100.0) - 0.5) * 0.02;
    col += grain;

    float2 px = uv * res;
    float edge = min(min(px.x, px.y), min(res.x - px.x, res.y - px.y));
    float mask = res.x * res.y > 600000.0 ? 1.0 : smoothstep(0.0, clamp(min(res.x, res.y) * 0.06, 6.0, 32.0), edge);
    return finish(float4(max(col, 0.0), 1.0) * mask, uv);
}
