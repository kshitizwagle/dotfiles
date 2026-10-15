//! aura-panel: Hyprland home-screen dashboard (desktop cards on the bottom layer) plus a
//! controls sidebar (Super+D). GTK4 + gtk4-layer-shell, Aura palette.
//!
//! Super+D sends SIGUSR1 (see toggle.sh). The handler is installed before any UI is built, and
//! `--raised` starts the panel already showing, so a toggle can never be lost during startup.
mod calendar;
mod connections;
mod hw;
mod util;
mod visualizer;

use connections::Connections;
use gtk::prelude::*;
use gtk::{gdk, gio, glib};
use gtk4_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};
use serde_json::{json, Value};
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use util::*;
use visualizer::Visualizer;

const ESSENTIAL_STACKS: [&str; 4] = ["infra", "arr", "media", "immich"];
const STATIC_EFFECTS: [&str; 3] = ["solid", "static", "off"];
const SPEEDS: [&str; 5] = ["still", "calm", "gentle", "lively", "fast"];
const MPRIS: &str = "org.mpris.MediaPlayer2";

/// One line in the System card.
#[derive(Clone)]
enum SysRow {
    Cpu,
    Ram,
    /// any vendor, discrete or integrated (see hw.rs)
    Gpu(hw::Gpu),
    /// block device name, model, rotational (HDD), mount point if mounted
    Disk { dev: String, model: String, hdd: bool, mount: Option<String> },
}

trait ThenTooltip {
    fn then_tooltip(self, w: &gtk::LevelBar, tip: &str) -> Self;
}
impl ThenTooltip for (f64, String, String) {
    fn then_tooltip(self, w: &gtk::LevelBar, tip: &str) -> Self {
        w.set_tooltip_text(Some(tip));
        self
    }
}

/// Average core clock as a 0..1 fraction between the CPU's minimum and maximum frequency.
fn cpu_clock_frac() -> impl Fn() -> f64 {
    let khz = |f: &str| read_u64(&format!("/sys/devices/system/cpu/cpu0/cpufreq/{f}")).unwrap_or(0) as f64;
    let (lo, hi) = (khz("cpuinfo_min_freq"), khz("cpuinfo_max_freq").max(1.0));
    move || {
        let freqs: Vec<f64> = std::fs::read_dir("/sys/devices/system/cpu").into_iter().flatten().flatten()
            .filter_map(|e| read_u64(&format!("{}/cpufreq/scaling_cur_freq", e.path().display())))
            .map(|k| k as f64).collect();
        let avg = if freqs.is_empty() { lo } else { freqs.iter().sum::<f64>() / freqs.len() as f64 };
        ((avg - lo) / (hi - lo).max(1.0)).clamp(0.0, 1.0)
    }
}

/// Disk activity: read+write throughput from /sys/block/<dev>/stat, scaled against the busiest
/// sample of the last ~20 s (floor 1 MB/s, so an idle disk doesn't magnify noise into spikes).
fn disk_activity(dev: String) -> impl Fn() -> f64 {
    let sectors = move || -> f64 {
        std::fs::read_to_string(format!("/sys/block/{dev}/stat")).ok()
            .map(|t| { let f: Vec<f64> = t.split_whitespace().filter_map(|x| x.parse().ok()).collect(); f.get(2).copied().unwrap_or(0.0) + f.get(6).copied().unwrap_or(0.0) })
            .unwrap_or(0.0)
    };
    let last = Cell::new(sectors());
    let recent: RefCell<std::collections::VecDeque<f64>> = RefCell::new(std::collections::VecDeque::new());
    move || {
        let now = sectors();
        let bytes_per_s = (now - last.replace(now)).max(0.0) * 512.0 / 0.5;
        let mut r = recent.borrow_mut();
        r.push_back(bytes_per_s);
        if r.len() > 40 {
            r.pop_front();
        }
        let peak = r.iter().cloned().fold(1.0e6, f64::max);
        (bytes_per_s / peak).clamp(0.0, 1.0)
    }
}

/// Memory in use (MemTotal − MemAvailable) as a 0..1 fraction.
fn mem_frac() -> f64 {
    let mi = std::fs::read_to_string("/proc/meminfo").unwrap_or_default();
    let kb = |k: &str| mi.lines().find(|l| l.starts_with(k)).and_then(|l| l.split_whitespace().nth(1)).and_then(|v| v.parse::<f64>().ok()).unwrap_or(0.0);
    let (t, a) = (kb("MemTotal:"), kb("MemAvailable:"));
    if t > 0.0 { ((t - a) / t).clamp(0.0, 1.0) } else { 0.0 }
}

/// ECG-style trace of `sample` (0..1): 2 samples/s over 20 s, faint monitor grid, soft glow and a
/// bright "pen" dot at the newest sample. Pauses while unmapped.
fn ecg(area: &gtk::DrawingArea, (r, g, b): (f64, f64, f64), sample: Box<dyn Fn() -> f64>) {
    const N: usize = 40;
    let first = sample();
    let samples: Rc<RefCell<std::collections::VecDeque<f64>>> = Rc::new(RefCell::new(std::collections::VecDeque::from(vec![first; N])));
    let hist = samples.clone();
    area.set_draw_func(move |_, cr, w, h| {
        let (w, h) = (w as f64, h as f64);
        let rad = 6.0;
        cr.new_sub_path();
        cr.arc(w - rad, rad, rad, -std::f64::consts::FRAC_PI_2, 0.0);
        cr.arc(w - rad, h - rad, rad, 0.0, std::f64::consts::FRAC_PI_2);
        cr.arc(rad, h - rad, rad, std::f64::consts::FRAC_PI_2, std::f64::consts::PI);
        cr.arc(rad, rad, rad, std::f64::consts::PI, 1.5 * std::f64::consts::PI);
        cr.close_path();
        cr.clip();
        let s = hist.borrow();
        let (top, bottom) = (h * 0.12, h * 0.88); // a little headroom so peaks read as spikes
        let pt = |i: usize, v: f64| (w * i as f64 / (N - 1) as f64, bottom - (bottom - top) * v);
        let path = |cr: &gtk::cairo::Context| {
            for (i, v) in s.iter().enumerate() {
                let (x, y) = pt(i, *v);
                if i == 0 { cr.move_to(x, y) } else { cr.line_to(x, y) }
            }
        };
        // soft area under the trace
        path(cr);
        cr.line_to(w, h);
        cr.line_to(0.0, h);
        cr.close_path();
        cr.set_source_rgba(r, g, b, 0.10);
        let _ = cr.fill();
        // glow, then the line itself
        cr.set_line_join(gtk::cairo::LineJoin::Round);
        cr.set_line_cap(gtk::cairo::LineCap::Round);
        path(cr);
        cr.set_source_rgba(r, g, b, 0.18);
        cr.set_line_width(5.0);
        let _ = cr.stroke();
        path(cr);
        cr.set_source_rgba(r, g, b, 0.85);
        cr.set_line_width(1.6);
        let _ = cr.stroke();
        // pen dot at the newest sample
        let (x, y) = pt(N - 1, *s.back().unwrap_or(&0.0));
        cr.arc(x - 2.0, y, 3.0, 0.0, 2.0 * std::f64::consts::PI);
        cr.set_source_rgba(r, g, b, 1.0);
        let _ = cr.fill();
    });
    let weak = area.downgrade();
    glib::timeout_add_local(Duration::from_millis(500), move || {
        let Some(area) = weak.upgrade() else { return glib::ControlFlow::Break };
        if !area.is_mapped() {
            return glib::ControlFlow::Continue;
        }
        let v = sample();
        let mut s = samples.borrow_mut();
        s.pop_front();
        s.push_back(v);
        drop(s);
        area.queue_draw();
        glib::ControlFlow::Continue
    });
}

fn read_u64(path: &str) -> Option<u64> {
    std::fs::read_to_string(path).ok()?.trim().parse().ok()
}

/// CPU, memory, every GPU (discrete first) and every mounted physical disk, with where each is mounted.
fn discover_rows() -> Vec<(SysRow, String, &'static str, &'static str)> {
    let mut rows = vec![(SysRow::Cpu, "CPU".to_string(), "󰻠", "c-red"), (SysRow::Ram, "Memory".to_string(), "󰍛", "c-orange")];
    // only the GPUs that exist: dGPU and/or iGPU, any vendor, any count
    for g in hw::discover("/sys") {
        let title = g.title.clone();
        rows.push((SysRow::Gpu(g), title, "󰢮", "c-purple"));
    }
    // mount points of whole disks (first mounted partition, skipping the EFI partition)
    let mounts = std::fs::read_to_string("/proc/mounts").unwrap_or_default();
    let mut disks: Vec<String> = std::fs::read_dir("/sys/block").into_iter().flatten().flatten()
        .map(|e| e.file_name().to_string_lossy().to_string())
        .filter(|n| n.starts_with("nvme") || n.starts_with("sd"))
        .collect();
    disks.sort();
    for d in disks {
        let mount = mounts.lines().filter_map(|l| {
            let mut f = l.split_whitespace();
            let (src, mp) = (f.next()?, f.next()?);
            (src.starts_with(&format!("/dev/{d}")) && mp != "/boot/efi").then(|| mp.to_string())
        }).next();
        let model = std::fs::read_to_string(format!("/sys/block/{d}/device/model")).unwrap_or_default().trim().to_string();
        let hdd = read_u64(&format!("/sys/block/{d}/queue/rotational")) == Some(1);
        let Some(mp) = mount.clone() else { continue }; // only mounted volumes
        let title = mp;
        rows.push((SysRow::Disk { dev: d, model, hdd, mount }, title, "󰋊", if hdd { "c-purple" } else { "c-pink" }));
    }
    rows
}

