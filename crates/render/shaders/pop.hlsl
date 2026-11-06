#include "tube.hlsli"

static const float3 POP[7] = {
    float3(0.60, 0.14, 0.98),
    float3(1.00, 0.92, 0.08),
    float3(0.08, 0.78, 1.00),
    float3(1.00, 0.22, 0.78),
    float3(0.22, 0.96, 0.22),
    float3(0.22, 0.30, 1.00),
    float3(1.00, 0.52, 0.06),
};
static const float3 INK = float3(0.015, 0.01, 0.03);

float3 pop_color(float idx) {
    int st = style_id();
    int i = (int)fmod(fmod(idx, 7.0) + 7.0, 7.0);
    float3 c = POP[i];
    if (st == SYNTH) c = POP[(i % 4 == 1) ? 3 : (i % 4 == 2 ? 2 : (i % 4 == 3 ? 6 : 0))];
    if (st == FLUENT) c = lerp(c, float3(1, 1, 1), 0.25);
    if (st == LIQUID) c = lerp(c, palette(idx / 7.0), 0.55);
    if (custom_pal()) c = lerp(palette(frac(i * 0.2857 + 0.05)), float3(1, 1, 1), (i % 3 == 2) ? 0.25 : 0.0);
    if (frac(sin(idx * 12.9898) * 43758.5453) < 0.07) c = INK;
    return c;
}

float3 surf_coord(float3 p) {
    float2 q = p.xy - path(p.z);
    float a = atan2(q.y, q.x);
    float twist = 0.1 * sin(floor(beat_pos() / 4.0) * 1.7 + scene);
    a += p.z * twist;
    return float3(cos(a) * 1.25, sin(a) * 1.25, p.z * 0.32);
}

// Band coordinate: integer part picks the colour, fractional part is the position across the band.
float band_x(float3 p) {
    float3 c = surf_coord(p);
    float bass = flux[7].x;
    float w1 = snoise(c * 0.8 + float3(0, 0, time * 0.07));
    float w2 = snoise(c * 0.8 + float3(5.3, 2.1, -time * 0.05));
    float3 c2 = c + float3(w1, w2, w1 - w2) * (1.7 + 0.4 * bass);
    float f = snoise(c2 * 0.7) * 0.6 + snoise(c2 * 1.6 + 3.0) * 0.3 + snoise(c2 * 3.4 + 9.0) * 0.1;
    float bp = beat_pos();
    float nb = 8.0 + 2.0 * sin(scene * 2.3) + 0.8 * flux[7].y;
    float steps = floor(bp) + smoothstep(0.0, 0.3, frac(bp));
    return f * nb - time * 0.12 - steps * 0.3 + 0.35 * sin(p.z * 0.8 - bp * TAU) * bass;
}

float band_h(float x) { return sqrt(saturate(sin(frac(x) * 3.14159265))); }

