#include "common.hlsli"

Texture2D prev_frame : register(t0);
SamplerState samp : register(s0);

float3 hue_rot(float3 c, float a) {
    const float3 k = float3(0.57735, 0.57735, 0.57735);
    float ca = cos(a);
    return c * ca + cross(k, c) * sin(a) + k * dot(k, c) * (1.0 - ca);
}

float3 hsv(float h, float s, float v) {
    float3 k = saturate(abs(frac(h + float3(0.0, 2.0 / 3.0, 1.0 / 3.0)) * 6.0 - 3.0) - 1.0);
    return v * lerp(float3(1, 1, 1), k, s);
}

float wave_at(float u) {
    float x = frac(u) * 127.0;
    uint i = (uint)x;
    return lerp(wav(i), wav(min(i + 1, 127u)), x - i);
}

float wave_smooth(float u) {
    return (wave_at(u - 0.008) + 2.0 * wave_at(u) + wave_at(u + 0.008)) * 0.25;
}

float line_glow(float d, float thick) {
    return exp(-d * d / (thick * thick)) + 0.25 * exp(-abs(d) / (thick * 6.0));
}

float draw_wave(int mode, float2 p, float aspect, float scale, float thick) {
    float r = length(p);
    float a = atan2(p.y, p.x) / TAU + 0.5;
    if (mode == 0) {
        float u = p.x / aspect + 0.5;
        float y = wave_smooth(u) * scale;
        return line_glow(p.y - y, thick) + 0.6 * line_glow(p.y + y * 0.7, thick * 0.8);
    }
    if (mode == 1) {
        float m = abs(a * 2.0 - 1.0);
        float rr = 0.32 + wave_smooth(m) * scale * 0.7;
        return line_glow(r - rr, thick);
    }
    if (mode == 2) {
        float m = abs(a * 2.0 - 1.0);
        float rr = 0.22 + band_at(m) * scale * 0.9;
        float bars = step(0.35, frac(a * 64.0));
        return line_glow(max(r - rr, 0.0) * 4.0 + max(0.2 - r, 0.0) * 4.0, thick * 3.0) * bars * step(0.2, r) * 0.5
            + line_glow(r - rr, thick);
    }
    if (mode == 3) {
        float u = p.y + 0.5;
        float x = 0.42 * aspect - abs(wave_smooth(u) * scale);
        return line_glow(abs(p.x) - x, thick);
    }
    float arm = frac(a * 2.0 - log(r + 0.05) * 1.6 - time * 0.15);
    float w = wave_smooth(arm) * scale * 0.5;
    return line_glow((frac(arm + w) - 0.5) * r * 2.0, thick * 1.5) * smoothstep(0.05, 0.2, r) * smoothstep(0.9, 0.4, r);
}

float4 main(PSIn i) : SV_Target {
    float aspect = res.x / res.y;
    float2 uv = i.uv;
    float2 p = (uv - 0.5) * float2(aspect, 1.0);
    float k60 = dt * 60.0;

    float4 f0 = flux[0], f1 = flux[1], f2 = flux[2], f3 = flux[3];
    float4 f4 = flux[4], f5 = flux[5], f7 = flux[7];
    float beat = f7.w;
    float2 center = f2.xy;
    float2 d = p - center;
    float r = length(d);
    float zoom = pow(max(f0.x + f0.y * beat + 0.02 * f7.x, 0.5), k60);
    float ang = (f0.z + f0.w * beat * (frac(bar_phase * 0.5) < 0.5 ? 1.0 : -1.0) + f1.w * (0.6 - r)) * k60;
    float2 q = d / zoom;
    float ca = cos(ang), sa = sin(ang);
    q = float2(q.x * ca - q.y * sa, q.x * sa + q.y * ca);
    float t = time * f1.z;
    float2 warp = f1.x * (1.0 + f7.y) * float2(sin(p.y * f1.y + t + 1.3 * sin(t * 0.37)), cos(p.x * f1.y * 1.27 - t * 0.83)) * k60;
    float2 src = center + q + warp * 0.01 + f2.zw * 0.003 * k60;
    float2 suv = src / float2(aspect, 1.0) + 0.5;

    float2 texel = 1.0 / res;
    float b = f3.z;
    float4 prev = prev_frame.SampleLevel(samp, suv, 0) * (1.0 - 0.6 * b);
    prev += (prev_frame.SampleLevel(samp, suv + float2(texel.x, 0) * 1.5, 0) + prev_frame.SampleLevel(samp, suv - float2(texel.x, 0) * 1.5, 0)
        + prev_frame.SampleLevel(samp, suv + float2(0, texel.y) * 1.5, 0) + prev_frame.SampleLevel(samp, suv - float2(0, texel.y) * 1.5, 0)) * 0.15 * b;
    float inside = step(0.0, suv.x) * step(suv.x, 1.0) * step(0.0, suv.y) * step(suv.y, 1.0);
    prev *= inside * pow(saturate(f3.x), k60);
    prev.rgb = hue_rot(prev.rgb, f3.y * k60);

    float energy = 0.4 + 0.8 * level + 0.6 * beat;
    float thick = f5.x * (1.0 + 0.6 * beat) / res.y * 600.0 * 0.004;
    float hue = f5.y + time * 0.03 + bar_phase * 0.25;
    float2 wp = p - center * 0.3;
    float wa = draw_wave((int)(f4.x + 0.5), wp, aspect, f4.w * (0.6 + 0.8 * level), thick);
    float wb = draw_wave((int)(f4.y + 0.5), wp, aspect, f4.w * (0.6 + 0.8 * level), thick);
    float wv = lerp(wa, wb, f4.z);
    float ang01 = atan2(wp.y, wp.x) / TAU;
    float3 wcol = lerp(hsv(hue + ang01 * 0.5, 0.75, 1.0), c0.rgb, 0.25);
    float3 add_col = wcol * wv * energy * 0.55 * k60;

    if (abs(beat_in_bar - 1.0) < 0.5 && beat_t < 0.2) {
        float ring_r = beat_t * 2.2;
        add_col += hsv(hue + 0.5, 0.6, 1.0) * line_glow(length(wp) - ring_r, thick * 2.0) * (1.0 - beat_t / 0.2) * drive * 0.8;
    }
    add_col += hsv(hue + 0.2, 0.5, 1.0) * jump * exp(-r * 3.0) * 0.6;

    float4 o = prev;
    o.rgb += add_col;
    o.rgb = min(o.rgb, 3.0);
    o.a = 1.0;
    return o;
}
