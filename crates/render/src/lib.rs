pub mod png;

use windows::core::{Interface, Result};
use windows::Win32::Foundation::{HMODULE, HWND};
use windows::Win32::Graphics::Direct3D::*;
use windows::Win32::Graphics::Direct3D11::*;
use windows::Win32::Graphics::DirectComposition::*;
use windows::Win32::Graphics::Dxgi::Common::*;
use windows::Win32::Graphics::Dxgi::*;

const VS_FULLSCREEN: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/fullscreen.cso"));
const PS_RING: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/ring.cso"));
const PS_PRESENT: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/present.cso"));
const PS_FLUX: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/flux.cso"));
const PS_FLUX_POST: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/flux_post.cso"));
const PS_COSMOS_POST: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/cosmos_post.cso"));
const PS_WARP: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/warp.cso"));
const PS_POP: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/pop.cso"));
const PS_GEO: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/geo.cso"));
const PS_POP2: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/pop2.cso"));
const PS_SKETCH: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/sketch.cso"));
const PS_SKETCH_POST: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/sketch_post.cso"));

pub const FORMAT: DXGI_FORMAT = DXGI_FORMAT_B8G8R8A8_UNORM;
const FEEDBACK_FORMAT: DXGI_FORMAT = DXGI_FORMAT_R16G16B16A16_FLOAT;
const NOISE_SIZE: u32 = 256;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WidgetKind {
    Flux,
    Warp,
    Pop,
    Pop2,
    Geo,
    Ring,
    Sketch,
}

impl WidgetKind {
    pub const ALL: [WidgetKind; 7] = [
        WidgetKind::Flux,
        WidgetKind::Warp,
        WidgetKind::Pop,
        WidgetKind::Pop2,
        WidgetKind::Geo,
        WidgetKind::Ring,
        WidgetKind::Sketch,
    ];

    pub fn name(self) -> &'static str {
        match self {
            WidgetKind::Flux => "flux",
            WidgetKind::Warp => "warp",
            WidgetKind::Pop => "pop",
            WidgetKind::Pop2 => "pop2",
            WidgetKind::Geo => "geo",
            WidgetKind::Ring => "ring",
            WidgetKind::Sketch => "sketch",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            WidgetKind::Flux => "Flux (flowing presets)",
            WidgetKind::Warp => "Warp Tunnel (3D)",
            WidgetKind::Pop => "Pop Tunnel (3D liquid)",
            WidgetKind::Pop2 => "Pop Tunnel II (nature)",
            WidgetKind::Geo => "Neon Geometry (3D)",
            WidgetKind::Ring => "Pulse Ring",
            WidgetKind::Sketch => "Whistle Sketch",
        }
    }

    pub fn from_name(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|k| k.name() == s)
    }

    pub fn two_pass(self) -> bool {
        self.post().is_some()
    }

    pub fn wide(self) -> bool {
        matches!(self, WidgetKind::Flux | WidgetKind::Warp | WidgetKind::Pop | WidgetKind::Pop2 | WidgetKind::Geo)
    }

    pub fn default_size(self) -> (i32, i32) {
        match self {
            WidgetKind::Flux | WidgetKind::Warp | WidgetKind::Pop | WidgetKind::Pop2 | WidgetKind::Geo => (480, 270),
            WidgetKind::Sketch => (360, 360),
            _ => (300, 300),
        }
    }

    fn shader(self) -> &'static [u8] {
        match self {
            WidgetKind::Flux => PS_FLUX,
            WidgetKind::Warp => PS_WARP,
            WidgetKind::Pop => PS_POP,
            WidgetKind::Pop2 => PS_POP2,
            WidgetKind::Geo => PS_GEO,
            WidgetKind::Ring => PS_RING,
            WidgetKind::Sketch => PS_SKETCH,
        }
    }

    fn post(self) -> Option<&'static [u8]> {
        match self {
            WidgetKind::Flux => Some(PS_FLUX_POST),
            WidgetKind::Warp => Some(PS_COSMOS_POST),
            WidgetKind::Pop | WidgetKind::Pop2 | WidgetKind::Geo => Some(PS_PRESENT),
            WidgetKind::Sketch => Some(PS_SKETCH_POST),
            WidgetKind::Ring => None,
        }
    }

    fn inner_pixels(self) -> [u32; 4] {
        match self {
            WidgetKind::Flux => [120_000, 250_000, 500_000, u32::MAX],
            WidgetKind::Warp => [100_000, 160_000, 300_000, u32::MAX],
            WidgetKind::Pop | WidgetKind::Pop2 | WidgetKind::Geo => [250_000, 500_000, 1_000_000, u32::MAX],
            WidgetKind::Sketch => [300_000, 600_000, 1_000_000, u32::MAX],
            WidgetKind::Ring => [200_000, 400_000, 900_000, u32::MAX],
        }
    }

    fn inner_size(self, q: Quality, w: u32, h: u32) -> (u32, u32) {
        let target = self.inner_pixels()[q.index() as usize] as f64;
        let scale = (target / (w as f64 * h as f64).max(1.0)).sqrt().min(1.0);
        (((w as f64 * scale) as u32).max(16), ((h as f64 * scale) as u32).max(16))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Quality {
    Low,
    Medium,
    High,
    Ultra,
}

impl Quality {
    pub const ALL: [Quality; 4] = [Quality::Low, Quality::Medium, Quality::High, Quality::Ultra];

    pub fn name(self) -> &'static str {
        match self {
            Quality::Low => "low",
            Quality::Medium => "medium",
            Quality::High => "high",
            Quality::Ultra => "ultra",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Quality::Low => "Low (old PCs, 30 fps)",
            Quality::Medium => "Medium",
            Quality::High => "High",
            Quality::Ultra => "Ultra (full resolution)",
        }
    }

    pub fn from_name(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|k| k.name() == s)
    }

    pub fn index(self) -> f32 {
        Self::ALL.iter().position(|&s| s == self).unwrap_or(1) as f32
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Style {
    Neon,
    Fluent,
    Synth,
    Liquid,
}

impl Style {
    pub const ALL: [Style; 4] = [Style::Neon, Style::Fluent, Style::Synth, Style::Liquid];

    pub fn name(self) -> &'static str {
        match self {
            Style::Neon => "neon",
            Style::Fluent => "fluent",
            Style::Synth => "synthwave",
            Style::Liquid => "liquid",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Style::Neon => "Neon Glass",
            Style::Fluent => "Fluent Minimal",
            Style::Synth => "Synthwave",
            Style::Liquid => "Liquid",
        }
    }

    pub fn from_name(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|k| k.name() == s)
    }

    pub fn index(self) -> f32 {
        Self::ALL.iter().position(|&s| s == self).unwrap_or(0) as f32
    }
}