#[derive(Default, Clone)]
struct Media {
    name: String,
    status: String,
    title: String,
    artist: String,
    pos: i64,
    length: i64,
    art: String,
    art_path: Option<String>,
}

struct Panel {
    desk: gtk::Window,
    side: gtk::Window,
    raised: Cell<bool>,
    columns: Vec<gtk::Box>,
    // system
    sys_rows: RefCell<Vec<(SysRow, gtk::LevelBar, gtk::Label, gtk::Label, gtk::Label)>>,
    cpu_prev: Cell<(u64, u64)>,
    // docker
    docker_summary: gtk::Label,
    docker_metrics: RefCell<Vec<(gtk::Label, gtk::Label)>>,
    docker_list: gtk::FlowBox,
    // media
    viz: Rc<Visualizer>,
    media_title: gtk::Label,
    media_artist: gtk::Label,
    media_player: gtk::Label,
    media_art: gtk::Image,
    media_play: gtk::Button,
    media_bus: RefCell<Option<String>>,
    media_art_url: RefCell<String>,
    // lights
    light_drop: gtk::DropDown,
    light_ids: RefCell<Vec<String>>,
    scene_catalog: RefCell<Value>,
    lights_state: gtk::Label,
    light_power: gtk::Button,
    lights_on: Cell<bool>,
    light_bright: gtk::Scale,
    light_bright_lbl: gtk::Label,
    light_speed: gtk::Scale,
    light_speed_lbl: gtk::Label,
    light_speed_row: gtk::Box,
    updating_lights: Cell<bool>,
    light_debounce: RefCell<Option<glib::SourceId>>,
    // sidebar
    confirm_bar: gtk::Revealer,
    confirm_text: gtk::Label,
    confirm_go: gtk::Button,
    confirm_cmd: RefCell<String>,
    confirm_timeout: RefCell<Option<glib::SourceId>>,
    dnd: gtk::Button,
    notif_list: gtk::Box,
    notifications: RefCell<Vec<Value>>,
    notif_fresh: Cell<bool>,
    monitor_drop: gtk::DropDown,
    monitor_names: RefCell<Vec<String>>,
    connections: RefCell<Option<Rc<Connections>>>,
    catchers: RefCell<Vec<gtk::Window>>,
    cal_win: gtk::Window,
    cal: Rc<calendar::Calendar>,
}

fn now() -> f64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs_f64()).unwrap_or(0.0)
}

fn monitor_state_file() -> String {
    format!("{}/.config/hypr/dashboard-monitor", home())
}

fn notif_log() -> String {
    format!("{}/.cache/dashboard-notifications.json", home())
}

/// Make a widget tree mouse-only: no keyboard focus anywhere (Tab/arrow navigation off).
fn unfocusable(w: &gtk::Widget) {
    w.set_focusable(false);
    w.set_can_focus(false);
    let mut child = w.first_child();
    while let Some(c) = child {
        unfocusable(&c);
        child = c.next_sibling();
    }
}

fn layer_window(ns: &str, layer: Layer, keyboard: KeyboardMode) -> gtk::Window {
    let w = gtk::Window::new();
    w.init_layer_shell();
    w.set_namespace(Some(ns));
    w.set_layer(layer);
    for e in [Edge::Left, Edge::Right, Edge::Top, Edge::Bottom] {
        w.set_anchor(e, true);
    }
    w.set_exclusive_zone(-1);
    w.set_keyboard_mode(keyboard);
    w.add_css_class("aura-surface");
    w
}

impl Panel {
    fn build(raise_at_start: bool) -> Rc<Self> {
        let css = gtk::CssProvider::new();
        css.load_from_path(format!("{}/.config/hypr/aura-panel/style.css", home()));
        gtk::style_context_add_provider_for_display(&gdk::Display::default().unwrap(), &css, gtk::STYLE_PROVIDER_PRIORITY_USER);

        // ---- desktop surface: cards sit on the bottom layer like a home screen ----
        let desk = layer_window("dashboard", Layer::Bottom, KeyboardMode::None);
        let saved = std::fs::read_to_string(monitor_state_file()).unwrap_or_default().trim().to_string();
        let mon = monitor_named(&saved).or_else(focused_monitor);
        if let Some(m) = &mon {
            desk.set_monitor(Some(m));
        }
        let width = mon.as_ref().map(|m| m.geometry().width()).unwrap_or(1920);
        let wide = width >= 1500;
        let avail = width - 16 - 28 - 24;
        let col_w = if wide { (avail as f64 * 0.24) as i32 } else { (avail - 12) / 2 };
        let docker_w = if wide { avail - 2 * col_w } else { col_w };
        let per_line = if docker_w >= 900 { 3 } else if docker_w >= 560 { 2 } else { 1 };

        let root = hbox(12);
        root.add_css_class("desktop-widgets");
        root.set_margin_start(8);
        root.set_margin_end(8);
        root.set_margin_top(38); // clear the top bar
        root.set_margin_bottom(64); // clear the dock
        root.set_valign(gtk::Align::Start);
        let columns: Vec<gtk::Box> = [docker_w, col_w, col_w]
            .iter()
            .map(|w| {
                let c = gtk::Box::new(gtk::Orientation::Vertical, 12);
                c.set_size_request(*w, -1);
                root.append(&c);
                c
            })
            .collect();
        desk.set_child(Some(&root));

        // ---- controls sidebar (overlay, right edge), Super+D ----
        let side = layer_window("dashboard-controls", Layer::Overlay, KeyboardMode::Exclusive);
        side.set_exclusive_zone(0); // stay below the top bar, so its dashboard icon can close the sidebar

        let docker_list = gtk::FlowBox::new();
        docker_list.set_selection_mode(gtk::SelectionMode::None);
        docker_list.set_min_children_per_line(per_line);
        docker_list.set_max_children_per_line(per_line);
        docker_list.set_column_spacing(10);
        docker_list.set_row_spacing(10);
        docker_list.set_valign(gtk::Align::Start);

        let light_drop = gtk::DropDown::from_strings(&[]);
        light_drop.set_hexpand(true);
        let monitor_drop = gtk::DropDown::from_strings(&[]);

        let p = Rc::new(Self {
            desk,
            side,
            raised: Cell::new(false),
            columns,
            sys_rows: RefCell::new(vec![]),
            cpu_prev: Cell::new((0, 0)),
            docker_summary: rlabel("loading…", &["dim"]),
            docker_metrics: RefCell::new(vec![]),
            docker_list,
            viz: Visualizer::new(),
            media_title: label("Nothing playing", &["media-title"]),
            media_artist: label("", &["media-artist"]),
            media_player: rlabel("", &["dim"]),
            media_art: gtk::Image::new(),
            media_play: gtk::Button::with_label("󰐊"),
            media_bus: RefCell::new(None),
            media_art_url: RefCell::new(String::new()),
            light_drop,
            light_ids: RefCell::new(vec![]),
            scene_catalog: RefCell::new(json!({})),
            lights_state: rlabel("", &["dim"]),
            light_power: gtk::Button::with_label("󰌶"),
            lights_on: Cell::new(true),
            light_bright: gtk::Scale::with_range(gtk::Orientation::Horizontal, 0.0, 100.0, 5.0),
            light_bright_lbl: label("30%", &["dim"]),
            light_speed: gtk::Scale::with_range(gtk::Orientation::Horizontal, 0.0, 4.0, 1.0),
            light_speed_lbl: label("Calm", &["dim"]),
            light_speed_row: hbox(10),
            updating_lights: Cell::new(false),
            light_debounce: RefCell::new(None),
            confirm_bar: gtk::Revealer::new(),
            confirm_text: label("", &["confirm-text"]),
            confirm_go: gtk::Button::with_label(""),
            confirm_cmd: RefCell::new(String::new()),
            confirm_timeout: RefCell::new(None),
            dnd: gtk::Button::with_label("󰂚"),
            notif_list: gtk::Box::new(gtk::Orientation::Vertical, 6),
            notifications: RefCell::new(vec![]),
            notif_fresh: Cell::new(false),
            monitor_drop,
            monitor_names: RefCell::new(vec![]),
            connections: RefCell::new(None),
            catchers: RefCell::new(vec![]),
            cal_win: layer_window("calendar", Layer::Overlay, KeyboardMode::OnDemand),
            cal: calendar::Calendar::new(),
        });
        p.assemble();
        p.desk.present();
        p.start_timers();
        if raise_at_start {
            p.peek();
        }
        p
    }

