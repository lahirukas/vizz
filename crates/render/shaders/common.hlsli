cbuffer Params : register(b0) {
    float2 res;     float time;      float pulse;
    float phase;    float conf;      float level;      float kick;
    float snare;    float hat;       float bpm;        float edit;
    float wind;     float bar_phase; float beat_in_bar; float drop;
    float whistle;  float pitch;     float drive;       float beat_t;
    float style;    float dark;      float dt;          float drop_eta;
    float travel;   float scene;     float jump;        float phrase;
    float quality;  float aux0;      float aux1;        float aux2;
    float4 sketch_pos;
    float4 sketch_info;
    float4 c0;
    float4 c1;
    float4 c2;
    float4 bands[8];
    float4 wave[32];
    float4 trail[8];
    float4 peaks[8];
    float4 flux[8];
};

struct PSIn { float4 pos : SV_Position; float2 uv : TEXCOORD0; };

Texture2D noise_tex : register(t1);
SamplerState noise_samp : register(s1);

// Value noise from a 256² texture whose G channel is R shifted by (37, 17), one z-slice per channel.
float tnoise(float3 x) {
    float3 p = floor(x);
    float3 f = frac(x);
    f = f * f * (3.0 - 2.0 * f);
    float2 uv = p.xy + float2(37.0, 17.0) * p.z + f.xy;
    float2 rg = noise_tex.SampleLevel(noise_samp, (uv + 0.5) / 256.0, 0).xy;
    return lerp(rg.x, rg.y, f.z);
}

float tnoise2(float2 x) { return noise_tex.SampleLevel(noise_samp, (x + 0.5) / 256.0, 0).x; }

static const float TAU = 6.2831853;
static const int NEON = 0;
static const int FLUENT = 1;
static const int SYNTH = 2;
static const int LIQUID = 3;

int style_id() { return (int)(style + 0.5); }
int quality_id() { return (int)(quality + 0.5); }

float band(uint i) { return bands[i >> 2][i & 3]; }
float peak(uint i) { return peaks[i >> 2][i & 3]; }
float wav(uint i)  { return wave[i >> 2][i & 3]; }

float band_at(float u) {
    float x = saturate(u) * 31.0;
    uint i = (uint)min(x, 30.0);
    return lerp(band(i), band(i + 1), x - i);
}

float peak_at(float u) {
    float x = saturate(u) * 31.0;
    uint i = (uint)min(x, 30.0);
    return lerp(peak(i), peak(i + 1), x - i);
}

float3 palette(float t) {
    t = frac(t) * 3.0;
    if (t < 1.0) return lerp(c0.rgb, c1.rgb, smoothstep(0.0, 1.0, t));
    if (t < 2.0) return lerp(c1.rgb, c2.rgb, smoothstep(1.0, 2.0, t));
    return lerp(c2.rgb, c0.rgb, smoothstep(2.0, 3.0, t));
}

bool custom_pal() { return c1.a > 1.5; }

float3 ink() { return dark > 0.5 ? float3(1, 1, 1) : float3(0.08, 0.09, 0.12); }

float aa(float d, float w, float px) { return smoothstep(w + px, w - px, d); }

float hash(float2 p) { return frac(sin(dot(p, float2(127.1, 311.7))) * 43758.5453); }

float noise(float2 p) {
    float2 i = floor(p);
    float2 f = frac(p);
    f = f * f * (3.0 - 2.0 * f);
    float a = hash(i), b = hash(i + float2(1, 0)), c = hash(i + float2(0, 1)), d = hash(i + float2(1, 1));
    return lerp(lerp(a, b, f.x), lerp(c, d, f.x), f.y);
}

// Damped spring: overshoots once and settles within ~0.4 s of the beat.
float spring() { return exp(-beat_t * 7.0) * cos(beat_t * 24.0); }

void over(inout float4 o, float3 rgb, float a) {
    a = saturate(a);
    o = o * (1.0 - a) + float4(rgb * a, a);
}

void add(inout float4 o, float3 rgb, float a) {
    o.rgb += rgb * saturate(a);
    o.a = saturate(o.a + saturate(a) * 0.5);
}

float4 finish(float4 o, float2 uv) {
    o = max(o, 0.0) * c0.a;
    o.a = saturate(max(o.a, max(o.r, max(o.g, o.b)) * 0.6));
    float tint = edit * 0.18;
    return o + float4(0.04, 0.05, 0.08, 1.0) * tint * (1.0 - o.a);
}
