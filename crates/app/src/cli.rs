#[derive(Debug, Clone, PartialEq)]
pub enum Source {
    Loopback,
    Mic,
    Wav(String),
    Process(u32),
}

#[derive(Debug, Clone, Default)]
pub struct Args {
    pub source: Option<Source>,
    pub exit_after: Option<f64>,
    pub screenshot: Option<String>,
    pub frames: u32,
    pub headless_analyze: Option<String>,
    pub gen_test_wavs: Option<String>,
    pub console: bool,
    pub edit: bool,
    pub config: Option<String>,
    pub warp: bool,
    pub background: bool,
    pub beat_eval: bool,
    pub eval_dir: Option<String>,
    pub kind: Option<String>,
    pub style: Option<String>,
}

pub fn parse(mut it: impl Iterator<Item = String>) -> Result<Args, String> {
    let mut a = Args { frames: 30, ..Default::default() };
    let val = |it: &mut dyn Iterator<Item = String>, flag: &str| it.next().ok_or(format!("{flag} needs a value"));
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--source" => {
                let v = val(&mut it, &arg)?;
                a.source = Some(match v.as_str() {
                    "loopback" => Source::Loopback,
                    "mic" => Source::Mic,
                    _ if v.starts_with("wav:") => Source::Wav(v[4..].to_string()),
                    _ if v.starts_with("pid:") => Source::Process(v[4..].parse().map_err(|_| "bad pid")?),
                    _ => return Err(format!("unknown source {v}")),
                });
            }
            "--exit-after" => a.exit_after = Some(val(&mut it, &arg)?.parse().map_err(|_| "bad --exit-after")?),
            "--screenshot" => a.screenshot = Some(val(&mut it, &arg)?),
            "--frames" => a.frames = val(&mut it, &arg)?.parse().map_err(|_| "bad --frames")?,
            "--headless-analyze" => a.headless_analyze = Some(val(&mut it, &arg)?),
            "--gen-test-wavs" => a.gen_test_wavs = Some(val(&mut it, &arg)?),
            "--console" => a.console = true,
            "--edit" => a.edit = true,
            "--warp" => a.warp = true,
            "--background" => a.background = true,
            "--beat-eval" => a.beat_eval = true,
            "--eval-dir" => a.eval_dir = Some(val(&mut it, &arg)?),
            "--config" => a.config = Some(val(&mut it, &arg)?),
            "--kind" => a.kind = Some(val(&mut it, &arg)?),
            "--style" => a.style = Some(val(&mut it, &arg)?),
            "-h" | "--help" => return Err(HELP.into()),
            _ => return Err(format!("unknown argument {arg}\n{HELP}")),
        }
    }
    Ok(a)
}

pub const HELP: &str = "usage: vizz [--source loopback|mic|wav:<path>|pid:<n>] [--exit-after <s>]
                  [--screenshot <out.png> --frames <n> [--kind flux|warp|pop|geo|ring|sketch]
                   [--style neon|fluent|synthwave|liquid]] [--console] [--edit] [--warp] [--background] [--config <file>]
       vizz --headless-analyze <file.wav>
       vizz --gen-test-wavs <dir>
       vizz --beat-eval | --eval-dir <folder of .wav>";