    fn assemble(self: &Rc<Self>) {
        let me = self.clone();
        let conns = Connections::new(move || me.close());
        *self.connections.borrow_mut() = Some(conns.clone());

        let docker = self.build_docker();
        let media = self.build_media();
        let actions = self.build_actions();
        let folders = self.build_folders();
        let system = self.build_system();
        let lights = self.build_lights();
        for (col, cards) in self.columns.iter().zip([
            vec![docker, media],
            vec![actions, folders],
            vec![system, lights, conns.audio_card.clone()],
        ]) {
            for c in cards {
                col.append(&c);
            }
        }

        // sidebar
        let sidebox = gtk::Box::new(gtk::Orientation::Vertical, 12);
        sidebox.add_css_class("panel");
        sidebox.set_size_request(380, -1);
        sidebox.set_margin_top(8);
        sidebox.set_margin_bottom(16);
        sidebox.set_margin_end(8);
        sidebox.append(&self.build_power());
        sidebox.append(&self.build_confirm());
        sidebox.append(&self.build_monitor_selector());
        sidebox.append(&conns.widget);
        sidebox.append(&self.build_notifications());
        let close = gtk::Button::with_label("Close");
        let me = self.clone();
        close.connect_clicked(move |_| me.close());
        sidebox.append(&close);
        let scroll = gtk::ScrolledWindow::new();
        scroll.set_hscrollbar_policy(gtk::PolicyType::Never);
        scroll.set_child(Some(&sidebox));
        scroll.set_halign(gtk::Align::End);
        scroll.set_size_request(400, -1);
        let outside = gtk::Box::new(gtk::Orientation::Vertical, 0);
        outside.add_css_class("backdrop");
        outside.add_css_class("raised"); // dimmed; Hyprland blurs behind it (rules.conf)
        outside.set_hexpand(true);
        outside.set_vexpand(true);
        let me = self.clone();
        on_click(&outside, move || me.close());
        let overlay = gtk::Overlay::new();
        overlay.set_child(Some(&outside));
        overlay.add_overlay(&scroll);
        self.side.set_child(Some(&overlay));
        // Keyboard: only Esc does anything here. Every other key (Tab, arrows, Enter, Space…) is
        // swallowed in the capture phase, and nothing in the sidebar can take keyboard focus.
        let keys = gtk::EventControllerKey::new();
        keys.set_propagation_phase(gtk::PropagationPhase::Capture);
        let me = self.clone();
        keys.connect_key_pressed(move |_, key, _, _| {
            if key == gdk::Key::Escape {
                if me.confirm_bar.reveals_child() {
                    me.dismiss_confirm();
                } else {
                    me.close();
                }
            }
            glib::Propagation::Stop
        });
        self.side.add_controller(keys);
        self.build_calendar();
        unfocusable(overlay.upcast_ref());
    }

    // ---------------- sections ----------------
    /// Month calendar under the top-bar clock; Esc or a click outside closes it.
    fn build_calendar(self: &Rc<Self>) {
        self.cal_win.set_exclusive_zone(0); // below the top bar
        let outside = gtk::Box::new(gtk::Orientation::Vertical, 0);
        outside.set_hexpand(true);
        outside.set_vexpand(true);
        let overlay = gtk::Overlay::new();
        overlay.set_child(Some(&outside));
        self.cal.widget.set_halign(gtk::Align::End);
        self.cal.widget.set_valign(gtk::Align::Start);
        self.cal.widget.set_margin_top(8);
        self.cal.widget.set_margin_end(8);
        overlay.add_overlay(&self.cal.widget);
        self.cal_win.set_child(Some(&overlay));
        let me = self.clone();
        on_click(&outside, move || me.cal_win.set_visible(false));
        let keys = gtk::EventControllerKey::new();
        let me = self.clone();
        keys.connect_key_pressed(move |_, k, _, _| {
            if k == gdk::Key::Escape {
                me.cal_win.set_visible(false);
                return glib::Propagation::Stop;
            }
            glib::Propagation::Proceed
        });
        self.cal_win.add_controller(keys);
    }

    fn toggle_calendar(self: &Rc<Self>) {
        if self.cal_win.is_visible() {
            self.cal_win.set_visible(false);
        } else {
            if let Some(m) = focused_monitor() {
                self.cal_win.set_monitor(Some(&m));
            }
            self.cal.reset();
            self.cal_win.present();
        }
    }

    fn build_power(self: &Rc<Self>) -> gtk::Box {
        let power = hbox(0);
        power.add_css_class("card");
        power.add_css_class("power");
        // buttons sit in a centered row inside the full-width card
        let row = hbox(10);
        row.set_halign(gtk::Align::Center);
        row.set_hexpand(true);
        power.append(&row);
        for (key, glyph, tip, cmd, cls) in [
            ("lock", "󰌾", "Lock", "loginctl lock-session", "c-blue"),
            ("suspend", "󰤄", "Suspend", "systemctl suspend", "c-purple"),
            ("reboot", "󰜉", "Reboot", "~/.config/hypr/scripts/session.sh stop; systemctl reboot", "c-orange"),
            ("shutdown", "\u{f011}", "Shut down", "~/.config/hypr/scripts/session.sh stop; systemctl poweroff", "c-red"),
        ] {
            let me = self.clone();
            row.append(&icon_button(glyph, tip, &["solid", cls], move || me.power(key, cmd)));
        }
        power
    }

    fn power(self: &Rc<Self>, key: &str, cmd: &str) {
        // every power action asks first, inline, so nothing can open behind the overlay
        let verb = match key { "lock" => "Lock", "suspend" => "Suspend", "reboot" => "Reboot", _ => "Shut down" };
        self.confirm_text.set_text(&format!("{verb} now?"));
        self.confirm_go.set_label(verb);
        *self.confirm_cmd.borrow_mut() = cmd.to_string();
        self.confirm_bar.set_reveal_child(true);
        // auto-dismiss: an unanswered prompt shouldn't linger (or fire later by accident)
        if let Some(id) = self.confirm_timeout.borrow_mut().take() {
            id.remove();
        }
        let me = self.clone();
        let id = glib::timeout_add_local_once(Duration::from_secs(5), move || {
            me.confirm_timeout.borrow_mut().take();
            me.confirm_bar.set_reveal_child(false);
        });
        *self.confirm_timeout.borrow_mut() = Some(id);
    }

    fn build_confirm(self: &Rc<Self>) -> gtk::Revealer {
        self.confirm_bar.set_transition_type(gtk::RevealerTransitionType::SlideDown);
        self.confirm_bar.set_transition_duration(150);
        let row = hbox(8);
        row.add_css_class("confirm-bar");
        self.confirm_text.set_hexpand(true);
        row.append(&self.confirm_text);
        let cancel = gtk::Button::with_label("Cancel");
        cancel.add_css_class("confirm-cancel");
        let me = self.clone();
        cancel.connect_clicked(move |_| me.dismiss_confirm());
        self.confirm_go.add_css_class("confirm-go");
        let me = self.clone();
        self.confirm_go.connect_clicked(move |_| {
            me.dismiss_confirm();
            me.close();
            sh(&me.confirm_cmd.borrow());
        });
        row.append(&cancel);
        row.append(&self.confirm_go);
        self.confirm_bar.set_child(Some(&row));
        self.confirm_bar.clone()
    }

    fn dismiss_confirm(&self) {
        if let Some(id) = self.confirm_timeout.borrow_mut().take() {
            id.remove();
        }
        self.confirm_bar.set_reveal_child(false);
    }

    fn build_monitor_selector(self: &Rc<Self>) -> gtk::Box {
        let b = card(&[]);
        b.append(&label("Primary monitor", &["title"]));
        let mons: Value = serde_json::from_str(&out("hyprctl monitors -j")).unwrap_or(json!([]));
        let saved = std::fs::read_to_string(monitor_state_file()).unwrap_or_default().trim().to_string();
        let current = self.desk.monitor().and_then(|m| m.connector()).map(|c| c.to_string()).unwrap_or_default();
        let mut names = vec![];
        let model = gtk::StringList::new(&[]);
        for m in mons.as_array().into_iter().flatten() {
            let n = m["name"].as_str().unwrap_or("").to_string();
            model.append(&format!("{} · {}", n, m["model"].as_str().unwrap_or("Display")));
            names.push(n);
        }
        self.monitor_drop.set_model(Some(&model));
        let pick = if names.contains(&saved) { saved } else { current };
        if let Some(i) = names.iter().position(|n| *n == pick) {
            self.monitor_drop.set_selected(i as u32);
        }
        *self.monitor_names.borrow_mut() = names;
        let me = self.clone();
        self.monitor_drop.connect_selected_notify(move |d| {
            let Some(name) = me.monitor_names.borrow().get(d.selected() as usize).cloned() else { return };
            let _ = std::fs::write(monitor_state_file(), format!("{name}\n"));
            if let Some(m) = monitor_named(&name) {
                me.desk.set_visible(false);
                me.desk.set_monitor(Some(&m));
                me.desk.set_visible(true);
            }
        });
        b.append(&self.monitor_drop);
        b
    }

