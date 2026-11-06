#include "tube.hlsli"

static const float3 NEONS[7] = {
    float3(1.00, 0.10, 0.80),
    float3(0.05, 0.90, 1.00),
    float3(1.00, 0.88, 0.05),
    float3(0.30, 1.00, 0.20),
    float3(1.00, 0.42, 0.05),
    float3(0.55, 0.25, 1.00),
    float3(1.00, 0.12, 0.30),
};
static const float SPACING = 2.2;
static const int FRAMES = 18;
static const int RAILS = 12;
static const float NSHAPES = 14.0;
static const float PI = 3.14159265;

float3 neon(float i) {
    int st = style_id();
    int k = (int)fmod(fmod(i, 7.0) + 7.0, 7.0);
    float3 c = NEONS[k];
    if (st == SYNTH) c = NEONS[k % 4 == 0 ? 0 : (k % 4 == 1 ? 1 : (k % 4 == 2 ? 5 : 4))];
    if (st == FLUENT) c = lerp(c, float3(1, 1, 1), 0.2) * 0.85;
    if (st == LIQUID) c = lerp(c, palette(i / 7.0), 0.5);
    if (custom_pal()) c = palette(frac(k / 7.0)) * 1.15;
    return c;
}

float hash1(float x) { return frac(sin(x * 91.345 + 7.13) * 43758.5453); }

float cross2(float2 a, float2 b) { return a.x * b.y - a.y * b.x; }

// Radius of an n-point star (k = inner/outer ratio; k = cos(pi/n) gives the regular polygon) along angle a.
float star_r(float a, float n, float k) {
    float s = PI / n;
    float f = s - abs(fmod(a + 64.0 * TAU, 2.0 * s) - s);
    float2 p1 = float2(1, 0);
    float2 e = float2(k * cos(s), k * sin(s)) - p1;
    return cross2(p1, e) / cross2(float2(cos(f), sin(f)), e);
}

float shape(float id, float a) {
    float m = fmod(fmod(id, NSHAPES) + NSHAPES, NSHAPES);
    if (m < 0.5) return star_r(a, 3.0, 0.5);
    if (m < 1.5) return star_r(a + PI / 4.0, 4.0, 0.7071);
    if (m < 2.5) return star_r(a, 6.0, 0.866);
    if (m < 3.5) return star_r(a, 5.0, 0.809);
    if (m < 4.5) return star_r(a, 6.0, 0.577);
    if (m < 5.5) return star_r(a, 4.0, 0.7071) * 0.95;
    if (m < 6.5) return star_r(a, 8.0, 0.924);
    if (m < 7.5) return star_r(a, 5.0, 0.42);
    if (m < 8.5) return star_r(a + PI / 4.0, 4.0, 0.38);
    if (m < 9.5) return star_r(a, 6.0, 0.866) * (0.9 + 0.1 * step(0.5, frac(a * 18.0 / TAU)));
    if (m < 10.5) return 0.82 + 0.18 * pow(abs(cos(a * 3.0)), 0.5);
    if (m < 11.5) return star_r(a, 12.0, 0.7);
    if (m < 12.5) return star_r(a, 3.0, 0.5) * 0.5 + star_r(a + PI, 3.0, 0.5) * 0.5;
    return 1.0 + 0.06 * cos(a * 16.0);
}

float shape_id(float b) {
    float beat = floor(b);
    float bar = floor(beat / 4.0);
    float fam = floor(hash1(bar + scene * 7.0) * NSHAPES);
    float step_in = floor(hash1(beat) * 4.0);
    return fam + (fmod(beat + 64.0, 4.0) < 0.5 ? 0.0 : 1.0 + step_in * 3.0);
}

float tunnel_r(float a, float b) {
    float f = frac(b);
    float e = 1.0 - exp(-f * 9.0) * cos(f * 22.0);
    float m = lerp(1.0, e, smoothstep(0.0, 0.15, drive + 0.05));
    return lerp(shape(shape_id(b - 1.0), a), shape(shape_id(b), a), m);
}

struct Glow {
    float3 col;
    float px;
    float width;
};

void add_line(inout Glow g, float d, float t, float3 c, float bright) {
    float dd = max(d - g.width, 0.0) / t;
    float core = 1.0 - smoothstep(0.0, g.px * 1.5, dd);
    float halo = exp(-dd / 0.012) * 0.4 + exp(-dd / 0.0035) * 0.5;
    float fade = exp(-t * 0.045) * bright;
    float3 lit = lerp(c, float3(1, 1, 1), 0.25 * core) * (core * 1.3 + halo) * fade;
    g.col = max(g.col, lit);
}

float2 ray_seg(float3 ro, float3 rd, float3 a, float3 b) {
    float3 ba = b - a;
    float3 oa = ro - a;
    float baba = dot(ba, ba);
    float rdba = dot(rd, ba);
    float oaba = dot(oa, ba);
    float rdoa = dot(rd, oa);
    float den = baba - rdba * rdba;
    float s = den > 1e-6 ? saturate((oaba - rdba * rdoa) / den) : 0.0;
    float t = max(s * rdba - rdoa, 0.0);
    s = saturate((oaba + t * rdba) / max(baba, 1e-6));
    return float2(length(oa + rd * t - ba * s), max(t, 1e-3));
}

struct Frame {
    float3 c;
    float r;
    float rot;
    float b;
};

