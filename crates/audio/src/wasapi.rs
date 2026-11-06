use crate::{push, Shared, Source};
use std::sync::mpsc;
use std::thread::Thread;
use std::time::{Duration, Instant};
use windows::core::{Result as WinResult, HRESULT, PWSTR};
use windows::Win32::Foundation::{CloseHandle, HANDLE, WAIT_OBJECT_0};
use windows::Win32::Media::Audio::*;
use windows::Win32::System::Com::*;
use windows::Win32::System::Threading::{CreateEventW, WaitForSingleObject};

const AUDCLNT_E_DEVICE_INVALIDATED: HRESULT = HRESULT(0x8889_0004_u32 as i32);
const WAVE_FORMAT_PCM: u16 = 1;
const WAVE_FORMAT_IEEE_FLOAT: u16 = 3;
const WAVE_FORMAT_EXTENSIBLE: u16 = 0xFFFE;
const WAIT_MS: u32 = 100;

#[derive(Clone, Copy, Debug, PartialEq)]
enum SampleFmt {
    F32,
    I16,
    I24,
    I32,
}

struct Open {
    client: IAudioClient,
    capture: IAudioCaptureClient,
    event: HANDLE,
    rate: u32,
    channels: usize,
    block_align: usize,
    fmt: SampleFmt,
    loopback: bool,
    track: Option<(IMMDeviceEnumerator, EDataFlow, String)>,
}

impl Drop for Open {
    fn drop(&mut self) {
        unsafe {
            let _ = self.client.Stop();
            let _ = CloseHandle(self.event);
        }
    }
}

enum Exit {
    Stop,
    Reopen,
}

pub(crate) fn run(
    source: Source,
    mut prod: rtrb::Producer<f32>,
    sh: &Shared,
    wake: &Thread,
    tx: mpsc::Sender<Result<(), String>>,
) {
    unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
    }
    let mut tx = Some(tx);
    let mut scratch: Vec<f32> = Vec::with_capacity(96_000);
    while !sh.stopping() {
        match unsafe { open(&source) } {
            Ok(mut s) => {
                sh.opened(s.rate);
                if let Some(t) = tx.take() {
                    let _ = t.send(Ok(()));
                }
                match unsafe { pump(&mut s, &mut prod, sh, wake, &mut scratch) } {
                    Ok(Exit::Stop) => break,
                    Ok(Exit::Reopen) => {}
                    Err(e) if e.code() == AUDCLNT_E_DEVICE_INVALIDATED => {}
                    Err(e) => eprintln!("audio: {e}"),
                }
            }
            Err(e) => {
                if let Some(t) = tx.take() {
                    let _ = t.send(Err(format!("opening {source:?}: {e}")));
                    break;
                }
            }
        }
        let until = Instant::now() + Duration::from_millis(500);
        while Instant::now() < until && !sh.stopping() {
            std::thread::sleep(Duration::from_millis(50));
        }
    }
    unsafe { CoUninitialize() };
}

unsafe fn default_id(en: &IMMDeviceEnumerator, flow: EDataFlow) -> WinResult<String> {
    let dev = en.GetDefaultAudioEndpoint(flow, eConsole)?;
    let p: PWSTR = dev.GetId()?;
    let s = p.to_string().unwrap_or_default();
    CoTaskMemFree(Some(p.0 as *const _));
    Ok(s)
}

unsafe fn open(source: &Source) -> WinResult<Open> {
    let (flow, loopback) = match source {
        Source::Loopback => (eRender, true),
        Source::Mic => (eCapture, false),
        Source::Process(_) => {
            return Err(windows::core::Error::new(HRESULT(-1), "process loopback: not implemented yet"))
        }
        Source::Samples { .. } => unreachable!(),
    };
    let en: IMMDeviceEnumerator = CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)?;
    let dev = en.GetDefaultAudioEndpoint(flow, eConsole)?;
    let id = dev.GetId()?;
    let id_s = id.to_string().unwrap_or_default();
    CoTaskMemFree(Some(id.0 as *const _));
    let client: IAudioClient = dev.Activate(CLSCTX_ALL, None)?;

    let mix = client.GetMixFormat()?;
    let wf = std::ptr::read_unaligned(mix);
    let tag = if wf.wFormatTag == WAVE_FORMAT_EXTENSIBLE && wf.cbSize >= 22 {
        // SubFormat GUID's first u32 is the plain format tag.
        std::ptr::read_unaligned((mix as *const u8).add(24) as *const u32) as u16
    } else {
        wf.wFormatTag
    };
    let fmt = match (tag, wf.wBitsPerSample) {
        (WAVE_FORMAT_IEEE_FLOAT, 32) => Some(SampleFmt::F32),
        (WAVE_FORMAT_PCM, 16) => Some(SampleFmt::I16),
        (WAVE_FORMAT_PCM, 24) => Some(SampleFmt::I24),
        (WAVE_FORMAT_PCM, 32) => Some(SampleFmt::I32),
        _ => None,
    };
    let mut flags = AUDCLNT_STREAMFLAGS_EVENTCALLBACK;
    if loopback {
        flags |= AUDCLNT_STREAMFLAGS_LOOPBACK;
    }
    let init = client.Initialize(AUDCLNT_SHAREMODE_SHARED, flags, 1_000_000, 0, mix, None);
    let rate = wf.nSamplesPerSec;
    let channels = wf.nChannels.max(1) as usize;
    let block_align = wf.nBlockAlign as usize;
    CoTaskMemFree(Some(mix as *const _));
    init?;
    let Some(fmt) = fmt else {
        return Err(windows::core::Error::new(HRESULT(-1), "unsupported mix format"));
    };

    let event = CreateEventW(None, false, false, None)?;
    client.SetEventHandle(event)?;
    let capture: IAudioCaptureClient = client.GetService()?;
    client.Start()?;
    Ok(Open { client, capture, event, rate, channels, block_align, fmt, loopback, track: Some((en, flow, id_s)) })
}

