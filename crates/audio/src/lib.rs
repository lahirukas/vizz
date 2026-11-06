use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{mpsc, Arc};
use std::thread::{JoinHandle, Thread};
use std::time::{Duration, Instant};

#[cfg(windows)]
mod wasapi;

#[derive(Clone, Debug)]
pub enum Source {
    Loopback,
    Mic,
    Process(u32),
    Samples { sample_rate: u32, data: Arc<[f32]>, looped: bool },
}

pub struct Shared {
    sample_rate: AtomicU32,
    generation: AtomicU32,
    stop: AtomicBool,
    finished: AtomicBool,
}

pub struct Stream {
    pub consumer: rtrb::Consumer<f32>,
    shared: Arc<Shared>,
    thread: Option<JoinHandle<()>>,
}

impl Stream {
    pub fn sample_rate(&self) -> u32 {
        self.shared.sample_rate.load(Ordering::Acquire)
    }
    pub fn generation(&self) -> u32 {
        self.shared.generation.load(Ordering::Acquire)
    }
    pub fn finished(&self) -> bool {
        self.shared.finished.load(Ordering::Acquire)
    }
}

impl Drop for Stream {
    fn drop(&mut self) {
        self.shared.stop.store(true, Ordering::Release);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

const RING_SECS: usize = 2;

pub fn start(source: Source, wake: Thread) -> Result<Stream, String> {
    let (prod, consumer) = rtrb::RingBuffer::new(192_000 * RING_SECS);
    let shared = Arc::new(Shared {
        sample_rate: AtomicU32::new(48000),
        generation: AtomicU32::new(0),
        stop: AtomicBool::new(false),
        finished: AtomicBool::new(false),
    });
    let (tx, rx) = mpsc::channel::<Result<(), String>>();
    let sh = shared.clone();
    let thread = std::thread::Builder::new()
        .name("audio-capture".into())
        .stack_size(256 * 1024)
        .spawn(move || match source {
            Source::Samples { sample_rate, data, looped } => {
                run_samples(sample_rate, &data, looped, prod, &sh, &wake, tx)
            }
            #[cfg(windows)]
            other => wasapi::run(other, prod, &sh, &wake, tx),
            #[cfg(not(windows))]
            _ => {
                let _ = tx.send(Err("WASAPI sources need Windows".into()));
            }
        })
        .map_err(|e| e.to_string())?;
    match rx.recv_timeout(Duration::from_secs(5)) {
        Ok(Ok(())) => Ok(Stream { consumer, shared, thread: Some(thread) }),
        Ok(Err(e)) => {
            let _ = thread.join();
            Err(e)
        }
        Err(_) => {
            shared.stop.store(true, Ordering::Release);
            Err("audio source did not start within 5 s".into())
        }
    }
}

pub(crate) fn push(prod: &mut rtrb::Producer<f32>, data: &[f32]) {
    let n = data.len().min(prod.slots());
    if n == 0 {
        return;
    }
    if let Ok(mut chunk) = prod.write_chunk(n) {
        let (a, b) = chunk.as_mut_slices();
        let (da, db) = data[..n].split_at(a.len());
        a.copy_from_slice(da);
        b.copy_from_slice(db);
        chunk.commit_all();
    }
}

impl Shared {
    pub(crate) fn opened(&self, rate: u32) {
        self.sample_rate.store(rate, Ordering::Release);
        self.generation.fetch_add(1, Ordering::AcqRel);
    }
    pub(crate) fn stopping(&self) -> bool {
        self.stop.load(Ordering::Acquire)
    }
}

fn run_samples(
    sr: u32,
    data: &[f32],
    looped: bool,
    mut prod: rtrb::Producer<f32>,
    sh: &Shared,
    wake: &Thread,
    tx: mpsc::Sender<Result<(), String>>,
) {
    sh.opened(sr);
    let _ = tx.send(Ok(()));
    let chunk = (sr / 100).max(1) as usize;
    let start = Instant::now();
    let mut pos = 0usize;
    let mut sent = 0u64;
    while !sh.stopping() {
        if pos >= data.len() {
            if looped && !data.is_empty() {
                pos = 0;
            } else {
                sh.finished.store(true, Ordering::Release);
                wake.unpark();
                break;
            }
        }
        let end = (pos + chunk).min(data.len());
        push(&mut prod, &data[pos..end]);
        sent += (end - pos) as u64;
        pos = end;
        wake.unpark();
        let due = start + Duration::from_secs_f64(sent as f64 / sr as f64);
        if let Some(d) = due.checked_duration_since(Instant::now()) {
            std::thread::sleep(d);
        }
    }
}
