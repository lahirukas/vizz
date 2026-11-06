#include "tube.hlsli"

static const float BEAT_DIST = 3.2;
float3 hallucination(float3 p) {
    float2 q = p.xy - path(p.z);
    float a = atan2(q.y, q.x);
    float folds = 6.0 + 2.0 * fmod(floor(scene), 3.0);
    float k = abs(frac(a * folds / TAU) - 0.5) * 2.0;
    float ka = k * TAU / folds;
    float zt = p.z * 0.35;
    float bp = beat_pos();
    float breathe = 1.0 + 0.12 * flux[7].x + 0.08 * pulse * drive;
    float3 c = float3(cos(ka) * 1.4 * breathe, sin(ka) * 1.4 * breathe, zt - time * 0.15);
    float w1 = tnoise(c * 1.1 + float3(0, 0, time * 0.1));
    float w2 = tnoise(c * 1.1 + float3(5.2, 1.3, -time * 0.08));
    float3 c2 = c + float3(w1, w2, 0) * 1.6;
    float n = tnoise(c2 * 0.9) * 0.6 + tnoise(c2 * 2.1 + 7.0) * 0.3 + tnoise(c2 * 4.3) * 0.1;
    float shift = time * 0.08 + floor(bp) * 0.07 + scene * 0.25;
    float bands = n * 7.0 - shift * 3.0;
    float band_line = 1.0 - abs(frac(bands) - 0.5) * 2.0;
    float glow_line = pow(band_line, 14.0);
    float3 fill = trip(n * 1.6 + zt * 0.05 + shift) * (0.12 + 0.25 * smoothstep(0.3, 0.9, band_line));
    float3 edge = trip(n * 1.6 + 0.5 + shift) * glow_line * (1.0 + 1.6 * flux[7].z + 1.2 * pulse * drive);
    float eye = pow(saturate(1.0 - abs(frac(n * 3.0 + zt * 0.2 - time * 0.2) - 0.5) * 2.0), 40.0);
    return (fill + edge * 1.4 + trip(n + 0.25) * eye * 0.8) * (0.55 + 0.6 * level);
}

float3 ring_color(int k) {
    int bib = (int)fmod(beat_in_bar - 1.0 + k + 1.0 + 64.0, 4.0);
    float3 c = trip(beat_pos() * 0.11 + k * 0.17);
    return bib == 0 ? lerp(c, float3(1, 1, 1), 0.3) * 1.5 : c;
}

float ring_z(int k, float cz) { return cz + (k + 1.0 - phase) * BEAT_DIST; }

float ring_power(int k) {
    float ahead = (k + 1.0 - phase);
    return (0.1 + 0.9 * drive) * exp(-ahead * 0.3) * (1.0 + 2.0 * pulse * drive * step(ahead, 1.0));
}

float3 ring_light(float3 p, float cz) {
    float3 l = 0;
    for (int k = 0; k < 6; k++) {
        float dz = abs(p.z - ring_z(k, cz));
        l += ring_color(k) * ring_power(k) * exp(-dz * 1.6);
    }
    return l;
}

float3 haze(float t, float3 rd) {
    float n = tnoise(float3(rd.xy * 3.0, t * 0.05 + time * 0.1));
    return trip(t * 0.03 + time * 0.05 + n * 0.3) * (0.05 + 0.06 * level);
}

float4 main(PSIn i) : SV_Target {
    int st = style_id();
    float scale = min(res.x, res.y);
    float2 p = (i.uv - 0.5) * res / scale * 2.0 * float2(1, -1);
    float speed = aux2;
    float cz = travel * 3.0;
    float3 eye = float3(path(cz), cz);
    float3 target = float3(path(cz + 2.2), cz + 2.2);
    float bank = -path_d2(cz + 1.5).x * 0.9;
    float roll = bank + TAU * smoothstep(0.0, 1.0, frac(scene)) + time * 0.04
        + 0.2 * drive * max(spring(), 0.0) * (frac(bar_phase * 0.5) < 0.5 ? 1.0 : -1.0);
    float fov = 1.2 - 0.12 * pulse * drive - 0.35 * jump + 0.05 * sin(time * 0.7);
    Camera cam = look_from(eye, target, p, fov, roll);

    float t = 0.02;
    bool hit = false;
    float3 scatter = 0;
    int steps = march_steps();
    [loop] for (int s = 0; s < 100; s++) {
        if (s >= steps) break;
        float3 pos = cam.pos + cam.dir * t;
        float d = map(pos);
        float rk = (pos.z - cz) / BEAT_DIST + phase - 1.0;
        int k0 = (int)clamp(floor(rk), 0.0, 5.0);
        float2 q = pos.xy - path(pos.z);
        float a = atan2(q.y, q.x);
        float rr = length(q);
        for (int j = 0; j < 2; j++) {
            int k = min(k0 + j, 5);
            float zk = ring_z(k, cz);
            float dz = abs(pos.z - zk);
            float dist = length(float2(dz, rr - radius_at(a, zk) * 0.82));
            scatter += ring_color(k) * ring_power(k) * exp(-dist * 9.0) * min(d, 0.25) * 0.8;
        }
        if (d < 0.0015 * t) {
            hit = true;
            break;
        }
        t += d;
        if (t > 40.0) break;
    }

    float3 col = haze(40.0, cam.dir);
    if (hit) {
        float3 pos = cam.pos + cam.dir * t;
        float2 e = float2(1.0, -1.0) * 0.0025 * t;
        float3 n = normalize(e.xyy * map(pos + e.xyy) + e.yyx * map(pos + e.yyx) + e.yxy * map(pos + e.yxy) + e.xxx * map(pos + e.xxx));
        float ao = 0.0;
        for (int a = 1; a <= 3; a++) {
            float h = 0.06 * a;
            ao += (h - map(pos + n * h) / 0.55) * (1.0 / a);
        }
        ao = saturate(1.0 - ao * 2.5);
        float3 v = -cam.dir;
        float fres = pow(1.0 - saturate(dot(n, v)), 3.0);
        float head = saturate(dot(n, v)) / (1.0 + t * 0.15);
        float3 pattern = hallucination(pos);
        float3 surf = pattern * (0.5 + 0.7 * ao) + pattern * ring_light(pos, cz) * 1.5 + trip(0.6) * head * 0.05;

        float3 rd = reflect(cam.dir, n);
        float rt = 0.05;
        float3 rp = pos + rd * rt;
        [loop] for (int r = 0; r < 10; r++) {
            float dd = map(rp);
            if (dd < 0.01) break;
            rt += dd;
            rp = pos + rd * rt;
        }
        float gloss = st == LIQUID ? 0.8 : (st == FLUENT ? 0.25 : 0.5);
        surf += (hallucination(rp) * 0.7 + ring_light(rp, cz) * 0.2) * fres * gloss * ao;

        float fog = exp(-t * (st == FLUENT ? 0.1 : 0.06));
        col = lerp(haze(t, cam.dir), surf, fog);
    }
    col += scatter * (st == FLUENT ? 0.5 : 1.0);
    col += trip(time * 0.2) * jump * 0.4;
    return float4(max(col, 0.0), 1.0);
}
