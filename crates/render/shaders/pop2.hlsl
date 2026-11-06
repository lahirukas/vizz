#include "tube.hlsli"

static const float AROUND = 8.0;
static const float R2 = 1.6;
static const float PI = 3.14159265;

float2 path2(float z) {
    return float2(sin(z * 0.11) * 3.4 + sin(z * 0.047) * 2.2, cos(z * 0.083) * 2.6 + sin(z * 0.061) * 1.2);
}

float2 path2_d2(float z) {
    float e = 0.5;
    return (path2(z + e) - 2.0 * path2(z) + path2(z - e)) / (e * e);
}

float radius2(float a, float z) {
    float t = time * 0.06;
    float r = 1.0 + 0.12 * sin(2.0 * a + z * 0.21 + t) + 0.07 * sin(3.0 * a - z * 0.13 - t * 1.3);
    return R2 * r * (1.0 + 0.04 * flux[7].x * drive);
}

float map2(float3 p) {
    float2 q = p.xy - path2(p.z);
    return (radius2(atan2(q.y, q.x), p.z) - length(q)) * 0.6;
}

float3 ramp(float t) {
    t = saturate(t);
    float3 deep = c0.rgb * 0.18;
    float3 light = lerp(c2.rgb, float3(1, 1, 1), 0.35);
    if (t < 0.25) return lerp(deep, c0.rgb, t / 0.25);
    if (t < 0.5) return lerp(c0.rgb, c1.rgb, (t - 0.25) / 0.25);
    if (t < 0.75) return lerp(c1.rgb, c2.rgb, (t - 0.5) / 0.25);
    return lerp(c2.rgb, light, (t - 0.75) / 0.25);
}

float2 hash2p(float2 c, float period) {
    c.x = fmod(fmod(c.x, period) + period, period);
    float2 h = float2(dot(c, float2(127.1, 311.7)), dot(c, float2(269.5, 183.3)));
    return frac(sin(h) * 43758.5453);
}

// Voronoi seamless around the tube: x = nearest distance, y = edge distance, zw = cell id.
float4 voronoi(float2 s, float period) {
    float2 n = floor(s);
    float2 f = frac(s);
    float d1 = 8.0;
    float d2 = 8.0;
    float2 id = 0;
    float2 mr = 0;
    for (int j = -1; j <= 1; j++) {
        for (int i = -1; i <= 1; i++) {
            float2 g = float2(i, j);
            float2 o = hash2p(n + g, period);
            o = 0.5 + 0.35 * sin(time * 0.05 + TAU * o);
            float2 r = g + o - f;
            float d = dot(r, r);
            if (d < d1) {
                d2 = d1;
                d1 = d;
                id = n + g;
                mr = r;
            } else if (d < d2) {
                d2 = d;
            }
        }
    }
    return float4(sqrt(d1), sqrt(d2) - sqrt(d1), id);
}

float fbm(float3 p) {
    return snoise(p) * 0.5 + snoise(p * 2.03 + 3.1) * 0.27 + snoise(p * 4.11 + 7.7) * 0.15 + snoise(p * 8.3 + 1.3) * 0.08;
}

struct Mat {
    float3 albedo;
    float height;
    float rough;
    float glow;
};

Mat agate(float2 s, float3 c) {
    float3 w = c + float3(snoise(c * 0.6), snoise(c * 0.6 + 4.0), 0) * 1.4;
    float f = fbm(w * 0.7);
    float b = f * 9.0 + time * 0.02;
    float fine = 0.5 + 0.5 * sin(b * TAU * 3.0);
    Mat m;
    m.albedo = ramp(frac(b * 0.23)) * (0.75 + 0.35 * fine);
    float quartz = smoothstep(0.72, 0.8, f);
    float4 v = voronoi(s * 3.0, AROUND * 3.0);
    float3 crystal = lerp(float3(0.85, 0.9, 1.0), c2.rgb, 0.25) * (0.6 + 0.4 * smoothstep(0.0, 0.08, v.y));
    m.albedo = lerp(m.albedo, crystal, quartz);
    m.height = fine * 0.5 + quartz * (0.6 + 0.4 * v.x);
    m.rough = lerp(0.35, 0.08, quartz);
    m.glow = 0.25 + 0.5 * quartz;
    return m;
}