#[repr(C, align(16))]
#[derive(Clone, Copy, Debug)]
pub struct Params {
    pub res: [f32; 2],
    pub time: f32,
    pub pulse: f32,
    pub phase: f32,
    pub conf: f32,
    pub level: f32,
    pub kick: f32,
    pub snare: f32,
    pub hat: f32,
    pub bpm: f32,
    pub edit: f32,
    pub wind: f32,
    pub bar_phase: f32,
    pub beat_in_bar: f32,
    pub drop: f32,
    pub whistle: f32,
    pub pitch: f32,
    pub drive: f32,
    pub beat_t: f32,
    pub style: f32,
    pub dark: f32,
    pub dt: f32,
    pub drop_eta: f32,
    pub travel: f32,
    pub scene: f32,
    pub jump: f32,
    pub phrase: f32,
    pub quality: f32,
    pub aux0: f32,
    pub aux1: f32,
    pub aux2: f32,
    pub sketch_pos: [f32; 4],
    pub sketch_info: [f32; 4],
    pub c0: [f32; 4],
    pub c1: [f32; 4],
    pub c2: [f32; 4],
    pub bands: [f32; 32],
    pub wave: [f32; 128],
    pub trail: [f32; 32],
    pub peaks: [f32; 32],
    pub flux: [f32; 32],
}

