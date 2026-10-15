//! Network (Ethernet/Wi-Fi), Bluetooth and sound accordions.
use crate::util::*;
use gtk::glib;
use gtk::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;

#[derive(Clone, Default)]
struct Net {
    ssid: String,
    signal: i64,
    secure: bool,
    active: bool,
}

#[derive(Clone, Default)]
struct Dev {
    id: String,
    name: String,
    on: bool,
}

#[derive(Clone, Default)]
struct State {
    eth: Option<(String, bool, String)>, // device, up, connection
    wifi_on: bool,
    nets: Vec<Net>,
    saved: Vec<String>,
    bt_on: bool,
    bt: Vec<Dev>, // id = MAC, on = connected
    sinks: Vec<Dev>,
    sources: Vec<Dev>, // on = default
    vol: Option<f64>,
}

/// nmcli -t escapes ':' inside fields as '\:'
fn split_nmcli(line: &str) -> Vec<String> {
    let mut parts = vec![String::new()];
    let mut esc = false;
    for ch in line.chars() {
        if esc {
            parts.last_mut().unwrap().push(ch);
            esc = false;
        } else if ch == '\\' {
            esc = true;
        } else if ch == ':' {
            parts.push(String::new());
        } else {
            parts.last_mut().unwrap().push(ch);
        }
    }
    parts
}

fn read() -> State {
    let mut s = State::default();
    for line in out("nmcli -t -f DEVICE,TYPE,STATE,CONNECTION device").lines() {
        let p = split_nmcli(line);
        if p.get(1).map(String::as_str) == Some("ethernet") {
            s.eth = Some((p[0].clone(), p.get(2).map(String::as_str) == Some("connected"), p.get(3).cloned().unwrap_or_default()));
            break;
        }
    }
    s.wifi_on = out("nmcli radio wifi").contains("enabled");
    let mut seen = Vec::new();
    for line in out("nmcli -t -f IN-USE,SSID,SIGNAL,SECURITY dev wifi list --rescan no").lines() {
        let p = split_nmcli(line);
        if p.len() < 4 || p[1].is_empty() || seen.contains(&p[1]) {
            continue;
        }
        seen.push(p[1].clone());
        s.nets.push(Net { ssid: p[1].clone(), signal: p[2].parse().unwrap_or(0), secure: !p[3].is_empty(), active: p[0] == "*" });
    }
    s.nets.sort_by_key(|n| (!n.active, -n.signal));
    s.nets.truncate(8);
    s.saved = out("nmcli -t -f NAME,TYPE connection show")
        .lines()
        .filter(|l| l.ends_with(":802-11-wireless"))
        .map(|l| split_nmcli(l)[0].clone())
        .collect();
    s.bt_on = out("bluetoothctl show").contains("Powered: yes");
    for line in out("bluetoothctl devices Paired").lines() {
        let mut it = line.splitn(3, ' ');
        if it.next() == Some("Device") {
            if let (Some(mac), Some(name)) = (it.next(), it.next()) {
                let on = out(&format!("bluetoothctl info {mac}")).contains("Connected: yes");
                s.bt.push(Dev { id: mac.into(), name: name.into(), on });
            }
        }
    }
    // `wpctl status`: Audio → Sinks / Sources sections, "*" marks the default
    let mut section = 0; // 1 sinks, 2 sources
    let mut in_audio = false;
    for line in out("wpctl status").lines() {
        if line.starts_with("Audio") {
            in_audio = true;
            continue;
        }
        if line.starts_with("Video") {
            break;
        }
        if !in_audio {
            continue;
        }
        if line.contains("Sinks:") {
            section = 1;
            continue;
        }
        if line.contains("Sources:") {
            section = 2;
            continue;
        }
        if ["Devices:", "Filters:", "Streams:"].iter().any(|k| line.contains(k)) {
            section = 0;
            continue;
        }
        if section == 0 {
            continue;
        }
        let t = line.trim_start_matches(|c: char| " │├└─".contains(c));
        let default = t.trim_start().starts_with('*');
        let t = t.trim_start().trim_start_matches('*').trim_start();
        if let Some((id, rest)) = t.split_once(". ") {
            if id.chars().all(|c| c.is_ascii_digit()) && !id.is_empty() {
                let name = rest.split(" [vol:").next().unwrap_or(rest).trim().to_string();
                let d = Dev { id: id.into(), name, on: default };
                if section == 1 { s.sinks.push(d) } else { s.sources.push(d) }
            }
        }
    }
    s.vol = out("wpctl get-volume @DEFAULT_AUDIO_SINK@").split_whitespace().nth(1).and_then(|v| v.parse().ok());
    s
}