Mat malachite(float2 s, float3 c) {
    float4 v = voronoi(s * 0.75, AROUND * 0.75);
    float warp = fbm(c * 1.3) * 0.6;
    float rings = v.x * 9.0 + warp * 3.0 - time * 0.03;
    float band = 0.5 + 0.5 * sin(rings * TAU);
    float id = frac(sin(dot(v.zw, float2(12.9, 78.2))) * 43758.5);
    Mat m;
    m.albedo = lerp(ramp(0.18 + 0.15 * id), ramp(0.55 + 0.2 * id), band);
    m.albedo *= 0.55 + 0.45 * smoothstep(0.0, 0.05, v.y);
    m.height = band * 0.8;
    m.rough = 0.22;
    m.glow = 0.12;
    return m;
}

Mat peacock(float2 s, float3 c) {
    float2 cell = s * float2(1.0, 1.4);
    float4 v = voronoi(cell, AROUND);
    float2 ctr = v.zw + 0.5;
    float d = v.x;
    float eye = smoothstep(0.42, 0.0, d);
    float3 col = ramp(0.15);
    col = lerp(col, ramp(0.95), smoothstep(0.42, 0.33, d));
    col = lerp(col, ramp(0.6), smoothstep(0.3, 0.22, d));
    col = lerp(col, ramp(0.35), smoothstep(0.2, 0.14, d));
    col = lerp(col, c0.rgb * 0.15, smoothstep(0.11, 0.07, d));
    float barbs = 0.5 + 0.5 * sin((s.x * 40.0 + snoise(c * 3.0) * 6.0) * 1.0);
    Mat m;
    m.albedo = col * lerp(0.7 + 0.3 * barbs, 1.0, eye);
    m.height = eye * 0.6 + barbs * 0.2;
    m.rough = 0.18;
    m.glow = 0.2 + 0.3 * eye;
    return m;
}

Mat leaf(float2 s, float3 c) {
    float4 big = voronoi(s * 0.75, AROUND * 0.75);
    float4 small = voronoi(s * 4.0, AROUND * 4.0);
    float vein = smoothstep(0.06, 0.0, big.y) + 0.5 * smoothstep(0.035, 0.0, small.y);
    float tone = fbm(c * 2.0);
    Mat m;
    m.albedo = lerp(ramp(0.3 + 0.25 * tone), ramp(0.85), saturate(vein));
    m.height = 1.0 - saturate(vein) * 0.8 + tone * 0.2;
    m.rough = 0.4;
    m.glow = 0.55 + 0.4 * saturate(vein);
    return m;
}

Mat material(float id, float2 s, float3 c) {
    float k = fmod(fmod(id, 4.0) + 4.0, 4.0);
    if (k < 0.5) return agate(s, c);
    if (k < 1.5) return malachite(s, c);
    if (k < 2.5) return peacock(s, c);
    return leaf(s, c);
}

void surface(float3 p, float id, out float2 s, out float3 c) {
    float2 q = p.xy - path2(p.z);
    float a = atan2(q.y, q.x);
    s = float2((a / TAU + 0.5) * AROUND, p.z * 0.75);
    c = float3(cos(a) * 1.2, sin(a) * 1.2, p.z * 0.3);
}

float height_at(float3 p, float id) {
    float2 s;
    float3 c;
    surface(p, id, s, c);
    return material(id, s, c).height;
}