unsafe fn pump(
    s: &mut Open,
    prod: &mut rtrb::Producer<f32>,
    sh: &Shared,
    wake: &Thread,
    scratch: &mut Vec<f32>,
) -> WinResult<Exit> {
    let mut last_check = Instant::now();
    let mut last_feed = Instant::now();
    loop {
        if sh.stopping() {
            return Ok(Exit::Stop);
        }
        let signaled = WaitForSingleObject(s.event, WAIT_MS) == WAIT_OBJECT_0;
        let mut got = false;
        loop {
            let n = s.capture.GetNextPacketSize()?;
            if n == 0 {
                break;
            }
            let mut data: *mut u8 = std::ptr::null_mut();
            let mut frames = 0u32;
            let mut flags = 0u32;
            s.capture.GetBuffer(&mut data, &mut frames, &mut flags, None, None)?;
            let frames_us = frames as usize;
            scratch.clear();
            if flags & (AUDCLNT_BUFFERFLAGS_SILENT.0 as u32) != 0 || data.is_null() {
                scratch.resize(frames_us.min(scratch.capacity()), 0.0);
            } else {
                let bytes = std::slice::from_raw_parts(data, frames_us * s.block_align);
                downmix(bytes, s.fmt, s.channels, s.block_align, scratch);
            }
            s.capture.ReleaseBuffer(frames)?;
            push(prod, scratch);
            got = true;
            last_feed = Instant::now();
        }
        if !signaled && !got && s.loopback {
            // Loopback delivers no packets while the device is idle, so synthesize the elapsed silence.
            let now = Instant::now();
            let n = (now.duration_since(last_feed).as_secs_f64() * s.rate as f64) as usize;
            last_feed = now;
            scratch.clear();
            scratch.resize(n.min(scratch.capacity()), 0.0);
            push(prod, scratch);
            got = true;
        }
        if got {
            wake.unpark();
        }
        if last_check.elapsed() >= Duration::from_secs(1) {
            last_check = Instant::now();
            if let Some((en, flow, id)) = &s.track {
                if let Ok(now) = default_id(en, *flow) {
                    if &now != id {
                        return Ok(Exit::Reopen);
                    }
                }
            }
        }
    }
}

fn downmix(bytes: &[u8], fmt: SampleFmt, ch: usize, align: usize, out: &mut Vec<f32>) {
    let inv = 1.0 / ch as f32;
    let width = match fmt {
        SampleFmt::F32 | SampleFmt::I32 => 4,
        SampleFmt::I16 => 2,
        SampleFmt::I24 => 3,
    };
    for frame in bytes.chunks_exact(align) {
        let mut acc = 0.0f32;
        for c in 0..ch {
            let b = &frame[c * width..c * width + width];
            acc += match fmt {
                SampleFmt::F32 => f32::from_le_bytes([b[0], b[1], b[2], b[3]]),
                SampleFmt::I16 => i16::from_le_bytes([b[0], b[1]]) as f32 / 32768.0,
                SampleFmt::I24 => (i32::from_le_bytes([0, b[0], b[1], b[2]]) >> 8) as f32 / 8_388_608.0,
                SampleFmt::I32 => i32::from_le_bytes([b[0], b[1], b[2], b[3]]) as f32 / 2_147_483_648.0,
            };
        }
        if out.len() < out.capacity() {
            out.push(acc * inv);
        }
    }
}