impl Default for Params {
    fn default() -> Self {
        Self {
            res: [1.0, 1.0],
            time: 0.0,
            pulse: 0.0,
            phase: 0.0,
            conf: 0.0,
            level: 0.0,
            kick: 0.0,
            snare: 0.0,
            hat: 0.0,
            bpm: 0.0,
            edit: 0.0,
            wind: 0.0,
            bar_phase: 0.0,
            beat_in_bar: 1.0,
            drop: 0.0,
            whistle: 0.0,
            pitch: 0.5,
            drive: 0.0,
            beat_t: 10.0,
            style: 0.0,
            dark: 1.0,
            dt: 1.0 / 60.0,
            drop_eta: -1.0,
            travel: 0.0,
            scene: 0.0,
            jump: 0.0,
            phrase: 0.0,
            quality: 1.0,
            aux0: 0.0,
            aux1: 0.0,
            aux2: 0.0,
            sketch_pos: [0.0; 4],
            sketch_info: [0.0; 4],
            c0: [0.0, 0.85, 1.0, 1.0],
            c1: [0.55, 0.3, 1.0, 1.0],
            c2: [1.0, 0.25, 0.7, 1.0],
            bands: [0.0; 32],
            wave: [0.0; 128],
            trail: [0.0; 32],
            peaks: [0.0; 32],
            flux: [0.0; 32],
        }
    }
}

pub struct Gpu {
    pub device: ID3D11Device,
    pub ctx: ID3D11DeviceContext,
    pub factory: IDXGIFactory2,
    pub dcomp: IDCompositionDevice,
    pub feature_level: D3D_FEATURE_LEVEL,
    pub warp: bool,
    vs: ID3D11VertexShader,
    ps: Vec<(WidgetKind, ID3D11PixelShader)>,
    posts: Vec<(WidgetKind, ID3D11PixelShader)>,
    cbuf: ID3D11Buffer,
    blend: ID3D11BlendState,
    sampler: ID3D11SamplerState,
    noise_sampler: ID3D11SamplerState,
    noise: ID3D11ShaderResourceView,
}

fn noise_texels() -> Vec<u8> {
    let n = NOISE_SIZE as usize;
    let mut state = 0x9E37_79B9_7F4A_7C15u64;
    let mut r = vec![0u8; n * n];
    for v in r.iter_mut() {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        *v = (state >> 56) as u8;
    }
    let mut out = vec![0u8; n * n * 4];
    for y in 0..n {
        for x in 0..n {
            let o = (y * n + x) * 4;
            out[o] = r[y * n + x];
            out[o + 1] = r[((y + 17) % n) * n + (x + 37) % n];
            out[o + 3] = 255;
        }
    }
    out
}

struct Feedback {
    kind: WidgetKind,
    quality: Quality,
    width: u32,
    height: u32,
    rtv: [ID3D11RenderTargetView; 2],
    srv: [ID3D11ShaderResourceView; 2],
    cur: usize,
}

impl Feedback {
    fn new(gpu: &Gpu, kind: WidgetKind, quality: Quality, w: u32, h: u32) -> Result<Self> {
        let (width, height) = kind.inner_size(quality, w, h);
        let desc = D3D11_TEXTURE2D_DESC {
            Width: width,
            Height: height,
            MipLevels: 1,
            ArraySize: 1,
            Format: FEEDBACK_FORMAT,
            SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
            Usage: D3D11_USAGE_DEFAULT,
            BindFlags: (D3D11_BIND_RENDER_TARGET.0 | D3D11_BIND_SHADER_RESOURCE.0) as u32,
            CPUAccessFlags: 0,
            MiscFlags: 0,
        };
        let make = || -> Result<(ID3D11RenderTargetView, ID3D11ShaderResourceView)> {
            unsafe {
                let mut tex = None;
                gpu.device.CreateTexture2D(&desc, None, Some(&mut tex))?;
                let tex = tex.unwrap();
                let (mut rtv, mut srv) = (None, None);
                gpu.device.CreateRenderTargetView(&tex, None, Some(&mut rtv))?;
                gpu.device.CreateShaderResourceView(&tex, None, Some(&mut srv))?;
                let rtv = rtv.unwrap();
                gpu.ctx.ClearRenderTargetView(&rtv, &[0.0; 4]);
                Ok((rtv, srv.unwrap()))
            }
        };
        let (a, b) = (make()?, make()?);
        Ok(Self { kind, quality, width, height, rtv: [a.0, b.0], srv: [a.1, b.1], cur: 0 })
    }
}

