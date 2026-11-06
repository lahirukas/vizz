#include "common.hlsli"

static const float BASE_R = 0.56;
static const float ARC = 0.375;

float2 on_ring(float turn, float r) { return float2(sin(turn * TAU), -cos(turn * TAU)) * r; }

float seg_dist(float2 p, float2 a, float2 b) {
    float2 ab = b - a;
    float h = saturate(dot(p - a, ab) / max(dot(ab, ab), 1e-6));
    return length(p - a - ab * h);
}

float4 main(PSIn i) : SV_Target {
    int st = style_id();
    float scale = min(res.x, res.y);
    float2 p = (i.uv - 0.5) * res / scale * 2.0;
    float px = 2.0 / scale;
    float r = length(p);
    float a = atan2(p.x, -p.y) / TAU;

    bool downbeat = abs(beat_in_bar - 1.0) < 0.5;
    float sp = spring();
    float punch = drive * (downbeat ? 1.35 : 1.0) * sp;
    float radius = BASE_R * (1.0 + 0.075 * punch - drive * wind * 0.025);

    float spec = band_at(abs(frac(a + 0.5) * 2.0 - 1.0));
    float wobble = 0.0;
    if (st == LIQUID) {
        wobble = (noise(normalize(p + 1e-5) * 3.0 + float2(time * 0.4, time * 0.3)) - 0.5) * (0.05 + 0.08 * level) + 0.03 * spec;
        radius += wobble;
    }
    float thick = 0.016 + 0.05 * pow(spec, 1.5) * (0.5 + 0.5 * level) + 0.012 * kick;
    if (st == FLUENT) thick = 0.007 + 0.012 * spec + 0.004 * kick;
    if (st == LIQUID) thick = 0.03 + 0.04 * spec;
    float d = abs(r - radius);

    float3 wc = palette(pitch * 0.8 + 0.1);
    float3 hue = lerp(palette(a + time * 0.03 + bar_phase * 0.25), wc, 0.35 * whistle);
    if (st == FLUENT) hue = lerp(ink(), c0.rgb, 0.55 + 0.25 * pulse * drive);
    float3 hot = lerp(hue, float3(1, 1, 1), 0.5 * pulse * drive + 0.15 * kick);
    float4 o = 0;

    float glow_gain = st == FLUENT ? 0.35 : (st == SYNTH ? 1.3 : 1.0);
    float glow = exp(-d * (10.0 - 4.0 * pulse * drive)) * (0.12 + 0.55 * pulse * drive + 0.2 * kick + 0.15 * level);
    add(o, hue, glow * glow_gain);

    float body = st == LIQUID ? smoothstep(thick, 0.0, d) : aa(d, thick, 1.5 * px);
    float depth = saturate(1.0 - d / max(thick, 1e-4));
    float3 body_col = hot * (0.6 + 0.4 * depth);
    if (st == SYNTH) {
        float band_y = p.y + 0.06 * time;
        float stripe = step(0.0, p.y) * step(0.55, frac(band_y * 9.0));
        body *= 1.0 - stripe * 0.85;
        body_col = lerp(c2.rgb, c0.rgb, saturate(0.5 - p.y));
    }
    over(o, body_col, body * 0.95);

    if (st != LIQUID) {
        float rim = aa(abs(r - radius - thick), 0.005, px);
        float2 n = p / max(r, 1e-4);
        float hl = pow(saturate(dot(n, normalize(float2(-0.55, -0.83)))), 10.0);
        add(o, float3(1, 1, 1), (rim * (0.25 + 0.6 * hl) + body * hl * 0.35) * (st == FLUENT ? 0.3 : 1.0));
    }

    if (downbeat && beat_t < 0.5) {
        float k = beat_t / 0.5;
        float wave_r = radius + 0.38 * sqrt(k);
        float shock = aa(abs(r - wave_r), 0.006 + 0.02 * k, px) * (1.0 - k) * (1.0 - k) * drive;
        add(o, lerp(hue, float3(1, 1, 1), 0.4), shock * (st == FLUENT ? 0.35 : 0.9));
    }

    if (st != LIQUID) {
        float track = aa(abs(r - 0.34), st == FLUENT ? 0.003 : 0.007, px);
        float done = step(frac(a + 1.0), bar_phase) * drive;
        over(o, lerp(hue, float3(1, 1, 1), 0.25), track * (0.08 + 0.5 * done));
        for (int k = 0; k < 4; k++) {
            float2 dp = on_ring(k / 4.0, 0.26);
            float on = step(abs(beat_in_bar - 1.0 - k), 0.5) * step(0.01, drive);
            float size = (k == 0 ? 0.034 : 0.025) * (1.0 + 0.5 * on * max(sp, 0.0)) * (st == FLUENT ? 0.6 : 1.0);
            float dotm = aa(length(p - dp), size, px);
            over(o, lerp(palette(k / 4.0), float3(1, 1, 1), on * 0.5), dotm * (0.15 + 0.8 * on));
        }
    }

    float core = smoothstep(0.3, 0.0, r) * (0.05 + 0.25 * level + 0.35 * pulse * drive + 0.2 * kick);
    add(o, st == SYNTH ? c2.rgb : palette(time * 0.05 + 0.5), core * (st == FLUENT ? 0.3 : 1.0));

    if (whistle > 0.01 || trail[0].y > 0.01) {
        float tg = 0;
        float2 prev = on_ring(lerp(-ARC, ARC, trail[0].x), radius);
        float prev_s = trail[0].y;
        for (int t = 1; t < 16; t++) {
            float4 q = trail[t >> 1];
            float pos = (t & 1) ? q.z : q.x;
            float s = (t & 1) ? q.w : q.y;
            float2 cur = on_ring(lerp(-ARC, ARC, pos), radius);
            float fade = 1.0 - t / 16.0;
            tg = max(tg, aa(seg_dist(p, prev, cur), 0.01 + 0.03 * fade, px * 2.0) * min(s, prev_s) * fade);
            prev = cur;
            prev_s = s;
        }
        add(o, wc, tg * 1.2);
        float2 head = on_ring(lerp(-ARC, ARC, pitch), radius);
        float hd = length(p - head);
        add(o, wc, aa(seg_dist(p, float2(0, 0), head), 0.004, px) * smoothstep(0.1, radius, r) * 0.45 * whistle);
        add(o, wc, exp(-hd * 9.0) * whistle * 0.8);
        over(o, lerp(wc, float3(1, 1, 1), 0.75), aa(hd, 0.045 + 0.025 * whistle, px) * whistle);
    }

    o *= smoothstep(1.0, 0.86, r);
    return finish(o, i.uv);
}