    fn build_actions(self: &Rc<Self>) -> gtk::Box {
        let c = card(&[]);
        c.append(&label("Quick actions", &["title"]));
        let grid = gtk::Grid::new();
        grid.set_column_spacing(10);
        grid.set_row_spacing(10);
        grid.set_column_homogeneous(true);
        let actions = [
            ("󰍉", "Launcher (Vicinae)", "vicinae toggle".to_string()),
            ("󰹑", "Screenshot region → clipboard", r#"sleep 0.3; grim -g "$(slurp)" - | wl-copy && notify-send "Screenshot copied""#.to_string()),
            ("󰆍", "Ghostty", "ghostty".to_string()),
            ("󰊯", "Chrome", "gtk-launch google-chrome.desktop".to_string()),
            ("󰨞", "VS Code", "gtk-launch com.microsoft.VSCode.desktop".to_string()),
            ("󰕮", "Status page", format!("xdg-open {STATUS_API}")),
        ];
        for (i, (g, tip, cmd)) in actions.into_iter().enumerate() {
            // screenshots keep the panel up so it can be captured too
            let keep = tip.starts_with("Screenshot");
            let me = self.clone();
            let b = icon_button(g, tip, &["action"], move || {
                if !keep {
                    me.close();
                }
                sh(&cmd);
            });
            grid.attach(&b, (i % 3) as i32, (i / 3) as i32, 1, 1);
        }
        c.append(&grid);
        c
    }

    fn build_folders(self: &Rc<Self>) -> gtk::Box {
        let c = card(&[]);
        c.append(&label("Folders", &["title"]));
        let grid = gtk::Grid::new();
        grid.set_column_spacing(8);
        grid.set_row_spacing(4);
        grid.set_column_homogeneous(true);
        let folders = [("Downloads", "~/Downloads"), ("Documents", "~/Documents"), ("Pictures", "~/Pictures"),
            ("Videos", "~/Videos"), ("Music", "~/Music"), ("dev", "~/dev"),
            ("media-server", "~/media-server"), ("HDD", "/mnt/hdd"), ("Home", "~")];
        for (i, (name, path)) in folders.into_iter().enumerate() {
            let b = gtk::Button::new();
            b.set_child(Some(&label(&format!("󰉋  {name}"), &[])));
            b.add_css_class("folder");
            let me = self.clone();
            b.connect_clicked(move |_| {
                me.close();
                sh(&format!("nautilus {path}"));
            });
            grid.attach(&b, (i % 3) as i32, (i / 3) as i32, 1, 1);
        }
        c.append(&grid);
        c
    }

    /// System card in the same style as the Docker totals: a grid of metric tiles
    /// (icon + name, big value, detail line).
    fn build_system(self: &Rc<Self>) -> gtk::Box {
        let c = card(&[]);
        c.append(&label("System", &["title"]));
        let grid = gtk::FlowBox::new();
        grid.set_selection_mode(gtk::SelectionMode::None);
        grid.set_homogeneous(true);
        // two per row: three don't fit the column and spilled off the screen edge
        grid.set_min_children_per_line(2);
        grid.set_max_children_per_line(2);
        grid.set_column_spacing(8);
        grid.set_row_spacing(8);
        let mut rows = vec![];
        for (kind, title, glyph, cls) in discover_rows() {
            let chip = gtk::Box::new(gtk::Orientation::Vertical, 2);
            chip.add_css_class("metric-content");
            let top = hbox(6);
            top.append(&label(glyph, &["metric-icon", cls]));
            top.append(&label(&title, &["dim"]));
            chip.append(&top);
            // hardware model under the title for CPU and GPU tiles
            let model = match &kind { SysRow::Cpu => hw::cpu_name(), SysRow::Gpu(g) => g.name.clone(), _ => String::new() };
            if !model.is_empty() {
                let m = label(&model, &["metric-name"]);
                m.set_tooltip_text(Some(&model));
                chip.append(&m);
            }
            let value = label("–", &["metric-value"]);
            chip.append(&value);
            let detail = label("", &["dim"]);
            chip.append(&detail);
            let clock = label("", &["dim", "metric-clock"]);
            clock.set_visible(!matches!(kind, SysRow::Disk { .. }));
            chip.append(&clock);
            // same chip as the Docker totals; the bar is kept (hidden) only to carry the drive-model tooltip
            // The hidden level bar holds the usage fraction; the "bucket" behind the text is
            // filled from the bottom to that height in the icon's colour.
            let bar = gtk::LevelBar::new();
            bar.set_visible(false);
            chip.append(&bar);
            let (r, g, b) = match cls { "c-red" => (1.0, 0.404, 0.404), "c-orange" => (1.0, 0.792, 0.522),
                                        "c-pink" => (0.965, 0.580, 1.0), _ => (0.635, 0.467, 1.0) };
            let fill = gtk::DrawingArea::new();
            // live traces for things that move (CPU clock, memory use, GPU load); drives keep the bucket
            let sampler: Option<Box<dyn Fn() -> f64>> = match &kind {
                SysRow::Cpu => Some(Box::new(cpu_clock_frac())),
                SysRow::Ram => Some(Box::new(mem_frac)),
                SysRow::Gpu(g) => {
                    let g = g.clone();
                    Some(Box::new(move || hw::stats(&g).busy.unwrap_or(0.0)))
                }
                SysRow::Disk { dev, .. } => Some(Box::new(disk_activity(dev.clone()))),
            };
            // drives also keep the bucket fill behind the text (height = how full the drive is)
            let bucket = matches!(kind, SysRow::Disk { .. });
            // drives: activity spikes ride on the bucket's surface line (one bar, not two)
            const SPIKES: usize = 40;
            let spikes: Rc<RefCell<std::collections::VecDeque<f64>>> =
                Rc::new(RefCell::new(std::collections::VecDeque::from(vec![0.0; SPIKES])));
            let sampler = if bucket {
                if let Some(sample) = sampler {
                    let (hist, weak) = (spikes.clone(), fill.downgrade());
                    glib::timeout_add_local(Duration::from_millis(500), move || {
                        let Some(area) = weak.upgrade() else { return glib::ControlFlow::Break };
                        if area.is_mapped() {
                            let mut h = hist.borrow_mut();
                            h.pop_front();
                            h.push_back(sample());
                            drop(h);
                            area.queue_draw();
                        }
                        glib::ControlFlow::Continue
                    });
                }
                None
            } else { sampler };
            if let Some(sample) = sampler {
                // the trace gets its own strip under the text (a background line crossed the numbers)
                let graph = gtk::DrawingArea::new();
                graph.set_content_height(30);
                graph.set_margin_top(6);
                ecg(&graph, (r, g, b), sample);
                chip.append(&graph);
            }
            if bucket {
            let level = bar.clone();
            let hist = spikes.clone();
            fill.set_draw_func(move |_, cr, w, h| {
                let (w, h) = (w as f64, h as f64);
                let rad = 10.0;
                // clip to the tile's rounded shape
                cr.new_sub_path();
                cr.arc(w - rad, rad, rad, -std::f64::consts::FRAC_PI_2, 0.0);
                cr.arc(w - rad, h - rad, rad, 0.0, std::f64::consts::FRAC_PI_2);
                cr.arc(rad, h - rad, rad, std::f64::consts::FRAC_PI_2, std::f64::consts::PI);
                cr.arc(rad, rad, rad, std::f64::consts::PI, 1.5 * std::f64::consts::PI);
                cr.close_path();
                cr.clip();
                let frac = level.value().clamp(0.0, 1.0);
                let top = h * (1.0 - frac);
                // the surface line carries the activity trace: spikes rise above the fill level
                let s = hist.borrow();
                let amp = 18.0_f64.min(top.max(6.0));
                let pt = |i: usize, v: f64| (w * i as f64 / (SPIKES - 1) as f64, top - amp * v.clamp(0.0, 1.0));
                let surface = |cr: &gtk::cairo::Context| {
                    for (i, v) in s.iter().enumerate() {
                        let (x, y) = pt(i, *v);
                        if i == 0 { cr.move_to(x, y) } else { cr.line_to(x, y) }
                    }
                };
                surface(cr);
                cr.line_to(w, h);
                cr.line_to(0.0, h);
                cr.close_path();
                cr.set_source_rgba(r, g, b, 0.22);
                let _ = cr.fill();
                // a brighter surface line, like liquid in a bucket
                cr.set_line_join(gtk::cairo::LineJoin::Round);
                cr.set_line_cap(gtk::cairo::LineCap::Round);
                surface(cr);
                cr.set_source_rgba(r, g, b, 0.85);
                cr.set_line_width(1.6);
                let _ = cr.stroke();
            });
            }
            let tile = gtk::Overlay::new();
            tile.add_css_class("metric");
            tile.set_child(Some(&fill));
            tile.add_overlay(&chip);
            tile.set_measure_overlay(&chip, true);
            let fill2 = fill.clone();
            bar.connect_value_notify(move |_| fill2.queue_draw());
            grid.insert(&tile, -1);
            rows.push((kind, bar, value, detail, clock));
        }
        c.append(&grid);
        *self.sys_rows.borrow_mut() = rows;
        c
    }

    fn build_lights(self: &Rc<Self>) -> gtk::Box {
        let c = card(&[]);
        let head = hbox(0);
        let t = label("󰛨  Lights", &["title"]);
        t.set_hexpand(true);
        head.append(&t);
        head.append(&self.lights_state);
        c.append(&head);

        let row = hbox(8);
        row.append(&self.light_drop);
        self.light_power.set_tooltip_text(Some("Turn lights off"));
        for cls in ["icon-btn", "small", "c-purple"] {
            self.light_power.add_css_class(cls);
        }
        let me = self.clone();
        self.light_power.connect_clicked(move |_| {
            let mode = if me.lights_on.get() { "off".to_string() } else { me.selected_scene().unwrap_or_else(|| "aura".into()) };
            me.set_light(&mode);
        });
        row.append(&self.light_power);
        c.append(&row);
        let me = self.clone();
        self.light_drop.connect_selected_notify(move |_| {
            me.update_speed_row();
            if !me.updating_lights.get() {
                if let Some(s) = me.selected_scene() {
                    me.set_light(&s);
                }
            }
        });

        let br = hbox(10);
        br.append(&label("Brightness", &["dim"]));
        self.light_bright.set_value(30.0);
        self.light_bright.set_draw_value(false);
        self.light_bright.set_hexpand(true);
        no_wheel(&self.light_bright);
        br.append(&self.light_bright);
        br.append(&self.light_bright_lbl);
        c.append(&br);

        self.light_speed_row.append(&label("Speed", &["dim"]));
        self.light_speed.set_value(1.0);
        self.light_speed.set_draw_value(false);
        self.light_speed.set_hexpand(true);
        no_wheel(&self.light_speed);
        self.light_speed_row.append(&self.light_speed);
        self.light_speed_row.append(&self.light_speed_lbl);
        c.append(&self.light_speed_row);
        for s in [&self.light_bright, &self.light_speed] {
            let me = self.clone();
            s.connect_value_changed(move |_| me.queue_light_update());
        }
        c
    }

    fn selected_scene(&self) -> Option<String> {
        self.light_ids.borrow().get(self.light_drop.selected() as usize).cloned()
    }

    /// Speed only means something for animated scenes; hide it for solid colours.
    fn update_speed_row(&self) {
        let key = self.selected_scene().unwrap_or_default();
        let cat = self.scene_catalog.borrow();
        let effect = cat[&key]["effect"].as_str().unwrap_or(&key).to_string();
        self.light_speed_row.set_visible(!STATIC_EFFECTS.contains(&effect.as_str()));
    }

    fn queue_light_update(self: &Rc<Self>) {
        self.light_bright_lbl.set_text(&format!("{}%", self.light_bright.value() as i64));
        let sp = SPEEDS[self.light_speed.value().round() as usize];
        self.light_speed_lbl.set_text(&format!("{}{}", sp[..1].to_uppercase(), &sp[1..]));
        if let Some(id) = self.light_debounce.borrow_mut().take() {
            id.remove();
        }
        let me = self.clone();
        let id = glib::timeout_add_local_once(Duration::from_millis(180), move || {
            me.light_debounce.borrow_mut().take();
            if me.lights_on.get() {
                if let Some(s) = me.selected_scene() {
                    me.set_light(&s);
                }
            }
        });
        *self.light_debounce.borrow_mut() = Some(id);
    }

    fn set_light(self: &Rc<Self>, mode: &str) {
        let mut body = json!({"target": "all", "mode": mode});
        if mode == "aura" {
            body["color"] = json!("6B21A8");
        }
        if mode != "off" {
            body["brightness"] = json!(self.light_bright.value() as i64);
            body["speed"] = json!(SPEEDS[self.light_speed.value().round() as usize]);
        }
        self.lights_on.set(mode != "off");
        self.light_power.set_label(if self.lights_on.get() { "󰌶" } else { "󰌵" });
        self.light_power.set_tooltip_text(Some(if self.lights_on.get() { "Turn lights off" } else { "Turn lights on" }));
        self.lights_state.set_text("applying…");
        let me = self.clone();
        let m = mode.to_string();
        bg(move || api("/api/lights", Some(&body)), move |r| {
            me.lights_state.set_text(if r.is_some() { &m } else { "error" });
        });
    }

    fn build_docker(self: &Rc<Self>) -> gtk::Box {
        let c = card(&[]);
        let head = hbox(0);
        let t = label("Containers", &["title"]);
        t.set_hexpand(true);
        head.append(&t);
        head.append(&self.docker_summary);
        c.append(&head);
        let metrics = hbox(8);
        metrics.set_homogeneous(true);
        let mut m = vec![];
        for (glyph, title, cls) in [("󰻠", "CPU", "c-red"), ("󰍛", "Memory", "c-orange"), ("󰢮", "GPU", "c-purple")] {
            let chip = gtk::Box::new(gtk::Orientation::Vertical, 2);
            chip.add_css_class("metric");
            let top = hbox(6);
            top.append(&label(glyph, &["metric-icon", cls]));
            top.append(&label(title, &["dim"]));
            chip.append(&top);
            let value = label("–", &["metric-value"]);
            let detail = label("", &["dim"]);
            chip.append(&value);
            chip.append(&detail);
            metrics.append(&chip);
            m.push((value, detail));
        }
        *self.docker_metrics.borrow_mut() = m;
        c.append(&metrics);
        let inner = gtk::ScrolledWindow::new();
        inner.set_hscrollbar_policy(gtk::PolicyType::Never);
        inner.set_propagate_natural_height(true); // Docker takes only the height it needs
        inner.set_child(Some(&self.docker_list));
        c.append(&inner);
        c
    }

    fn build_media(self: &Rc<Self>) -> gtk::Box {
        let c = card(&["media"]);
        let head = hbox(0);
        let t = label("󰎆  Now playing", &["title"]);
        t.set_hexpand(true);
        head.append(&t);
        head.append(&self.media_player);
        c.append(&head);
        let body = hbox(12);
        self.media_art.set_pixel_size(25);
        self.media_art.add_css_class("art");
        body.append(&self.media_art);
        let info = gtk::Box::new(gtk::Orientation::Vertical, 3);
        info.set_valign(gtk::Align::Center);
        info.set_hexpand(true);
        info.append(&self.media_title);
        info.append(&self.media_artist);
        body.append(&info);
        let controls = hbox(10);
        controls.set_valign(gtk::Align::Center);
        let me = self.clone();
        controls.append(&icon_button("󰒮", "Previous", &["media-btn"], move || me.media_call("Previous")));
        for cls in ["icon-btn", "media-btn", "solid", "c-purple"] {
            self.media_play.add_css_class(cls);
        }
        self.media_play.set_tooltip_text(Some("Play / pause"));
        let me = self.clone();
        self.media_play.connect_clicked(move |_| me.media_call("PlayPause"));
        controls.append(&self.media_play);
        let me = self.clone();
        controls.append(&icon_button("󰒭", "Next", &["media-btn"], move || me.media_call("Next")));
        body.append(&controls);
        c.append(&body);
        c.append(&self.viz.area);
        c
    }

    fn build_notifications(self: &Rc<Self>) -> gtk::Box {
        let c = card(&[]);
        c.set_vexpand(true);
        let head = hbox(8);
        let t = label("󰂚  Notifications", &["title"]);
        t.set_hexpand(true);
        head.append(&t);
        let me = self.clone();
        head.append(&icon_button("󰎟", "Clear history", &["small"], move || {
            me.notifications.borrow_mut().clear();
            me.save_notifications();
            me.render_notifications();
        }));
        // Do Not Disturb as an icon toggle in the header (bell / bell-off), next to "clear"
        for cls in ["icon-btn", "small", "dnd-btn"] {
            self.dnd.add_css_class(cls);
        }
        head.insert_child_after(&self.dnd, Some(&t));
        c.append(&head);
        let me = self.clone();
        self.dnd.connect_clicked(move |_| {
            let me2 = me.clone();
            bg(|| out("makoctl mode -t do-not-disturb; pkill -RTMIN+8 waybar"), move |_| me2.refresh_dnd());
        });
        let inner = gtk::ScrolledWindow::new();
        inner.set_hscrollbar_policy(gtk::PolicyType::Never);
        inner.set_vexpand(true);
        inner.set_child(Some(&self.notif_list));
        c.append(&inner);
        let saved: Vec<Value> = std::fs::read_to_string(notif_log()).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default();
        *self.notifications.borrow_mut() = saved.into_iter().rev().take(50).rev().collect();
        self.render_notifications();
        self.watch_notifications();
        c
    }

    fn save_notifications(&self) {
        let n = self.notifications.borrow();
        let _ = std::fs::write(notif_log(), Value::Array(n.clone()).to_string());
    }

    fn render_notifications(&self) {
        clear(&self.notif_list);
        let n = self.notifications.borrow();
        if n.is_empty() {
            self.notif_list.append(&label("Nothing new", &["dim"]));
        }
        let fresh = self.notif_fresh.replace(false);
        for (i, x) in n.iter().rev().take(20).enumerate() {
            let b = gtk::Box::new(gtk::Orientation::Vertical, 2);
            b.add_css_class("notif");
            let top = hbox(8);
            let app = label(x["app"].as_str().filter(|s| !s.is_empty()).unwrap_or("notification"), &["notif-app"]);
            app.set_hexpand(true);
            top.append(&app);
            let mins = ((now() - x["t"].as_f64().unwrap_or(now())) / 60.0) as i64;
            let ago = if mins < 1 { "now".into() } else if mins < 60 { format!("{mins}m") } else if mins < 1440 { format!("{}h", mins / 60) } else { format!("{}d", mins / 1440) };
            top.append(&rlabel(&ago, &["dim"]));
            b.append(&top);
            if let Some(s) = x["summary"].as_str().filter(|s| !s.is_empty()) {
                b.append(&label(s, &["notif-summary"]));
            }
            if let Some(s) = x["body"].as_str().filter(|s| !s.is_empty()) {
                let l = label(&s.replace("\\n", " "), &["dim"]);
                l.set_wrap(true);
                l.set_lines(2);
                b.append(&l);
            }
            if i == 0 && fresh {
                // the one that just arrived slides down into place (200 ms, ease-out)
                let rev = gtk::Revealer::new();
                rev.set_transition_type(gtk::RevealerTransitionType::SlideDown);
                rev.set_transition_duration(200);
                rev.set_child(Some(&b));
                self.notif_list.append(&rev);
                glib::idle_add_local_once(move || rev.set_reveal_child(true));
            } else {
                self.notif_list.append(&b);
            }
        }
    }

    /// Eavesdrop on org.freedesktop.Notifications.Notify (mako still shows the popups).
    fn watch_notifications(self: &Rc<Self>) {
        let (tx, rx) = async_channel::unbounded::<Value>();
        std::thread::spawn(move || {
            use std::io::BufRead;
            let Ok(mut child) = std::process::Command::new("dbus-monitor")
                .args(["--session", "interface='org.freedesktop.Notifications',member='Notify'"])
                .stdout(std::process::Stdio::piped())
                .spawn()
            else {
                return;
            };
            let stdout = child.stdout.take().unwrap();
            let (mut args, mut collecting) = (Vec::<String>::new(), false);
            for line in std::io::BufReader::new(stdout).lines().map_while(Result::ok) {
                if line.contains("member=Notify") {
                    args.clear();
                    collecting = true;
                    continue;
                }
                if !collecting {
                    continue;
                }
                let l = line.trim();
                if let Some(s) = l.strip_prefix("string ") {
                    args.push(s.trim().trim_matches('"').to_string());
                } else if let Some(s) = l.strip_prefix("uint32 ") {
                    args.push(s.to_string());
                } else if l.starts_with("array") && args.len() >= 5 {
                    collecting = false;
                    let _ = tx.send_blocking(json!({"app": args[0], "summary": args[3], "body": args[4], "t": now()}));
                }
            }
        });
        let me = self.clone();
        glib::spawn_future_local(async move {
            while let Ok(n) = rx.recv().await {
                let mut v = me.notifications.borrow_mut();
                v.push(n);
                let len = v.len();
                if len > 50 {
                    v.drain(..len - 50);
                }
                drop(v);
                me.save_notifications();
                me.notif_fresh.set(true);
                me.render_notifications();
            }
        });
    }

    // ---------------- refresh ----------------
    fn start_timers(self: &Rc<Self>) {
        self.refresh_system();
        self.refresh_remote();
        self.refresh_media();
        self.refresh_dnd();
        for (secs, f) in [
            (2u32, Panel::refresh_system as fn(&Rc<Panel>)),
            (2, Panel::refresh_media),
            (3, Panel::refresh_dnd),
            (10, Panel::refresh_remote),
        ] {
            let weak = Rc::downgrade(self);
            glib::timeout_add_seconds_local(secs, move || {
                let Some(p) = weak.upgrade() else { return glib::ControlFlow::Break };
                f(&p);
                glib::ControlFlow::Continue
            });
        }
        // Lowered, only the cards take input, so the transparent rest of the surface never
        // swallows the pointer (keeps focus-follows-mouse working between windows).
        let weak = Rc::downgrade(self);
        glib::timeout_add_local(Duration::from_millis(1000), move || {
            let Some(p) = weak.upgrade() else { return glib::ControlFlow::Break };
            p.update_input_region();
            glib::ControlFlow::Continue
        });
    }

    fn update_input_region(&self) {
        let Some(surface) = self.desk.surface() else { return };
        let region = gtk::cairo::Region::create();
        for col in &self.columns {
            let mut child = col.first_child();
            while let Some(c) = child {
                if let Some(r) = c.compute_bounds(&self.desk) {
                    let _ = region.union_rectangle(&gtk::cairo::RectangleInt::new(
                        r.x() as i32, r.y() as i32, r.width() as i32, r.height() as i32));
                }
                child = c.next_sibling();
            }
        }
        if std::env::var_os("AURA_DEBUG").is_some() {
            let n = region.num_rectangles();
            let rects: Vec<String> = (0..n).map(|i| { let r = region.rectangle(i); format!("{}x{}+{}+{}", r.width(), r.height(), r.x(), r.y()) }).collect();
            eprintln!("input region: {}", rects.join(" "));
        }
        surface.set_input_region(Some(&region));
    }

    fn refresh_system(self: &Rc<Self>) {
        let read = |p: &str| std::fs::read_to_string(p).unwrap_or_default();
        // CPU from /proc/stat deltas; "cores used" = busy share × core count
        let stat = read("/proc/stat");
        let nums: Vec<u64> = stat.lines().next().unwrap_or("").split_whitespace().skip(1).filter_map(|x| x.parse().ok()).collect();
        let total: u64 = nums.iter().sum();
        let idle = nums.get(3).copied().unwrap_or(0) + nums.get(4).copied().unwrap_or(0);
        let (pt, pi) = self.cpu_prev.replace((total, idle));
        let cpu = if total > pt { 100.0 * (1.0 - (idle - pi) as f64 / (total - pt) as f64) } else { 0.0 };
        let cores = stat.lines().filter(|l| l.starts_with("cpu") && l.as_bytes().get(3).is_some_and(u8::is_ascii_digit)).count().max(1) as f64;
        let mi = read("/proc/meminfo");
        let kb = |k: &str| mi.lines().find(|l| l.starts_with(k)).and_then(|l| l.split_whitespace().nth(1)).and_then(|v| v.parse::<f64>().ok()).unwrap_or(0.0) * 1024.0;
        let (mt, ma) = (kb("MemTotal:"), kb("MemAvailable:"));
        // CPU clocks: average across cores and the fastest core right now
        let freqs: Vec<f64> = std::fs::read_dir("/sys/devices/system/cpu").into_iter().flatten().flatten()
            .filter_map(|e| read_u64(&format!("{}/cpufreq/scaling_cur_freq", e.path().display())))
            .map(|k| k as f64 / 1e6).collect();
        let cpu_clock = if freqs.is_empty() { String::new() } else {
            format!("{:.1} GHz avg · {:.1} peak", freqs.iter().sum::<f64>() / freqs.len() as f64, freqs.iter().cloned().fold(0.0, f64::max))
        };
        // RAM speed needs root to read (DMI); taken once from a file the user can create
        let ram_speed = std::fs::read_to_string(format!("{}/.config/aura-panel/ram-speed", home())).unwrap_or_default().trim().to_string();
        for (kind, bar, value, detail, clock) in self.sys_rows.borrow().iter() {
            clock.set_text(&match kind {
                SysRow::Cpu => cpu_clock.clone(),
                SysRow::Ram => if ram_speed.is_empty() { String::new() } else { ram_speed.clone() },
                SysRow::Gpu(g) => {
                    let st = hw::stats(g);
                    match (st.core_mhz, st.mem_mhz) {
                        (Some(c), Some(m)) => format!("core {c} MHz · mem {m} MHz"),
                        (Some(c), None) => format!("core {c} MHz"),
                        _ => String::new(),
                    }
                }
                SysRow::Disk { .. } => String::new(),
            });
            let (frac, big, txt) = match kind {
                SysRow::Cpu => (cpu / 100.0, format!("{cpu:.0}%"), format!("{:.1} of {cores:.0} cores", cpu / 100.0 * cores)),
                SysRow::Ram => {
                    let used = mt - ma;
                    (used / mt.max(1.0), human(used), format!("{:.0}% of {}", used / mt.max(1.0) * 100.0, human(mt)))
                }
                SysRow::Gpu(g) => {
                    let st = hw::stats(g);
                    let big = st.busy.map(|b| format!("{:.0}%", b * 100.0)).unwrap_or_else(|| "–".into());
                    // iGPUs share system RAM, and nouveau reports nothing: say so instead of "0 B / 0 B"
                    let detail = match (st.vram_used, st.vram_total) {
                        (Some(u), Some(t)) if t > 0 && g.discrete => format!("VRAM {} / {}", human(u as f64), human(t as f64)),
                        _ if !g.discrete => "shared memory".to_string(),
                        _ => "no stats from driver".to_string(),
                    };
                    (st.busy.unwrap_or(0.0), big, detail)
                }
                SysRow::Disk { dev, model, hdd, mount } => {
                    let kind = if *hdd { "HDD" } else { "SSD" };
                    match mount {
                        Some(path) => {
                            let mut v: libc::statvfs = unsafe { std::mem::zeroed() };
                            let cp = std::ffi::CString::new(path.as_str()).unwrap();
                            if unsafe { libc::statvfs(cp.as_ptr(), &mut v) } == 0 {
                                let total = v.f_blocks as f64 * v.f_frsize as f64;
                                let used = total - v.f_bfree as f64 * v.f_frsize as f64;
                                (used / total.max(1.0), format!("{:.0}%", used / total.max(1.0) * 100.0), format!("{kind} · {} / {}", human(used), human(total)))
                            } else {
                                (0.0, "–".to_string(), format!("{kind} · unavailable"))
                            }
                        }
                        None => {
                            let size = read_u64(&format!("/sys/block/{dev}/size")).unwrap_or(0) as f64 * 512.0;
                            (0.0, human(size), format!("{kind} · not mounted"))
                        }
                    }
                    .then_tooltip(bar, model)
                }
            };
            bar.set_value(frac.clamp(0.0, 1.0));
            value.set_text(&big);
            detail.set_text(&txt);
        }
    }

    fn refresh_dnd(self: &Rc<Self>) {
        let me = self.clone();
        bg(|| out("makoctl mode").split_whitespace().any(|m| m == "do-not-disturb"), move |on| {
            me.dnd.set_label(if on { "󰂛" } else { "󰂚" });
            me.dnd.set_tooltip_text(Some(if on { "Do Not Disturb is on — click to turn off" } else { "Turn on Do Not Disturb" }));
            if on { me.dnd.add_css_class("on") } else { me.dnd.remove_css_class("on") }
        });
    }

    fn refresh_remote(self: &Rc<Self>) {
        let me = self.clone();
        bg(|| api("/api/overview", None), move |o| me.apply_overview(o));
        let me = self.clone();
        bg(|| api("/api/lights", None), move |l| me.apply_lights(l));
    }

    fn apply_overview(self: &Rc<Self>, o: Option<Value>) {
        let Some(o) = o else {
            self.docker_summary.set_text("status page offline");
            return;
        };
        let cs = o["containers"].as_array().cloned().unwrap_or_default();
        let running: Vec<&Value> = cs.iter().filter(|x| x["state"] == "running").collect();
        self.docker_summary.set_text(&format!("{} running · {} stopped", running.len(), cs.len() - running.len()));
        // Docker reports CPU per core (100% = one core); show it as a share of the whole machine
        let cores = o["host"]["cpus"].as_f64().unwrap_or(1.0).max(1.0);
        let cpu: f64 = running.iter().map(|x| x["cpuPercent"].as_f64().unwrap_or(0.0)).sum();
        let mem: f64 = running.iter().map(|x| x["memUsed"].as_f64().unwrap_or(0.0)).sum();
        let total = o["host"]["memTotal"].as_f64().unwrap_or(1.0);
        if let [c, m, g] = &self.docker_metrics.borrow()[..] {
            c.0.set_text(&format!("{:.1}%", cpu / cores));
            c.1.set_text(&format!("{:.2} of {} cores", cpu / 100.0, cores));
            m.0.set_text(&human(mem));
            m.1.set_text(&format!("{:.0}% of {}", mem / total * 100.0, human(total)));
            // no container has /dev/dri or /dev/kfd mapped, so none can use the GPU
            g.0.set_text("0%");
            g.1.set_text("no GPU access");
        }
        self.docker_list.remove_all();
        // one block per compose stack; stopped containers only shown for essential stacks
        let mut stacks: Vec<(String, Vec<&Value>)> = vec![];
        for x in &cs {
            let module = x["module"].as_str().unwrap_or("other").to_string();
            if x["state"] == "running" || ESSENTIAL_STACKS.contains(&module.as_str()) {
                match stacks.iter_mut().find(|(m, _)| *m == module) {
                    Some((_, v)) => v.push(x),
                    None => stacks.push((module, vec![x])),
                }
            }
        }
        // stacks with something down first, then alphabetical
        stacks.sort_by_key(|(m, v)| (v.iter().all(|x| x["state"] == "running"), m.clone()));
        for (m, mut group) in stacks {
            group.sort_by_key(|x| (x["state"] == "running", x["name"].as_str().unwrap_or("").to_string()));
            let up = group.iter().filter(|x| x["state"] == "running").count();
            let b = gtk::Box::new(gtk::Orientation::Vertical, 0);
            b.add_css_class("stackcard");
            let head = hbox(0);
            let t = label(&m, &["stack"]);
            t.set_hexpand(true);
            head.append(&t);
            head.append(&rlabel(&if up < group.len() { format!("{up}/{}", group.len()) } else { up.to_string() }, &["dim"]));
            b.append(&head);
            for x in group {
                b.append(&self.container_row(x));
            }
            self.docker_list.insert(&b, -1);
        }
    }

    fn container_row(self: &Rc<Self>, x: &Value) -> gtk::Button {
        let running = x["state"] == "running";
        let btn = gtk::Button::new();
        btn.add_css_class("container");
        let row = hbox(8);
        row.append(&label("●", &[if running { "c-green" } else { "c-red" }]));
        let ports = x["ports"].as_array().cloned().unwrap_or_default();
        let port = web_port(&ports);
        let mut title = x["service"].as_str().or(x["name"].as_str()).unwrap_or("?").to_string();
        if x["module"] == "arr" {
            if let Some(p) = &port {
                title = format!("{title}:{p}"); // arr apps: show where their web UI lives
            }
        }
        let name = label(&title, &["cname"]);
        name.set_hexpand(true);
        row.append(&name);
        let detail = if running {
            format!("{:.1}%  {}", x["cpuPercent"].as_f64().unwrap_or(0.0), human(x["memUsed"].as_f64().unwrap_or(0.0)))
        } else {
            x["status"].as_str().unwrap_or("").split(" (").next().unwrap_or("").to_string()
        };
        row.append(&rlabel(&detail, &["dim"]));
        btn.set_child(Some(&row));
        if let Some(p) = port {
            btn.set_tooltip_text(Some(&format!("Open http://localhost:{p}")));
            let me = self.clone();
            btn.connect_clicked(move |_| {
                if std::env::var_os("AURA_DEBUG").is_some() { eprintln!("container click -> port {p}"); }
                me.close();
                // Chrome adds the tab to its most recent window, which may be on another workspace;
                // follow it there (macOS brings the app forward) instead of seeming to do nothing.
                sh(&format!(
                    "xdg-open http://localhost:{p}; sleep 0.6; \
                     a=$(hyprctl clients -j | jq -r '[.[] | select(.class == \"google-chrome\")] | sort_by(.focusHistoryID) | .[0].address // \"\"'); \
                     [ -n \"$a\" ] && hyprctl dispatch focuswindow address:$a"
                ));
            });
        } else {
            btn.set_tooltip_text(x["image"].as_str());
        }
        btn
    }

    fn apply_lights(self: &Rc<Self>, l: Option<Value>) {
        let Some(l) = l else {
            self.lights_state.set_text("offline");
            return;
        };
        let cat = l["sceneCatalog"].clone();
        *self.scene_catalog.borrow_mut() = cat.clone();
        let title = |p: &str| {
            cat[p]["name"].as_str().map(String::from).unwrap_or_else(|| {
                p.split('-').map(|w| format!("{}{}", w[..1].to_uppercase(), &w[1..])).collect::<Vec<_>>().join(" ")
            })
        };
        let mut presets: Vec<String> = l["presets"].as_array().into_iter().flatten()
            .filter_map(|p| p.as_str()).filter(|p| *p != "off" && *p != "ambience").map(String::from).collect();
        presets.sort_by_key(|p| title(p).to_lowercase());
        if presets != *self.light_ids.borrow() {
            let selected = self.selected_scene();
            self.updating_lights.set(true);
            let model = gtk::StringList::new(&[]);
            for p in &presets {
                let meta = &cat[p];
                let mut colors: Vec<String> = match meta["palette"].as_array() {
                    Some(a) => a.iter().filter_map(|c| c.as_str()).map(String::from).collect(),
                    None => meta["color"].as_str().map(|c| vec![c.to_string()]).unwrap_or_default(),
                };
                colors.dedup();
                if p == "aura" {
                    colors = vec!["6B21A8".into()];
                }
                if let Some(low) = meta["low"].as_str() {
                    if !colors.iter().any(|c| c == low) {
                        colors.push(low.into());
                    }
                }
                let suffix = if meta["effect"] == "colorful" {
                    "Full spectrum".to_string()
                } else {
                    colors.iter().filter(|c| !c.is_empty()).map(|c| format!("#{}", c.trim_start_matches('#').to_uppercase())).collect::<Vec<_>>().join(" / ")
                };
                model.append(&if suffix.is_empty() { title(p) } else { format!("{} · {suffix}", title(p)) });
            }
            *self.light_ids.borrow_mut() = presets.clone();
            self.light_drop.set_model(Some(&model));
            let pick = selected.filter(|s| presets.contains(s)).or_else(|| presets.iter().find(|p| *p == "aura").cloned());
            if let Some(i) = pick.and_then(|s| presets.iter().position(|p| *p == s)) {
                self.light_drop.set_selected(i as u32);
            }
            self.updating_lights.set(false);
            self.update_speed_row();
        }
        if let Some(states) = l["states"].as_object() {
            if !states.is_empty() && self.lights_state.text() != "applying…" {
                let on = states.values().any(|v| !matches!(v.to_string().trim_matches('"').to_lowercase().as_str(), "off" | "false" | "0"));
                self.lights_on.set(on);
                self.light_power.set_label(if on { "󰌶" } else { "󰌵" });
                self.lights_state.set_text(&states.iter().map(|(k, v)| format!("{k}: {}", v.as_str().unwrap_or("?"))).collect::<Vec<_>>().join(" · "));
            }
        }
    }

    // ---------------- media (MPRIS over D-Bus) ----------------
    fn media_state(known_art: String) -> Option<Media> {
        let bus = gio::bus_get_sync(gio::BusType::Session, None::<&gio::Cancellable>).ok()?;
        let call = |dest: &str, path: &str, iface: &str, method: &str, args: Option<&glib::Variant>| {
            bus.call_sync(Some(dest), path, iface, method, args, None, gio::DBusCallFlags::NONE, 1000, None::<&gio::Cancellable>).ok()
        };
        let names: Vec<String> = call("org.freedesktop.DBus", "/org/freedesktop/DBus", "org.freedesktop.DBus", "ListNames", None)?
            .child_value(0).get()?;
        let mut players = vec![];
        for n in names.into_iter().filter(|n| n.starts_with(&format!("{MPRIS}."))) {
            let arg = (format!("{MPRIS}.Player"),).to_variant();
            if let Some(v) = call(&n, "/org/mpris/MediaPlayer2", "org.freedesktop.DBus.Properties", "GetAll", Some(&arg)) {
                players.push((n, glib::VariantDict::new(Some(&v.child_value(0)))));
            }
        }
        // prefer whatever is playing, then Spotify
        players.sort_by_key(|(n, d)| {
            let playing = d.lookup_value("PlaybackStatus", None).and_then(|v| v.str().map(String::from)) == Some("Playing".into());
            (!playing, !n.contains("spotify"))
        });
        let (name, props) = players.into_iter().next()?;
        let meta = glib::VariantDict::new(props.lookup_value("Metadata", None).as_ref());
        let s = |k: &str| meta.lookup_value(k, None).and_then(|v| v.str().map(String::from)).unwrap_or_default();
        let int = |v: Option<glib::Variant>| v.and_then(|v| v.get::<i64>().or_else(|| v.get::<u64>().map(|x| x as i64))).unwrap_or(0);
        let mut m = Media {
            name,
            status: props.lookup_value("PlaybackStatus", None).and_then(|v| v.str().map(String::from)).unwrap_or_default(),
            title: s("xesam:title"),
            artist: meta.lookup_value("xesam:artist", None).and_then(|v| v.get::<Vec<String>>()).unwrap_or_default().join(", "),
            pos: int(props.lookup_value("Position", None)),
            length: int(meta.lookup_value("mpris:length", None)),
            art: s("mpris:artUrl"),
            art_path: None,
        };
        if !m.art.is_empty() && m.art != known_art {
            m.art_path = if let Some(p) = m.art.strip_prefix("file://") {
                Some(p.to_string())
            } else {
                let dir = format!("{}/.cache/dashboard-art", home());
                let _ = std::fs::create_dir_all(&dir);
                let path = format!("{dir}/{:x}.img", m.art.bytes().fold(0xcbf29ce484222325u64, |h, b| (h ^ b as u64).wrapping_mul(0x100000001b3)));
                if !std::path::Path::new(&path).exists() {
                    let _ = std::process::Command::new("curl").args(["-s", "-m", "5", "-o", &path, &m.art]).status();
                }
                Some(path)
            };
        }
        Some(m)
    }

    fn refresh_media(self: &Rc<Self>) {
        let me = self.clone();
        let known = self.media_art_url.borrow().clone();
        bg(move || Panel::media_state(known), move |st| match st {
            None => {
                *me.media_bus.borrow_mut() = None;
                me.media_title.set_text("Nothing playing");
                me.media_artist.set_text("");
                me.media_player.set_text("");
                me.viz.playing.set(false);
                me.viz.progress.set(0.0);
            }
            Some(st) => {
                me.viz.playing.set(st.status == "Playing");
                me.media_player.set_text(&{
                    let n = st.name.rsplit('.').next().unwrap_or("");
                    format!("{}{}", n.get(..1).unwrap_or("").to_uppercase(), n.get(1..).unwrap_or(""))
                });
                me.media_title.set_text(if st.title.is_empty() { "Unknown title" } else { &st.title });
                me.media_artist.set_text(&st.artist);
                me.viz.progress.set(if st.length > 0 { (st.pos as f64 / st.length as f64).min(1.0) } else { 0.0 });
                me.media_play.set_label(if st.status == "Playing" { "󰏤" } else { "󰐊" });
                if let Some(p) = &st.art_path {
                    me.media_art.set_from_file(Some(p));
                    *me.media_art_url.borrow_mut() = st.art.clone();
                }
                *me.media_bus.borrow_mut() = Some(st.name);
            }
        });
    }

    fn media_call(self: &Rc<Self>, method: &'static str) {
        let Some(name) = self.media_bus.borrow().clone() else { return };
        let me = self.clone();
        bg(move || {
            if let Ok(bus) = gio::bus_get_sync(gio::BusType::Session, None::<&gio::Cancellable>) {
                let _ = bus.call_sync(Some(&name), "/org/mpris/MediaPlayer2", &format!("{MPRIS}.Player"), method,
                    None, None, gio::DBusCallFlags::NONE, 2000, None::<&gio::Cancellable>);
            }
        }, move |_| me.refresh_media());
    }

    // ---------------- show / hide ----------------
    /// Super+D: show/hide the controls sidebar on the focused monitor. No animation — it's a
    /// keyboard toggle used many times a day.
    fn peek(self: &Rc<Self>) {
        let raise = !self.raised.get();
        self.raised.set(raise);
        if raise {
            let focused = focused_monitor();
            if let Some(m) = &focused {
                self.side.set_monitor(Some(m));
            }
            self.side.present();
            // Invisible full-screen catchers on every other monitor: a click there closes the sidebar
            let mons = gdk::Display::default().unwrap().monitors();
            for i in 0..mons.n_items() {
                let Some(m) = mons.item(i).and_downcast::<gdk::Monitor>() else { continue };
                if focused.as_ref().is_some_and(|f| f.connector() == m.connector()) {
                    continue;
                }
                let w = layer_window("dashboard-catcher", Layer::Overlay, KeyboardMode::None);
                w.set_exclusive_zone(0);
                w.set_monitor(Some(&m));
                let area = gtk::Box::new(gtk::Orientation::Vertical, 0);
                area.add_css_class("backdrop");
                area.add_css_class("raised");
                area.set_hexpand(true);
                area.set_vexpand(true);
                w.set_child(Some(&area));
                // The click handler goes on the window: an empty box is not a reliable pick target,
                // so presses on the other monitor's blur never reached it.
                let me = self.clone();
                on_click(&w, move || me.close());
                w.present();
                self.catchers.borrow_mut().push(w);
            }
            if let Some(c) = self.connections.borrow().as_ref() {
                c.refresh();
            }
            self.publish_state(true);
        } else {
            self.side.set_visible(false);
            for w in self.catchers.borrow_mut().drain(..) {
                w.destroy();
            }
            self.publish_state(false);
        }
    }

    /// Tell Waybar whether the sidebar is open (the dashboard icon highlights while it is).
    fn publish_state(&self, open: bool) {
        let path = format!("{}/aura-panel-open", std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/tmp".into()));
        let _ = if open { std::fs::write(&path, "1") } else { std::fs::remove_file(&path) };
        sh("pkill -RTMIN+9 waybar");
    }

    fn close(self: &Rc<Self>) {
        if self.raised.get() {
            self.peek();
        }
    }
}

/// SIGUSR1 count, bumped by the signal handler (async-signal-safe) and drained on the GTK thread.
static TOGGLES: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

extern "C" fn on_sigusr1(_: libc::c_int) {
    TOGGLES.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
}

/// SIGUSR2 (top-bar clock click) toggles the calendar.
static CAL_TOGGLES: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

extern "C" fn on_sigusr2(_: libc::c_int) {
    CAL_TOGGLES.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
}

fn main() {
    // Install the Super+D handler before anything else, so a toggle during startup is never lost
    // (SIGUSR1's default action would kill the process).
    unsafe {
        libc::signal(libc::SIGUSR1, on_sigusr1 as *const () as libc::sighandler_t);
        libc::signal(libc::SIGUSR2, on_sigusr2 as *const () as libc::sighandler_t);
    }
    let raise = std::env::args().any(|a| a == "--raised");
    gtk::init().expect("GTK init");
    let p = Panel::build(raise);
    // Toggles that arrived while building (beyond the --raised start) are applied now, then polled.
    let p2 = p.clone();
    glib::timeout_add_local(Duration::from_millis(100), move || {
        let n = TOGGLES.swap(0, std::sync::atomic::Ordering::SeqCst);
        if n % 2 == 1 {
            p2.peek();
        }
        if CAL_TOGGLES.swap(0, std::sync::atomic::Ordering::SeqCst) % 2 == 1 {
            p2.toggle_calendar();
        }
        glib::ControlFlow::Continue
    });
    glib::MainLoop::new(None, false).run();
}