impl Gpu {
    pub fn new(force_warp: bool) -> Result<Self> {
        let levels = [D3D_FEATURE_LEVEL_11_0, D3D_FEATURE_LEVEL_10_1, D3D_FEATURE_LEVEL_10_0];
        let attempt = |kind: D3D_DRIVER_TYPE| -> Result<(ID3D11Device, ID3D11DeviceContext, D3D_FEATURE_LEVEL)> {
            let mut dev = None;
            let mut ctx = None;
            let mut fl = D3D_FEATURE_LEVEL::default();
            unsafe {
                D3D11CreateDevice(
                    None,
                    kind,
                    HMODULE::default(),
                    D3D11_CREATE_DEVICE_BGRA_SUPPORT | D3D11_CREATE_DEVICE_SINGLETHREADED,
                    Some(&levels),
                    D3D11_SDK_VERSION,
                    Some(&mut dev),
                    Some(&mut fl),
                    Some(&mut ctx),
                )?;
            }
            Ok((dev.unwrap(), ctx.unwrap(), fl))
        };
        let (warp, (device, ctx, feature_level)) = if force_warp {
            (true, attempt(D3D_DRIVER_TYPE_WARP)?)
        } else {
            match attempt(D3D_DRIVER_TYPE_HARDWARE) {
                Ok(v) => (false, v),
                Err(_) => (true, attempt(D3D_DRIVER_TYPE_WARP)?),
            }
        };

        unsafe {
            let dxgi: IDXGIDevice = device.cast()?;
            let adapter = dxgi.GetAdapter()?;
            let factory: IDXGIFactory2 = adapter.GetParent()?;
            let dcomp: IDCompositionDevice = DCompositionCreateDevice(&dxgi)?;

            let mut vs = None;
            device.CreateVertexShader(VS_FULLSCREEN, None, Some(&mut vs))?;
            let pixel = |code: &[u8]| -> Result<ID3D11PixelShader> {
                let mut p = None;
                device.CreatePixelShader(code, None, Some(&mut p))?;
                Ok(p.unwrap())
            };
            let mut ps = Vec::new();
            let mut posts = Vec::new();
            for kind in WidgetKind::ALL {
                ps.push((kind, pixel(kind.shader())?));
                if let Some(code) = kind.post() {
                    posts.push((kind, pixel(code)?));
                }
            }

            let mut cbuf = None;
            let desc = D3D11_BUFFER_DESC {
                ByteWidth: std::mem::size_of::<Params>() as u32,
                Usage: D3D11_USAGE_DEFAULT,
                BindFlags: D3D11_BIND_CONSTANT_BUFFER.0 as u32,
                ..Default::default()
            };
            device.CreateBuffer(&desc, None, Some(&mut cbuf))?;
            let mut bd = D3D11_BLEND_DESC::default();
            bd.RenderTarget[0] = D3D11_RENDER_TARGET_BLEND_DESC {
                BlendEnable: false.into(),
                SrcBlend: D3D11_BLEND_ONE,
                DestBlend: D3D11_BLEND_ZERO,
                BlendOp: D3D11_BLEND_OP_ADD,
                SrcBlendAlpha: D3D11_BLEND_ONE,
                DestBlendAlpha: D3D11_BLEND_ZERO,
                BlendOpAlpha: D3D11_BLEND_OP_ADD,
                RenderTargetWriteMask: D3D11_COLOR_WRITE_ENABLE_ALL.0 as u8,
            };
            let mut blend = None;
            device.CreateBlendState(&bd, Some(&mut blend))?;
            let sd = D3D11_SAMPLER_DESC {
                Filter: D3D11_FILTER_MIN_MAG_MIP_LINEAR,
                AddressU: D3D11_TEXTURE_ADDRESS_CLAMP,
                AddressV: D3D11_TEXTURE_ADDRESS_CLAMP,
                AddressW: D3D11_TEXTURE_ADDRESS_CLAMP,
                MaxLOD: f32::MAX,
                ComparisonFunc: D3D11_COMPARISON_NEVER,
                ..Default::default()
            };
            let mut sampler = None;
            device.CreateSamplerState(&sd, Some(&mut sampler))?;
            let wrap = D3D11_SAMPLER_DESC {
                AddressU: D3D11_TEXTURE_ADDRESS_WRAP,
                AddressV: D3D11_TEXTURE_ADDRESS_WRAP,
                AddressW: D3D11_TEXTURE_ADDRESS_WRAP,
                ..sd
            };
            let mut noise_sampler = None;
            device.CreateSamplerState(&wrap, Some(&mut noise_sampler))?;
            let texels = noise_texels();
            let nd = D3D11_TEXTURE2D_DESC {
                Width: NOISE_SIZE,
                Height: NOISE_SIZE,
                MipLevels: 1,
                ArraySize: 1,
                Format: DXGI_FORMAT_R8G8B8A8_UNORM,
                SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
                Usage: D3D11_USAGE_IMMUTABLE,
                BindFlags: D3D11_BIND_SHADER_RESOURCE.0 as u32,
                CPUAccessFlags: 0,
                MiscFlags: 0,
            };
            let init = D3D11_SUBRESOURCE_DATA {
                pSysMem: texels.as_ptr() as *const _,
                SysMemPitch: NOISE_SIZE * 4,
                SysMemSlicePitch: 0,
            };
            let mut ntex = None;
            device.CreateTexture2D(&nd, Some(&init), Some(&mut ntex))?;
            let mut noise = None;
            device.CreateShaderResourceView(&ntex.unwrap(), None, Some(&mut noise))?;

            Ok(Self {
                device,
                ctx,
                factory,
                dcomp,
                feature_level,
                warp,
                vs: vs.unwrap(),
                ps,
                posts,
                cbuf: cbuf.unwrap(),
                blend: blend.unwrap(),
                sampler: sampler.unwrap(),
                noise_sampler: noise_sampler.unwrap(),
                noise: noise.unwrap(),
            })
        }
    }