Frame frame_at(float k, float bp, float cz, float mode) {
    float gi = floor(bp) + k + 1.0;
    float dz = (k + 1.0 - frac(bp)) * SPACING;
    float z = cz + dz;
    Frame f;
    f.c = float3(path(z), z);
    f.b = bp - dz / SPACING * 0.3;
    float kick = max(spring(), 0.0) * drive * exp(-dz * 0.12);
    float taper = (mode > 0.5 && mode < 1.5 && fmod(gi + 64.0, 2.0) > 0.5) ? 0.6 : 1.0;
    f.r = BASE_R * (1.0 + 0.1 * flux[7].x + 0.25 * kick) * taper * (1.0 + 1.2 * jump);
    float dir = fmod(floor(scene), 2.0) < 0.5 ? 1.0 : -1.0;
    float twist = (mode > 1.5 && fmod(gi + 64.0, 2.0) > 0.5) ? PI / 4.0 : 0.0;
    f.rot = dir * (gi * (0.06 + 0.1 * level) + time * 0.08 + flux[7].y * 0.35) + twist;
    return f;
}

float3 frame_point(Frame f, float a) {
    float r = f.r * tunnel_r(a - f.rot, f.b);
    return f.c + float3(cos(a) * r, sin(a) * r, 0);
}

float4 main(PSIn i) : SV_Target {
    float scale = min(res.x, res.y);
    float2 p = (i.uv - 0.5) * res / scale * 2.0 * float2(1, -1);
    float bp = beat_pos();
    float hit = pulse * drive;
    float cz = travel * 3.0;
    float3 eye = float3(path(cz), cz);
    float3 target = float3(path(cz + 2.5), cz + 2.5);
    float bank = -path_d2(cz + 1.5).x * 0.8;
    float roll = bank + TAU * smoothstep(0.0, 1.0, frac(scene)) + time * 0.04
        + 0.25 * drive * max(spring(), 0.0) * (frac(floor(bp / 4.0) * 0.5) < 0.5 ? 1.0 : -1.0);
    float fov = 1.3 - 0.1 * hit - 0.35 * jump;
    Camera cam = look_from(eye, target, bend_screen(p), fov, roll);
    float3 ro = cam.pos;
    float3 rd = cam.dir;

    Glow g;
    g.col = 0;
    g.px = 2.0 / scale / fov;
    g.width = 0.012 * (1.0 + 0.8 * hit);
    float energy = 0.35 + 0.65 * smoothstep(0.0, 0.3, drive + level * 0.5);
    float mode = fmod(floor(scene) + 64.0, 3.0);
    float wave_d = beat_t * 14.0;

    [loop] for (int k = 0; k < FRAMES; k++) {
        Frame f = frame_at(k, bp, cz, mode);
        float gi = floor(bp) + k + 1.0;
        float down = fmod(gi + 64.0, 4.0) < 0.5 ? 1.0 : 0.0;
        float dz = f.c.z - ro.z;
        float ripple = exp(-pow((dz - wave_d) * 0.6, 2.0)) * drive * exp(-beat_t * 1.5);
        float bright = energy * (0.75 + 0.6 * down + 0.5 * hit + 1.6 * ripple);
        if (rd.z > 0.01 && dz > 0.15) {
            float tp = dz / rd.z;
            float2 q = (ro + rd * tp).xy - f.c.xy;
            float a = atan2(q.y, q.x);
            float lq = length(q);
            float ea = 0.01;
            float rr = f.r * tunnel_r(a - f.rot, f.b);
            float dr = f.r * (tunnel_r(a + ea - f.rot, f.b) - tunnel_r(a - ea - f.rot, f.b)) / (2.0 * ea);
            float slope = rsqrt(1.0 + pow(dr / max(lq, 0.05), 2.0));
            add_line(g, abs(lq - rr) * slope, tp, neon(gi), bright);
            float echo = 0.86 - 0.08 * max(spring(), 0.0) * drive;
            add_line(g, abs(lq - rr * echo) * slope * 1.7, tp, neon(gi + 2.0) * 0.7, bright * 0.55);
            float echo2 = 0.72 + 0.1 * flux[7].y;
            add_line(g, abs(lq - rr * echo2) * slope * 2.6, tp, neon(gi + 4.0) * 0.5, bright * 0.35 * (0.3 + flux[7].y));
        }
        if (k + 1 < FRAMES && dz + SPACING > 0.2) {
            Frame n = frame_at(k + 1, bp, cz, mode);
            [loop] for (int j = 0; j < RAILS; j++) {
                float va = j * TAU / RAILS;
                float3 a = frame_point(f, va + f.rot);
                float3 b = frame_point(n, va + n.rot);
                float2 r = ray_seg(ro, rd, a, b);
                add_line(g, r.x * 1.3, r.y, neon(gi + 3.0) * 0.85, energy * 0.7);
                float run = frac(time * 1.7 + j * 0.37 + gi * 0.21);
                float3 pd = lerp(a, b, run) - ro;
                float tpr = max(dot(pd, rd), 1e-3);
                float spark = exp(-length(pd - rd * tpr) / tpr / 0.005) * (0.1 + 1.6 * flux[7].z);
                g.col += float3(1, 1, 1) * spark * exp(-tpr * 0.05) * energy;
                if ((j & 1) == 0) {
                    float3 bd = frame_point(n, va + TAU / RAILS + n.rot);
                    float2 r2 = ray_seg(ro, rd, a, bd);
                    add_line(g, r2.x * 2.2, r2.y, neon(gi + 5.0) * 0.55, energy * 0.4 * (0.5 + 0.8 * flux[7].y));
                }
            }
        }
    }

    float2 sp = i.uv - 0.5;
    float3 bg = lerp(neon(floor(bp / 16.0)), neon(floor(bp / 16.0) + 4.0), i.uv.y) * 0.025 * (1.0 - dot(sp, sp) * 1.5);
    float3 col = max(g.col, bg);
    col += neon(floor(bp)) * jump * 0.25;
    col = col / (1.0 + 0.35 * max(col.r, max(col.g, col.b)));
    return float4(col, 1.0);
}
