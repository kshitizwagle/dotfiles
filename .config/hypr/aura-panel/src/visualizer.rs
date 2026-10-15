//! Media progress bar drawn as live spectrum bars (cava-style), purple up to the playhead.
//! Captures the default sink's monitor via `pw-cat` — never a microphone.
use gtk::prelude::*;
use gtk::glib;
use std::cell::{Cell, RefCell};
use std::f64::consts::PI;
use std::io::Read;
use std::os::unix::process::CommandExt;
use std::process::{Command, Stdio};
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const BANDS: usize = 32;
const RATE: f64 = 16000.0;
const CHUNK: usize = 512;

struct Shared {
    levels: Mutex<(Vec<f64>, Option<Instant>)>,
    paused: AtomicBool,
}

pub struct Visualizer {
    pub area: gtk::DrawingArea,
    pub progress: Cell<f64>,
    pub playing: Cell<bool>,
    bars: RefCell<Vec<f64>>,
    shared: Arc<Shared>,
}

/// Band magnitudes (0..1, dB-scaled) via Goertzel at log-spaced 60 Hz – 6 kHz centres.
fn spectrum(samples: &[i16], window: &[f64], coeffs: &[f64]) -> Vec<f64> {
    let n = samples.len() as f64;
    let x: Vec<f64> = samples.iter().zip(window).map(|(s, w)| *s as f64 * w).collect();
    coeffs
        .iter()
        .map(|c| {
            let (mut s1, mut s2) = (0.0, 0.0);
            for v in &x {
                let s0 = v + c * s1 - s2;
                s2 = s1;
                s1 = s0;
            }
            let mag = (s1 * s1 + s2 * s2 - c * s1 * s2).max(0.0).sqrt() / (n * 32768.0 / 4.0);
            let db = 20.0 * (mag + 1e-9).log10();
            ((db + 62.0) / 50.0).clamp(0.0, 1.0)
        })
        .collect()
}

fn capture(shared: Arc<Shared>) {
    let window: Vec<f64> = (0..CHUNK).map(|i| 0.5 - 0.5 * (2.0 * PI * i as f64 / (CHUNK as f64 - 1.0)).cos()).collect();
    let coeffs: Vec<f64> = (0..BANDS)
        .map(|i| 60.0 * (6000.0f64 / 60.0).powf(i as f64 / (BANDS as f64 - 1.0)))
        .map(|f| 2.0 * (2.0 * PI * f / RATE).cos())
        .collect();
    loop {
        if shared.paused.load(Ordering::Relaxed) {
            std::thread::sleep(Duration::from_millis(100));
            continue;
        }
        let child = unsafe {
            Command::new("pw-cat")
                .args(["--record", "--raw", "--format=s16", "--rate=16000", "--channels=1",
                       "--properties=stream.capture.sink=true", "-"])
                .stdout(Stdio::piped())
                .stderr(Stdio::null())
                // die with the panel instead of lingering as an orphan
                .pre_exec(|| {
                    libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGTERM);
                    Ok(())
                })
                .spawn()
        };
        let Ok(mut child) = child else {
            std::thread::sleep(Duration::from_secs(1));
            continue;
        };
        let mut stdout = child.stdout.take().unwrap();
        let mut buf = vec![0u8; CHUNK * 2];
        while !shared.paused.load(Ordering::Relaxed) {
            if stdout.read_exact(&mut buf).is_err() {
                break;
            }
            let samples: Vec<i16> = buf.chunks_exact(2).map(|b| i16::from_le_bytes([b[0], b[1]])).collect();
            let lv = spectrum(&samples, &window, &coeffs);
            *shared.levels.lock().unwrap() = (lv, Some(Instant::now()));
        }
        let _ = child.kill();
        let _ = child.wait();
        std::thread::sleep(Duration::from_millis(200));
    }
}

impl Visualizer {
    pub fn new() -> Rc<Self> {
        let area = gtk::DrawingArea::new();
        area.set_content_height(28);
        area.set_hexpand(true);
        area.set_tooltip_text(Some("Live system audio"));
        let shared = Arc::new(Shared { levels: Mutex::new((vec![0.0; BANDS], None)), paused: AtomicBool::new(false) });
        let v = Rc::new(Self {
            area,
            progress: Cell::new(0.0),
            playing: Cell::new(false),
            bars: RefCell::new(vec![0.0; BANDS]),
            shared: shared.clone(),
        });
        std::thread::spawn(move || capture(shared));

        let weak = Rc::downgrade(&v);
        v.area.set_draw_func(move |_, cr, w, h| {
            if let Some(v) = weak.upgrade() {
                v.draw(cr, w as f64, h as f64);
            }
        });
        let weak = Rc::downgrade(&v);
        glib::timeout_add_local(Duration::from_millis(50), move || {
            let Some(v) = weak.upgrade() else { return glib::ControlFlow::Break };
            if !v.area.is_mapped() {
                return glib::ControlFlow::Continue;
            }
            let (levels, at) = v.shared.levels.lock().unwrap().clone();
            let live = v.playing.get() && at.is_some_and(|t| t.elapsed() < Duration::from_millis(500));
            let mut bars = v.bars.borrow_mut();
            for (i, b) in bars.iter_mut().enumerate() {
                let t = if live { levels[i] } else { 0.0 };
                // quick attack, gentle release
                *b += (t - *b) * if t > *b { 0.55 } else { 0.14 };
            }
            drop(bars);
            v.area.queue_draw();
            glib::ControlFlow::Continue
        });
        v
    }

    fn draw(&self, cr: &gtk::cairo::Context, w: f64, h: f64) {
        let bars = self.bars.borrow();
        let n = bars.len() as f64;
        let gap = 3.0;
        let bw = ((w - gap * (n - 1.0)) / n).max(2.0);
        let played = w * self.progress.get();
        let mid = h / 2.0;
        for (i, level) in bars.iter().enumerate() {
            let x = i as f64 * (bw + gap);
            let half = (level * (h / 2.0 - 1.0)).max(1.5);
            if x + bw / 2.0 <= played {
                cr.set_source_rgba(0.635, 0.467, 1.0, 0.55 + 0.45 * level); // Aura purple, played
            } else {
                cr.set_source_rgba(0.42, 0.40, 0.52, 0.45); // muted, still to play
            }
            let r = (bw / 2.0).min(half);
            let (y0, y1) = (mid - half, mid + half);
            cr.new_sub_path();
            cr.arc(x + bw - r, y0 + r, r, -PI / 2.0, 0.0);
            cr.arc(x + bw - r, y1 - r, r, 0.0, PI / 2.0);
            cr.arc(x + r, y1 - r, r, PI / 2.0, PI);
            cr.arc(x + r, y0 + r, r, PI, 3.0 * PI / 2.0);
            cr.close_path();
            let _ = cr.fill();
        }
    }
}