    pub fn describe(&self) -> String {
        let fl = match self.feature_level {
            D3D_FEATURE_LEVEL_11_0 => "11_0",
            D3D_FEATURE_LEVEL_10_1 => "10_1",
            D3D_FEATURE_LEVEL_10_0 => "10_0",
            _ => "?",
        };
        format!("D3D11 FL {fl}{}", if self.warp { " (WARP)" } else { "" })
    }

    fn pass(
        &self,
        rtv: &ID3D11RenderTargetView,
        ps: &ID3D11PixelShader,
        p: &Params,
        src: Option<&ID3D11ShaderResourceView>,
    ) {
        unsafe {
            let c = &self.ctx;
            c.ClearRenderTargetView(rtv, &[0.0; 4]);
            c.OMSetRenderTargets(Some(&[Some(rtv.clone())]), None);
            c.OMSetBlendState(&self.blend, None, 0xffff_ffff);
            c.RSSetViewports(Some(&[D3D11_VIEWPORT {
                TopLeftX: 0.0,
                TopLeftY: 0.0,
                Width: p.res[0],
                Height: p.res[1],
                MinDepth: 0.0,
                MaxDepth: 1.0,
            }]));
            c.UpdateSubresource(&self.cbuf, 0, None, p as *const Params as *const _, 0, 0);
            c.IASetInputLayout(None);
            c.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
            c.VSSetShader(&self.vs, None);
            c.PSSetShader(ps, None);
            c.PSSetConstantBuffers(0, Some(&[Some(self.cbuf.clone())]));
            c.PSSetSamplers(0, Some(&[Some(self.sampler.clone()), Some(self.noise_sampler.clone())]));
            c.PSSetShaderResources(0, Some(&[src.cloned(), Some(self.noise.clone())]));
            c.Draw(3, 0);
            c.PSSetShaderResources(0, Some(&[None]));
        }
    }

    fn shader(&self, kind: WidgetKind) -> &ID3D11PixelShader {
        &self.ps.iter().find(|(k, _)| *k == kind).unwrap().1
    }

    fn draw(&self, rtv: &ID3D11RenderTargetView, kind: WidgetKind, p: &Params, fb: Option<&mut Feedback>) {
        match (fb, self.posts.iter().find(|(k, _)| *k == kind)) {
            (Some(fb), Some((_, post))) => {
                let next = 1 - fb.cur;
                let mut inner = *p;
                inner.res = [fb.width as f32, fb.height as f32];
                self.pass(&fb.rtv[next], self.shader(kind), &inner, Some(&fb.srv[fb.cur]));
                fb.cur = next;
                self.pass(rtv, post, p, Some(&fb.srv[next]));
            }
            _ => self.pass(rtv, self.shader(kind), p, None),
        }
    }