pub struct Connections {
    pub widget: gtk::Box,
    pub net_card: gtk::Box,
    pub bt_card: gtk::Box,
    pub audio_card: gtk::Box,
    open: RefCell<Option<&'static str>>,
    animate: RefCell<Option<&'static str>>, // section the user just opened (animate once)
    state: RefCell<State>,
    on_close: Box<dyn Fn()>,
}

impl Connections {
    pub fn new(on_close: impl Fn() + 'static) -> Rc<Self> {
        let c = Rc::new(Self {
            widget: gtk::Box::new(gtk::Orientation::Vertical, 12),
            net_card: card(&[]),
            bt_card: card(&[]),
            audio_card: card(&[]),
            open: RefCell::new(None),
            animate: RefCell::new(None),
            state: RefCell::new(State::default()),
            on_close: Box::new(on_close),
        });
        c.widget.append(&c.net_card);
        c.widget.append(&c.bt_card);
        c.refresh();
        let weak = Rc::downgrade(&c);
        glib::timeout_add_seconds_local(5, move || {
            let Some(c) = weak.upgrade() else { return glib::ControlFlow::Break };
            c.refresh();
            glib::ControlFlow::Continue
        });
        c
    }

    pub fn refresh(self: &Rc<Self>) {
        let weak = Rc::downgrade(self);
        bg(read, move |s| {
            if let Some(c) = weak.upgrade() {
                *c.state.borrow_mut() = s;
                c.render();
            }
        });
    }

    fn run(self: &Rc<Self>, cmd: String) {
        let weak = Rc::downgrade(self);
        bg(move || out(&cmd), move |_| {
            if let Some(c) = weak.upgrade() {
                c.refresh();
            }
        });
    }

    fn item(self: &Rc<Self>, b: &gtk::Box, glyph: &str, text: &str, detail: &str, active: bool, f: impl Fn() + 'static) {
        let btn = gtk::Button::new();
        btn.add_css_class("cc-item");
        if active {
            btn.add_css_class("active");
        }
        let row = hbox(10);
        row.append(&label(glyph, &["cc-glyph"]));
        let t = label(text, &[]);
        t.set_hexpand(true);
        row.append(&t);
        if !detail.is_empty() {
            row.append(&rlabel(detail, &["dim"]));
        }
        btn.set_child(Some(&row));
        btn.connect_clicked(move |_| f());
        b.append(&btn);
    }

    /// Collapsible section: header (title · current value, optional switch) + revealed body.
    fn accordion(
        self: &Rc<Self>,
        b: &gtk::Box,
        key: &'static str,
        glyph: &str,
        title: &str,
        summary: &str,
        switch: Option<(bool, Box<dyn Fn(bool)>)>,
    ) -> gtk::Box {
        let open = *self.open.borrow() == Some(key);
        let row = hbox(8);
        row.add_css_class("cc-acc-head");
        row.append(&label(glyph, &["cc-glyph"]));
        row.append(&label(title, &["cc-acc-title"]));
        let s = label(summary, &["dim"]);
        s.set_hexpand(true);
        row.append(&s);
        if let Some((state, f)) = switch {
            let sw = gtk::Switch::new();
            sw.set_active(state);
            sw.set_valign(gtk::Align::Center);
            sw.connect_state_set(move |_, st| {
                f(st);
                glib::Propagation::Proceed
            });
            row.append(&sw);
        }
        row.append(&label(if open { "󰅀" } else { "󰅂" }, &["dim"]));
        let weak = Rc::downgrade(self);
        on_click(&s, move || {
            if let Some(c) = weak.upgrade() {
                let mut o = c.open.borrow_mut();
                *o = if *o == Some(key) { None } else { Some(key) };
                *c.animate.borrow_mut() = *o;
                drop(o);
                c.render();
            }
        });
        b.append(&row);
        let body = gtk::Box::new(gtk::Orientation::Vertical, 2);
        body.add_css_class("cc-acc-body");
        // Rebuilt on every 5 s refresh: restore the open state without motion. Only a section the
        // user just clicked open slides down (180 ms, ease-out), once.
        let rev = gtk::Revealer::new();
        rev.set_transition_type(gtk::RevealerTransitionType::SlideDown);
        rev.set_child(Some(&body));
        if open && self.animate.borrow_mut().take_if(|k| *k == key).is_some() {
            rev.set_transition_duration(180);
            let r = rev.clone();
            glib::idle_add_local_once(move || r.set_reveal_child(true));
        } else {
            rev.set_transition_duration(0);
            rev.set_reveal_child(open);
        }
        b.append(&rev);
        body
    }