float4 main(PSIn i) : SV_Target {
    float scale = min(res.x, res.y);
    float2 p = (i.uv - 0.5) * res / scale * 2.0 * float2(1, -1);
    float bp = beat_pos();
    float bar = floor(bp / 4.0);
    float hit = pulse * drive;
    float cz = travel * 3.0;
    float3 eye = float3(path(cz), cz);
    float3 target = float3(path(cz + 2.2), cz + 2.2);
    float bank = -path_d2(cz + 1.5).x * 0.9;
    float roll = bank + TAU * smoothstep(0.0, 1.0, frac(scene)) + time * 0.03
        + 0.2 * drive * max(spring(), 0.0) * (frac(bar * 0.5) < 0.5 ? 1.0 : -1.0);
    float fov = 1.25 - 0.1 * hit - 0.3 * jump;
    Camera cam = look_from(eye, target, bend_screen(p), fov, roll);

    float bendk = 0.03 * sin(time * 0.17 + scene * 2.1) + 0.05 * hit;
    float pull = 0.006 * (0.5 + flux[7].x + drive);
    float3 rd = cam.dir;
    float t = 0.02;
    float3 pos = cam.pos + rd * t;
    [loop] for (int s = 0; s < 200; s++) {
        float d = map(pos);
        if (d < 0.001 * t) break;
        pos += rd * d;
        t += d;
        rd.xy = rot2(rd.xy, d * bendk);
        rd = normalize(rd - float3(pos.xy - path(pos.z), 0) * pull * d);
        if (t > 60.0) break;
    }
    for (int r = 0; r < 4; r++) pos += rd * map(pos);
    cam.dir = rd;
    float x = band_x(pos);
    float fw = fwidth(x);

    float2 e = float2(1.0, -1.0) * 0.002 * t;
    float3 n = normalize(e.xyy * map(pos + e.xyy) + e.yyx * map(pos + e.yyx) + e.yxy * map(pos + e.yxy) + e.xxx * map(pos + e.xxx));
    float eb = 0.01;
    float3 gx = float3(band_x(pos + float3(eb, 0, 0)), band_x(pos + float3(0, eb, 0)), band_x(pos + float3(0, 0, eb))) - x;
    float dh = 3.14159265 * cos(frac(x) * 3.14159265) * 0.5 / max(band_h(x), 0.15);
    float3 grad = gx / eb * dh;
    grad -= n * dot(grad, n);
    float bump = (0.035 + 0.02 * flux[7].x) * saturate(1.0 - fw * 1.5);
    float3 nb = normalize(n - grad * bump);

    float idx = floor(x) + bar;
    float3 base = pop_color(idx);
    float3 v = -cam.dir;
    float3 l1 = normalize(v + float3(0.4, 0.5, 0.0));
    float diff = saturate(dot(nb, l1));
    float3 h = normalize(l1 + v);
    float spec = pow(saturate(dot(nb, h)), 60.0);
    float spec2 = pow(saturate(dot(nb, normalize(v + float3(-0.5, -0.3, 0.2)))), 18.0);
    float ao = 0.0;
    for (int a = 1; a <= 3; a++) {
        float hh = 0.08 * a;
        ao += (hh - map(pos + n * hh) / 0.55) * (1.0 / a);
    }
    ao = saturate(1.0 - ao * 2.0);
    float fres = pow(1.0 - saturate(dot(nb, v)), 4.0);

    float3 col = base * (0.8 + 0.35 * diff) * (0.8 + 0.2 * ao);
    float glossy = (base.r + base.g + base.b) > 0.2 ? 1.0 : 0.6;
    col += float3(1, 1, 1) * (spec * 0.9 + spec2 * 0.12) * glossy * saturate(1.0 - fw * 2.0);
    col += pop_color(idx + 2.0) * fres * 0.35;

    float fr = frac(x);
    float dline = min(fr, 1.0 - fr);
    float lw = 0.055 + 0.05 * hit;
    float outline = (1.0 - smoothstep(lw, lw + fw + 0.01, dline)) * saturate(1.3 - fw * 1.6);
    float sub = 1.0 - smoothstep(0.012, 0.012 + fw * 0.7, abs(dline - lw - 0.07));
    col = lerp(col, col * 0.45, sub * saturate(1.0 - fw * 2.5));
    float3 film = 0.5 + 0.5 * cos(TAU * (fres * 1.5 + x * 0.08 + time * 0.1 + float3(0.0, 0.33, 0.67)));
    col += film * fres * 0.45 * glossy;
    col *= 1.0 - 0.45 * exp(-fr * 9.0) * saturate(1.0 - fw * 2.0);
    float zl = cz + beat_t * 10.0 + 1.0;
    float3 lp = float3(path(zl), zl);
    float ld = length(pos - lp);
    float3 lcol = pop_color(floor(bp) * 3.0 + 1.0);
    col += lcol * base * 2.2 / (1.0 + ld * ld * 0.8) * drive * exp(-beat_t * 1.2);
    col = lerp(col, INK, outline);
    float far = saturate(fw * 1.2 - 0.25);
    col = lerp(col, (pop_color(idx) + pop_color(idx + 1.0)) * 0.4, far);

    float spark = smoothstep(0.88, 0.92, snoise(surf_coord(pos) * 6.0 + time * 0.6)) * (0.15 + 1.3 * flux[7].z);
    col = lerp(col, float3(1, 1, 1), saturate(spark) * saturate(1.0 - fw * 3.0) * (1.0 - outline));

    col *= 0.3 + 0.7 * exp(-t * 0.055);
    col *= 0.9 + 0.3 * hit;
    col *= 0.7 + 0.3 * smoothstep(0.0, 0.25, drive + level);
    col = lerp(col, float3(1, 1, 1), jump * 0.35);
    col = saturate(col);
    col = col / (1.0 - 0.24 * max(col.r, max(col.g, col.b)));
    return float4(col, 1.0);
}