    pub fn render_offscreen(
        &self,
        w: u32,
        h: u32,
        kind: WidgetKind,
        mut frame: impl FnMut(u32) -> Params,
        frames: u32,
    ) -> Result<Vec<u8>> {
        unsafe {
            let mut desc = D3D11_TEXTURE2D_DESC {
                Width: w,
                Height: h,
                MipLevels: 1,
                ArraySize: 1,
                Format: FORMAT,
                SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
                Usage: D3D11_USAGE_DEFAULT,
                BindFlags: D3D11_BIND_RENDER_TARGET.0 as u32,
                CPUAccessFlags: 0,
                MiscFlags: 0,
            };
            let mut tex = None;
            self.device.CreateTexture2D(&desc, None, Some(&mut tex))?;
            let tex = tex.unwrap();
            let mut rtv = None;
            self.device.CreateRenderTargetView(&tex, None, Some(&mut rtv))?;
            let rtv = rtv.unwrap();
            let mut fb = if kind.two_pass() { Some(Feedback::new(self, kind, Quality::High, w, h)?) } else { None };
            for i in 0..frames.max(1) {
                self.draw(&rtv, kind, &frame(i), fb.as_mut());
            }
            desc.Usage = D3D11_USAGE_STAGING;
            desc.BindFlags = 0;
            desc.CPUAccessFlags = D3D11_CPU_ACCESS_READ.0 as u32;
            let mut staging = None;
            self.device.CreateTexture2D(&desc, None, Some(&mut staging))?;
            let staging = staging.unwrap();
            self.ctx.CopyResource(&staging, &tex);
            let mut m = D3D11_MAPPED_SUBRESOURCE::default();
            self.ctx.Map(&staging, 0, D3D11_MAP_READ, 0, Some(&mut m))?;
            let mut out = vec![0u8; (w * h * 4) as usize];
            for y in 0..h as usize {
                let row =
                    std::slice::from_raw_parts((m.pData as *const u8).add(y * m.RowPitch as usize), w as usize * 4);
                for x in 0..w as usize {
                    let (b, g, r, a) = (row[x * 4], row[x * 4 + 1], row[x * 4 + 2], row[x * 4 + 3]);
                    let un =
                        |c: u8| if a == 0 { 0 } else { ((c as u32 * 255 + a as u32 / 2) / a as u32).min(255) as u8 };
                    let o = &mut out[(y * w as usize + x) * 4..][..4];
                    o.copy_from_slice(&[un(r), un(g), un(b), a]);
                }
            }
            self.ctx.Unmap(&staging, 0);
            Ok(out)
        }
    }
}

pub struct Surface {
    pub swapchain: IDXGISwapChain1,
    rtv: Option<ID3D11RenderTargetView>,
    feedback: Option<Feedback>,
    _target: IDCompositionTarget,
    _visual: IDCompositionVisual,
    pub width: u32,
    pub height: u32,
}

impl Surface {
    pub fn new(gpu: &Gpu, hwnd: HWND, width: u32, height: u32) -> Result<Self> {
        unsafe {
            let desc = DXGI_SWAP_CHAIN_DESC1 {
                Width: width,
                Height: height,
                Format: FORMAT,
                Stereo: false.into(),
                SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
                BufferUsage: DXGI_USAGE_RENDER_TARGET_OUTPUT,
                BufferCount: 2,
                Scaling: DXGI_SCALING_STRETCH,
                SwapEffect: DXGI_SWAP_EFFECT_FLIP_SEQUENTIAL,
                AlphaMode: DXGI_ALPHA_MODE_PREMULTIPLIED,
                Flags: 0,
            };
            let swapchain = gpu.factory.CreateSwapChainForComposition(&gpu.device, &desc, None)?;
            let target = gpu.dcomp.CreateTargetForHwnd(hwnd, true)?;
            let visual = gpu.dcomp.CreateVisual()?;
            visual.SetContent(&swapchain)?;
            target.SetRoot(&visual)?;
            gpu.dcomp.Commit()?;
            let mut s = Self { swapchain, rtv: None, feedback: None, _target: target, _visual: visual, width, height };
            s.make_rtv(gpu)?;
            Ok(s)
        }
    }

    fn make_rtv(&mut self, gpu: &Gpu) -> Result<()> {
        unsafe {
            let buf: ID3D11Texture2D = self.swapchain.GetBuffer(0)?;
            let mut rtv = None;
            gpu.device.CreateRenderTargetView(&buf, None, Some(&mut rtv))?;
            self.rtv = rtv;
        }
        Ok(())
    }

    pub fn resize(&mut self, gpu: &Gpu, width: u32, height: u32) -> Result<()> {
        if width == self.width && height == self.height {
            return Ok(());
        }
        self.rtv = None;
        self.feedback = None;
        unsafe {
            gpu.ctx.OMSetRenderTargets(None, None);
            self.swapchain.ResizeBuffers(0, width, height, DXGI_FORMAT_UNKNOWN, DXGI_SWAP_CHAIN_FLAG(0))?;
        }
        self.width = width;
        self.height = height;
        self.make_rtv(gpu)
    }

