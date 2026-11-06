#include "common.hlsli"

float2 rot2(float2 p, float a) {
    float c = cos(a), s = sin(a);
    return float2(p.x * c - p.y * s, p.x * s + p.y * c);
}

float3 hsv3(float h, float s, float v) {
    float3 k = saturate(abs(frac(h + float3(0.0, 2.0 / 3.0, 1.0 / 3.0)) * 6.0 - 3.0) - 1.0);
    return v * lerp(float3(1, 1, 1), k, s);
}

float sd_capsule(float3 p, float3 a, float3 b, float r) {
    float3 pa = p - a, ba = b - a;
    float h = saturate(dot(pa, ba) / dot(ba, ba));
    return length(pa - ba * h) - r;
}

float sd_ellipse2(float2 p, float2 c, float2 r, float a) {
    float2 q = rot2(p - c, a) / r;
    return (length(q) - 1.0) * min(r.x, r.y);
}

float smin(float a, float b, float k) {
    float h = saturate(0.5 + 0.5 * (b - a) / k);
    return lerp(b, a, h) - k * h * (1.0 - h);
}

int march_steps() { return 40 + 16 * quality_id(); }

struct Camera {
    float3 pos;
    float3 dir;
};

Camera look_from(float3 eye, float3 target, float2 p, float fov, float roll) {
    float3 f = normalize(target - eye);
    float3 r = normalize(cross(float3(sin(roll), cos(roll), 0), f));
    float3 u = cross(f, r);
    Camera c;
    c.pos = eye;
    c.dir = normalize(f * fov + r * p.x + u * p.y);
    return c;
}
