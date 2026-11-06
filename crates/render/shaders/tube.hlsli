#include "raymarch.hlsli"

float3 ghash(float3 p) {
    p = float3(dot(p, float3(127.1, 311.7, 74.7)), dot(p, float3(269.5, 183.3, 246.1)), dot(p, float3(113.5, 271.9, 124.6)));
    return -1.0 + 2.0 * frac(sin(p) * 43758.5453123);
}

float snoise(float3 x) {
    float3 i = floor(x);
    float3 f = frac(x);
    float3 u = f * f * f * (f * (f * 6.0 - 15.0) + 10.0);
    float n000 = dot(ghash(i), f);
    float n100 = dot(ghash(i + float3(1, 0, 0)), f - float3(1, 0, 0));
    float n010 = dot(ghash(i + float3(0, 1, 0)), f - float3(0, 1, 0));
    float n110 = dot(ghash(i + float3(1, 1, 0)), f - float3(1, 1, 0));
    float n001 = dot(ghash(i + float3(0, 0, 1)), f - float3(0, 0, 1));
    float n101 = dot(ghash(i + float3(1, 0, 1)), f - float3(1, 0, 1));
    float n011 = dot(ghash(i + float3(0, 1, 1)), f - float3(0, 1, 1));
    float n111 = dot(ghash(i + float3(1, 1, 1)), f - float3(1, 1, 1));
    float n = lerp(lerp(lerp(n000, n100, u.x), lerp(n010, n110, u.x), u.y), lerp(lerp(n001, n101, u.x), lerp(n011, n111, u.x), u.y), u.z);
    return 0.5 + 0.9 * n;
}

static const float BASE_R = 1.45;
static const float SHAPES = 8.0;

float beat_pos() { return flux[6].w; }

float3 cospal(float t) {
    float3 d = float3(0.0, 0.33, 0.67) + float3(0.1, 0.05, 0.0) * sin(scene * 1.3);
    return 0.5 + 0.5 * cos(TAU * (t + d));
}

float3 trip(float t) {
    int st = style_id();
    if (custom_pal()) return lerp(palette(t), cospal(t), 0.12);
    float3 rainbow = cospal(t);
    float3 pal = palette(t);
    if (st == NEON) return lerp(rainbow, pal, 0.25);
    if (st == FLUENT) return lerp(pal, rainbow, 0.25) * 0.8;
    if (st == SYNTH) return lerp(pal, rainbow, 0.15);
    return lerp(rainbow, pal, 0.5);
}

float2 path(float z) {
    float k = 1.0 + 0.6 * sin(z * 0.013);
    return float2(sin(z * 0.081) * 2.4 + sin(z * 0.193) * 0.7, cos(z * 0.067) * 1.7 + sin(z * 0.151) * 0.6) * k;
}

float2 path_d2(float z) {
    float e = 0.5;
    return (path(z + e) - 2.0 * path(z) + path(z - e)) / (e * e);
}

float poly(float a, float n) {
    float s = TAU / n;
    return cos(3.14159265 / n) / cos(fmod(a + 64.0 * TAU, s) - s * 0.5);
}

float shape_r(float id, float a, float z) {
    float m = fmod(id, SHAPES);
    if (m < 0.5) return 1.0 + 0.05 * cos(a * 12.0);
    if (m < 1.5) return 0.78 + 0.32 * pow(abs(cos(a * 2.5)), 0.6);
    if (m < 2.5) return poly(a, 6.0);
    if (m < 3.5) return 1.0 + 0.2 * cos(a * 8.0) - 0.06 * cos(a * 16.0);
    if (m < 4.5) return 1.0 + 0.24 * (tnoise(float3(cos(a) * 1.6, sin(a) * 1.6, z * 0.25 + id)) - 0.5) * 2.0;
    if (m < 5.5) return poly(a, 3.0) * 1.05;
    if (m < 6.5) return 0.9 + 0.12 * step(0.5, frac(a * 12.0 / TAU)) + 0.04 * cos(a * 24.0);
    return poly(a, 4.0) * (1.0 + 0.1 * cos(a * 4.0 + z));
}

float radius_at(float a, float z) {
    float bp = beat_pos();
    float bar = floor(bp / 4.0);
    float morph = smoothstep(0.7, 1.0, frac(bp / 4.0));
    float id = bar + floor(scene) * 3.0;
    float tw = 0.15 * sin(bar * 1.7) + 0.35 * drive * max(spring(), 0.0) * (abs(beat_in_bar - 1.0) < 0.5 ? 1.0 : 0.3);
    float aa = a + z * tw + 0.6 * sin(z * 0.11 + time * 0.3);
    float r = lerp(shape_r(id, aa, z), shape_r(id + 1.0, aa, z), morph);
    float wave = sin(z * 0.9 - bp * TAU * 0.5) * 0.08 * (0.3 + flux[7].x);
    r *= BASE_R * (1.0 + 0.1 * pulse * drive + wave);
    r += 0.05 * sin(aa * 5.0 + z * 1.7 - time * 2.0) * (0.3 + flux[7].y);
    return r;
}

float map(float3 p) {
    float2 q = p.xy - path(p.z);
    float a = atan2(q.y, q.x);
    return (radius_at(a, p.z) - length(q)) * 0.55;
}

float kaleido_amount() {
    float s = fmod(floor(scene) + 64.0, 3.0);
    return s > 1.5 ? 1.0 : 0.0;
}

float2 kaleido(float2 p, float n) {
    float r = length(p);
    float w = TAU / n;
    float a = atan2(p.y, p.x) + time * 0.05;
    a = abs(fmod(a + 64.0 * TAU, w) - w * 0.5);
    return float2(cos(a), sin(a)) * r;
}

// Screen-space spacetime bending: drifting gravity well, swirl, kick shockwave and scene kaleidoscope.
float2 bend_screen(float2 p) {
    float hit = pulse * drive;
    float bass = flux[7].x;
    float r = length(p);
    float sw = (0.9 * sin(time * 0.13 + scene * 1.9) + 0.6 * hit) * exp(-r * 1.1);
    p = rot2(p, sw);
    float2 m = float2(sin(time * 0.11 + scene), cos(time * 0.087 + scene * 0.7)) * 0.45;
    float2 dm = p - m;
    float rm2 = dot(dm, dm) + 0.12;
    p -= dm * (0.05 * (0.6 + 0.8 * bass + 0.6 * drive)) / rm2;
    float ring = beat_t * 2.4;
    float shock = exp(-pow((r - ring) * 7.0, 2.0)) * exp(-beat_t * 2.5) * drive;
    p *= 1.0 - 0.09 * shock * (0.6 + bass);
    p += float2(sin(p.y * 3.0 + time * 0.7), sin(p.x * 3.0 - time * 0.6)) * 0.025 * (0.4 + level);
    float k = kaleido_amount();
    if (k > 0.0) p = lerp(p, kaleido(p, 6.0 + 2.0 * fmod(floor(scene / 3.0), 2.0)), k);
    return p;
}
