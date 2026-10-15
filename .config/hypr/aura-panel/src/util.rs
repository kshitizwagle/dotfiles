//! Small helpers shared by the panel: shell commands, background work, widget builders.
use gtk::prelude::*;
use gtk::{gdk, gio, glib};
use std::process::{Command, Stdio};

pub const STATUS_API: &str = "http://127.0.0.1:8000";

/// Fire-and-forget a shell command, detached from the panel.
pub fn sh(cmd: &str) {
    let _ = Command::new("setsid")
        .args(["-f", "sh", "-c", cmd])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn();
}

/// Run a shell command and return its trimmed stdout (bounded so a stuck tool can't hang a worker).
pub fn out(cmd: &str) -> String {
    Command::new("timeout")
        .args(["4", "sh", "-c", cmd])
        .stderr(Stdio::null())
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default()
}

/// GET/POST the status page API (curl keeps the binary small; no HTTP stack linked in).
pub fn api(path: &str, body: Option<&serde_json::Value>) -> Option<serde_json::Value> {
    let url = format!("{STATUS_API}{path}");
    let mut c = Command::new("curl");
    c.args(["-s", "-m", "20", &url]);
    if let Some(b) = body {
        c.args(["-H", "Content-Type: application/json", "-d", &b.to_string()]);
    }
    let o = c.stderr(Stdio::null()).output().ok()?;
    serde_json::from_slice(&o.stdout).ok()
}

/// Run `work` on a worker thread, then hand its result to `done` on the GTK thread.
pub fn bg<T: Send + 'static>(work: impl FnOnce() -> T + Send + 'static, done: impl FnOnce(T) + 'static) {
    glib::spawn_future_local(async move {
        if let Ok(v) = gio::spawn_blocking(work).await {
            done(v);
        }
    });
}

pub fn human(mut n: f64) -> String {
    for unit in ["B", "K", "M", "G", "T"] {
        if n < 1024.0 {
            return if matches!(unit, "B" | "K" | "M") { format!("{n:.0}{unit}") } else { format!("{n:.1}{unit}") };
        }
        n /= 1024.0;
    }
    format!("{n:.1}P")
}

pub fn label(text: &str, classes: &[&str]) -> gtk::Label {
    let l = gtk::Label::new(Some(text));
    l.set_xalign(0.0);
    l.set_ellipsize(gtk::pango::EllipsizeMode::End);
    for c in classes {
        l.add_css_class(c);
    }
    l
}

pub fn rlabel(text: &str, classes: &[&str]) -> gtk::Label {
    let l = label(text, classes);
    l.set_xalign(1.0);
    l
}

pub fn card(classes: &[&str]) -> gtk::Box {
    let b = gtk::Box::new(gtk::Orientation::Vertical, 10);
    b.add_css_class("card");
    for c in classes {
        b.add_css_class(c);
    }
    b
}

pub fn hbox(spacing: i32) -> gtk::Box {
    gtk::Box::new(gtk::Orientation::Horizontal, spacing)
}

pub fn icon_button(glyph: &str, tip: &str, classes: &[&str], on_click: impl Fn() + 'static) -> gtk::Button {
    let b = gtk::Button::with_label(glyph);
    b.set_tooltip_text(Some(tip));
    b.add_css_class("icon-btn");
    for c in classes {
        b.add_css_class(c);
    }
    b.connect_clicked(move |_| on_click());
    b
}

pub fn clear(b: &gtk::Box) {
    while let Some(c) = b.first_child() {
        b.remove(&c);
    }
}

/// Sliders ignore the mouse wheel, so scrolling the panel never nudges volume/brightness.
pub fn no_wheel(w: &impl IsA<gtk::Widget>) {
    let sc = gtk::EventControllerScroll::new(gtk::EventControllerScrollFlags::BOTH_AXES);
    sc.set_propagation_phase(gtk::PropagationPhase::Capture);
    sc.connect_scroll(|_, _, _| glib::Propagation::Stop);
    w.add_controller(sc);
}

pub fn on_click(w: &impl IsA<gtk::Widget>, f: impl Fn() + 'static) {
    let g = gtk::GestureClick::new();
    g.connect_pressed(move |_, _, _, _| f());
    w.add_controller(g);
}

/// GDK monitor by Hyprland connector name ("DP-1"), or the focused one.
pub fn monitor_named(name: &str) -> Option<gdk::Monitor> {
    let mons = gdk::Display::default()?.monitors();
    (0..mons.n_items())
        .filter_map(|i| mons.item(i).and_downcast::<gdk::Monitor>())
        .find(|m| m.connector().as_deref() == Some(name))
}

pub fn focused_connector() -> Option<String> {
    let v: serde_json::Value = serde_json::from_str(&out("hyprctl monitors -j")).ok()?;
    v.as_array()?
        .iter()
        .find(|m| m["focused"].as_bool() == Some(true))
        .and_then(|m| m["name"].as_str().map(String::from))
}

pub fn focused_monitor() -> Option<gdk::Monitor> {
    focused_connector().and_then(|n| monitor_named(&n)).or_else(|| {
        let mons = gdk::Display::default()?.monitors();
        mons.item(0).and_downcast::<gdk::Monitor>()
    })
}

/// Host port that most likely serves a web UI; ports also published over UDP are peer ports.
pub fn web_port(ports: &[serde_json::Value]) -> Option<String> {
    let host = |p: &str| p.split(':').nth(1).unwrap_or("").split(['→', '-']).next().unwrap_or("").to_string();
    let published: Vec<&str> = ports.iter().filter_map(|p| p.as_str()).filter(|p| p.starts_with("0.0.0.0:")).collect();
    let udp: Vec<String> = published.iter().filter(|p| p.ends_with("/udp")).map(|p| host(p)).collect();
    published
        .iter()
        .filter(|p| p.ends_with("/tcp"))
        .map(|p| host(p))
        .find(|h| !udp.contains(h))
}

pub fn home() -> String {
    std::env::var("HOME").unwrap_or_default()
}