    pub fn render(&mut self, gpu: &Gpu, kind: WidgetKind, quality: Quality, p: &Params) {
        if self.feedback.as_ref().is_some_and(|f| f.kind != kind || f.quality != quality) {
            self.feedback = None;
        }
        if kind.two_pass() && self.feedback.is_none() {
            match Feedback::new(gpu, kind, quality, self.width, self.height) {
                Ok(fb) => self.feedback = Some(fb),
                Err(e) => eprintln!("feedback buffers: {e}"),
            }
        }
        if let Some(rtv) = &self.rtv {
            gpu.draw(rtv, kind, p, self.feedback.as_mut());
        }
    }

    pub fn present(&self) -> Result<()> {
        unsafe { self.swapchain.Present(0, DXGI_PRESENT(0)).ok() }
    }
}

pub struct Preview {
    tex: ID3D11Texture2D,
    rtv: ID3D11RenderTargetView,
    staging: ID3D11Texture2D,
    feedback: Option<Feedback>,
    pub width: u32,
    pub height: u32,
}

impl Preview {
    pub fn new(gpu: &Gpu, width: u32, height: u32) -> Result<Self> {
        unsafe {
            let mut desc = D3D11_TEXTURE2D_DESC {
                Width: width,
                Height: height,
                MipLevels: 1,
                ArraySize: 1,
                Format: FORMAT,
                SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
                Usage: D3D11_USAGE_DEFAULT,
                BindFlags: D3D11_BIND_RENDER_TARGET.0 as u32,
                CPUAccessFlags: 0,
                MiscFlags: 0,
            };
            let mut tex = None;
            gpu.device.CreateTexture2D(&desc, None, Some(&mut tex))?;
            let tex = tex.unwrap();
            let mut rtv = None;
            gpu.device.CreateRenderTargetView(&tex, None, Some(&mut rtv))?;
            desc.Usage = D3D11_USAGE_STAGING;
            desc.BindFlags = 0;
            desc.CPUAccessFlags = D3D11_CPU_ACCESS_READ.0 as u32;
            let mut staging = None;
            gpu.device.CreateTexture2D(&desc, None, Some(&mut staging))?;
            Ok(Self { tex, rtv: rtv.unwrap(), staging: staging.unwrap(), feedback: None, width, height })
        }
    }

    pub fn render(&mut self, gpu: &Gpu, kind: WidgetKind, quality: Quality, p: &Params) {
        if self.feedback.as_ref().is_some_and(|f| f.kind != kind || f.quality != quality) {
            self.feedback = None;
        }
        if kind.two_pass() && self.feedback.is_none() {
            self.feedback = Feedback::new(gpu, kind, quality, self.width, self.height).ok();
        }
        unsafe {
            gpu.ctx.ClearRenderTargetView(&self.rtv, &[0.0; 4]);
        }
        gpu.draw(&self.rtv, kind, p, self.feedback.as_mut());
    }

    // Composites the premultiplied frame over `bg` and writes tightly packed RGB8.
    pub fn read_rgb(&self, gpu: &Gpu, bg: [u8; 3], out: &mut Vec<u8>) -> Result<()> {
        unsafe {
            gpu.ctx.CopyResource(&self.staging, &self.tex);
            let mut m = D3D11_MAPPED_SUBRESOURCE::default();
            gpu.ctx.Map(&self.staging, 0, D3D11_MAP_READ, 0, Some(&mut m))?;
            out.resize((self.width * self.height * 3) as usize, 0);
            for y in 0..self.height as usize {
                let row = std::slice::from_raw_parts(
                    (m.pData as *const u8).add(y * m.RowPitch as usize),
                    self.width as usize * 4,
                );
                for x in 0..self.width as usize {
                    let px = &row[x * 4..x * 4 + 4];
                    let inv = 255 - px[3] as u32;
                    let o = &mut out[(y * self.width as usize + x) * 3..][..3];
                    for (c, (src, b)) in o.iter_mut().zip([px[2], px[1], px[0]].iter().zip(bg.iter())) {
                        *c = (*src as u32 + (*b as u32 * inv + 127) / 255).min(255) as u8;
                    }
                }
            }
            gpu.ctx.Unmap(&self.staging, 0);
        }
        Ok(())
    }
}