float4 main(PSIn i) : SV_Target {
    float scale = min(res.x, res.y);
    float2 p = (i.uv - 0.5) * res / scale * 2.0 * float2(1, -1);
    float bp = flux[6].w;
    float cz = travel * 1.2 + time * 0.15;
    float3 eye = float3(path2(cz), cz);
    float3 target = float3(path2(cz + 1.6), cz + 1.6);
    float roll = -path2_d2(cz + 1.0).x * 0.6 + 0.15 * sin(time * 0.05);
    Camera cam = look_from(eye, target, p, 1.15, roll);

    float t = 0.02;
    float3 pos = cam.pos;
    [loop] for (int k = 0; k < 160; k++) {
        pos = cam.pos + cam.dir * t;
        float d = map2(pos);
        if (d < 0.0008 * t) break;
        t += d;
        if (t > 40.0) break;
    }
    t = min(t, 40.0);
    pos = cam.pos + cam.dir * t;

    float mid = scene;
    float m0 = floor(mid);
    float2 s;
    float3 c;
    surface(pos, m0, s, c);
    Mat ma = material(m0 + 1.0, s, c);
    Mat mb = material(m0, s, c);
    float wmix = smoothstep(0.35, 0.95, frac(mid));
    Mat m;
    m.albedo = lerp(mb.albedo, ma.albedo, wmix);
    m.height = lerp(mb.height, ma.height, wmix);
    m.rough = lerp(mb.rough, ma.rough, wmix);
    m.glow = lerp(mb.glow, ma.glow, wmix);

    float fw = length(fwidth(c)) * AROUND / 7.5;
    float lod = saturate(fw * 4.0 - 0.15);

    float2 e2 = float2(1.0, -1.0) * 0.002 * t;
    float3 n = normalize(e2.xyy * map2(pos + e2.xyy) + e2.yyx * map2(pos + e2.yyx) + e2.yxy * map2(pos + e2.yxy) + e2.xxx * map2(pos + e2.xxx));
    float2 q = pos.xy - path2(pos.z);
    float a = atan2(q.y, q.x);
    float3 tang = float3(-sin(a), cos(a), 0);
    float3 bit = float3(0, 0, 1);
    float eb = 0.01;
    float h0 = m.height;
    float hu = height_at(pos + tang * eb, m0);
    float hv = height_at(pos + bit * eb, m0);
    float bump = 0.05 * (1.0 - lod);
    float3 nb = normalize(n - (tang * (hu - h0) + bit * (hv - h0)) / eb * bump);

    float3 v = -cam.dir;
    float3 key = normalize(v * 0.6 + float3(0.3, 0.6, 0.2));
    float wrap = saturate((dot(nb, key) + 0.4) / 1.4);
    float3 hdir = normalize(key + v);
    float shin = lerp(140.0, 12.0, m.rough);
    float spec = pow(saturate(dot(nb, hdir)), shin) * (1.0 - m.rough) * 1.4;
    float fres = pow(1.0 - saturate(dot(nb, v)), 5.0);
    float back = pow(saturate(dot(-nb, key) * 0.5 + 0.5), 3.0) * m.glow;
    float ao = 0.0;
    for (int j = 1; j <= 3; j++) {
        float hh = 0.08 * j;
        ao += (hh - map2(pos + n * hh) / 0.6) * (1.0 / j);
    }
    ao = saturate(1.0 - ao * 2.0);

    float zl = cz + 1.5 + beat_t * 6.0;
    float3 lp = float3(path2(zl), zl);
    float ld = length(pos - lp);
    float pulse_light = drive * exp(-beat_t * 1.6) * 0.8 / (1.0 + ld * ld);

    float3 alb = lerp(m.albedo, (ramp(0.35) + ramp(0.65)) * 0.5, lod);
    float grey = dot(alb, float3(0.299, 0.587, 0.114));
    alb = max(lerp(float3(grey, grey, grey), alb, 1.35), 0.0);
    float3 col = alb * (0.35 + 0.95 * wrap) * (0.6 + 0.4 * ao);
    col += alb * back * 0.6 + alb * pulse_light * c1.rgb * 1.5;
    float shimmer = 1.0 + 1.5 * flux[7].z * drive;
    col += lerp(float3(1, 1, 1), c2.rgb, 0.3) * spec * shimmer * (1.0 - lod);
    col += ramp(0.8) * fres * 0.25;

    float fog = exp(-t * 0.09);
    float3 deep = c0.rgb * 0.06 + c1.rgb * 0.03;
    col = lerp(deep, col, fog);
    col *= 0.85 + 0.15 * smoothstep(0.0, 0.3, drive + level);
    float2 vv = i.uv - 0.5;
    col *= 1.0 - dot(vv, vv) * 0.6;
    col = col / (1.0 + 0.25 * max(col.r, max(col.g, col.b)));
    col = pow(saturate(col), 0.95);
    return float4(col, 1.0);
}