    fn render(self: &Rc<Self>) {
        let s = self.state.borrow().clone();

        // Network
        clear(&self.net_card);
        if let Some((dev, up, conn)) = s.eth.clone() {
            let me = self.clone();
            self.item(&self.net_card, "󰈀", "Ethernet", if up { &conn } else { "disconnected" }, up, move || {
                me.run(format!("nmcli device {} {dev}", if up { "disconnect" } else { "connect" }));
            });
        }
        let current = if !s.wifi_on {
            "off".to_string()
        } else {
            s.nets.iter().find(|n| n.active).map(|n| n.ssid.clone()).unwrap_or_else(|| "not connected".into())
        };
        let me = self.clone();
        let body = self.accordion(&self.net_card, "wifi", "󰖩", "Wi-Fi", &current,
            Some((s.wifi_on, Box::new(move |on| me.run(format!("nmcli radio wifi {}", if on { "on" } else { "off" }))))));
        if s.wifi_on {
            for n in &s.nets {
                let bars = match n.signal { s if s > 75 => "󰤨", s if s > 50 => "󰤥", s if s > 25 => "󰤢", _ => "󰤟" };
                let (me, n2, saved) = (self.clone(), n.clone(), s.saved.clone());
                self.item(&body, bars, &n.ssid, if n.secure { "󰌾" } else { "" }, n.active, move || me.connect_wifi(&n2, &saved));
            }
        }

        // Bluetooth
        clear(&self.bt_card);
        let connected: Vec<String> = s.bt.iter().filter(|d| d.on).map(|d| d.name.clone()).collect();
        let summary = if !s.bt_on { "off".into() } else if connected.is_empty() { "no devices connected".into() } else { connected.join(", ") };
        let me = self.clone();
        let body = self.accordion(&self.bt_card, "bt", "󰂯", "Bluetooth", &summary,
            Some((s.bt_on, Box::new(move |on| me.run(format!("bluetoothctl power {}", if on { "on" } else { "off" }))))));
        if s.bt_on {
            if s.bt.is_empty() {
                body.append(&label("No paired devices", &["dim"]));
            }
            for d in &s.bt {
                let (me, d2) = (self.clone(), d.clone());
                self.item(&body, if d.on { "󰂱" } else { "󰂯" }, &d.name, if d.on { "connected" } else { "" }, d.on, move || {
                    me.run(format!("bluetoothctl {} {}", if d2.on { "disconnect" } else { "connect" }, d2.id));
                });
            }
            let me = self.clone();
            self.item(&body, "󰐕", "Pair a new device…", "", false, move || {
                (me.on_close)();
                sh("blueman-manager");
            });
        }

        // Sound: volume always visible; output/input accordions show the current device
        clear(&self.audio_card);
        let vol = hbox(10);
        vol.append(&label("󰕾", &["cc-glyph"]));
        let scale = gtk::Scale::with_range(gtk::Orientation::Horizontal, 0.0, 100.0, 1.0);
        scale.set_draw_value(false);
        scale.set_hexpand(true);
        no_wheel(&scale);
        if let Some(v) = s.vol {
            scale.set_value((v * 100.0).round());
        }
        scale.connect_value_changed(|sc| sh(&format!("wpctl set-volume @DEFAULT_AUDIO_SINK@ {}%", sc.value() as i64)));
        vol.append(&scale);
        self.audio_card.append(&vol);
        for (key, glyph, title, devs) in [("out", "󰓃", "Output", &s.sinks), ("in", "󰍬", "Input", &s.sources)] {
            let cur = devs.iter().find(|d| d.on).map(|d| d.name.clone()).unwrap_or_default();
            let body = self.accordion(&self.audio_card, key, glyph, title, &cur, None);
            for d in devs {
                let (me, id) = (self.clone(), d.id.clone());
                self.item(&body, glyph, &d.name, "", d.on, move || me.run(format!("wpctl set-default {id}")));
            }
        }
    }

    fn connect_wifi(self: &Rc<Self>, n: &Net, saved: &[String]) {
        if n.active {
            return;
        }
        let ssid = n.ssid.replace('\'', "");
        if saved.contains(&n.ssid) || !n.secure {
            self.run(format!("nmcli device wifi connect '{ssid}' || nmcli connection up '{ssid}'"));
        } else {
            // new secured network: ask for the password in a small terminal
            (self.on_close)();
            sh(&format!("ghostty --title='Wi-Fi password' -e sh -c \"nmcli --ask device wifi connect '{ssid}'; sleep 2\""));
        }
    }
}
