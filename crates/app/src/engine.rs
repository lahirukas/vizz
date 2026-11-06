use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender, TryRecvError};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;
use vizz_dsp::{Analyzer, SnapshotBus};

pub struct Engine {
    pub bus: Arc<SnapshotBus>,
    status: Arc<Mutex<String>>,
    switch: Sender<vizz_audio::Source>,
    stop: Arc<AtomicBool>,
    done: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl Engine {
    pub fn start(source: vizz_audio::Source) -> Result<Self, String> {
        let bus = Arc::new(SnapshotBus::new());
        let stop = Arc::new(AtomicBool::new(false));
        let done = Arc::new(AtomicBool::new(false));
        let status = Arc::new(Mutex::new(String::new()));
        let (switch, sources) = channel();
        let (ready_tx, ready_rx) = channel();
        let (b, s, d, st) = (bus.clone(), stop.clone(), done.clone(), status.clone());
        let thread = std::thread::Builder::new()
            .name("dsp".into())
            .stack_size(512 * 1024)
            .spawn(move || {
                let first = vizz_audio::start(source, std::thread::current());
                let mut stream = match first {
                    Ok(stream) => {
                        let _ = ready_tx.send(Ok(()));
                        Some(stream)
                    }
                    Err(e) => {
                        let _ = ready_tx.send(Err(e));
                        return;
                    }
                };
                while let Some(cur) = stream.take() {
                    let next = run(cur, &b, &s, &sources);
                    let Some(src) = next else { break };
                    match vizz_audio::start(src.clone(), std::thread::current()) {
                        Ok(new) => {
                            *st.lock().unwrap() = String::new();
                            stream = Some(new);
                        }
                        Err(e) => {
                            *st.lock().unwrap() = format!("{src:?}: {e}");
                            stream = wait_for_source(&sources, &s);
                        }
                    }
                }
                d.store(true, Ordering::Release);
            })
            .map_err(|e| e.to_string())?;
        match ready_rx.recv() {
            Ok(Ok(())) => Ok(Self { bus, status, switch, stop, done, thread: Some(thread) }),
            Ok(Err(e)) => Err(e),
            Err(_) => Err("dsp thread died".into()),
        }
    }

    pub fn set_source(&self, source: vizz_audio::Source) {
        let _ = self.switch.send(source);
        if let Some(t) = &self.thread {
            t.thread().unpark();
        }
    }

    pub fn status(&self) -> String {
        self.status.lock().unwrap().clone()
    }

    pub fn finished(&self) -> bool {
        self.done.load(Ordering::Acquire)
    }
}

impl Drop for Engine {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(t) = self.thread.take() {
            t.thread().unpark();
            let _ = t.join();
        }
    }
}

fn wait_for_source(sources: &Receiver<vizz_audio::Source>, stop: &AtomicBool) -> Option<vizz_audio::Stream> {
    while !stop.load(Ordering::Acquire) {
        match sources.try_recv() {
            Ok(src) => {
                if let Ok(s) = vizz_audio::start(src, std::thread::current()) {
                    return Some(s);
                }
            }
            Err(TryRecvError::Disconnected) => return None,
            Err(TryRecvError::Empty) => std::thread::park_timeout(Duration::from_millis(200)),
        }
    }
    None
}

fn run(
    mut stream: vizz_audio::Stream,
    bus: &SnapshotBus,
    stop: &AtomicBool,
    sources: &Receiver<vizz_audio::Source>,
) -> Option<vizz_audio::Source> {
    let mut generation = stream.generation();
    let mut an = Analyzer::new(stream.sample_rate());
    let mut nop = |_t: f64| {};
    while !stop.load(Ordering::Acquire) {
        if let Ok(src) = sources.try_recv() {
            return Some(src);
        }
        let g = stream.generation();
        if g != generation {
            generation = g;
            an = Analyzer::new(stream.sample_rate());
        }
        let n = stream.consumer.slots();
        if n == 0 {
            if stream.finished() {
                break;
            }
            std::thread::park_timeout(Duration::from_millis(100));
            continue;
        }
        if let Ok(chunk) = stream.consumer.read_chunk(n) {
            let (a, b) = chunk.as_slices();
            an.process(a, &mut nop);
            an.process(b, &mut nop);
            chunk.commit_all();
        }
        bus.publish(an.snapshot());
    }
    None
}
