//! aura-bar: event-driven Waybar helpers for Hyprland (replaces per-second shell polling).
//!
//!   aura-bar dock <glyph> <class>[,<class>…]   long-running; reprints on Hyprland window events
//!   aura-bar clock                             long-running; reprints on each minute boundary
//!   aura-bar dnd                               prints once (Waybar re-runs it on SIGRTMIN+8)
//!   aura-bar bsdate                            today's Bikram Sambat date, e.g. "२९ असोज २०८३"
//!   aura-bar lockdate                          lock-screen date line (English + BS)
//!   aura-bar panelstate                        dashboard icon state (Waybar re-runs it on SIGRTMIN+9)
//!
//! Output is Waybar's JSON format: {"text", "class", "tooltip"}.
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Read, Write};
use std::os::unix::net::UnixStream;
use std::process::Command;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

fn hypr_dir() -> String {
    format!(
        "{}/hypr/{}",
        std::env::var("XDG_RUNTIME_DIR").unwrap_or_default(),
        std::env::var("HYPRLAND_INSTANCE_SIGNATURE").unwrap_or_default()
    )
}

/// One request on Hyprland's command socket (what `hyprctl -j <cmd>` does, minus the process).
fn hypr_json(cmd: &str) -> Value {
    let Ok(mut s) = UnixStream::connect(format!("{}/.socket.sock", hypr_dir())) else { return Value::Null };
    let _ = s.write_all(format!("j/{cmd}").as_bytes());
    let mut buf = String::new();
    let _ = s.read_to_string(&mut buf);
    serde_json::from_str(&buf).unwrap_or(Value::Null)
}

fn emit(v: &Value) {
    let mut out = std::io::stdout().lock();
    // Waybar gone (pipe closed)? Exit instead of lingering as an orphan.
    if writeln!(out, "{v}").and_then(|_| out.flush()).is_err() {
        std::process::exit(0);
    }
}

// ---------------- dock ----------------

fn dock_status(glyph: &str, classes: &[String], output: &str) -> Value {
    let mon = hypr_json("monitors")
        .as_array()
        .and_then(|ms| ms.iter().find(|m| m["name"] == output))
        .and_then(|m| m["id"].as_i64())
        .unwrap_or(-1);
    let matches = |c: &Value| classes.iter().any(|k| c["class"].as_str().is_some_and(|cl| cl.eq_ignore_ascii_case(k)));
    let n = hypr_json("clients")
        .as_array()
        .map(|cs| {
            cs.iter()
                .filter(|c| matches(c))
                // parked in a special workspace (minimized / closed-to-tray) = not open on screen
                .filter(|c| !c["workspace"]["name"].as_str().is_some_and(|w| w.starts_with("special:")))
                .filter(|c| mon == -1 || c["monitor"].as_i64() == Some(mon))
                .count()
        })
        .unwrap_or(0);
    let focused = matches(&hypr_json("activewindow"));
    if n > 0 {
        let class = if focused { json!(["running", "focused"]) } else { json!("running") };
        json!({"text": glyph, "class": class, "tooltip": format!("{n} window(s)")})
    } else {
        json!({"text": glyph, "class": "idle"})
    }
}

/// Window lifecycle/placement events that can change a dock indicator.
fn relevant(event: &str) -> bool {
    let name = event.split(">>").next().unwrap_or("");
    matches!(name, "openwindow" | "closewindow" | "movewindow" | "movewindowv2" | "activewindowv2" | "monitoradded" | "monitorremoved")
}

fn dock(glyph: &str, classes: &str) {
    let classes: Vec<String> = classes.split(',').map(String::from).collect();
    // Waybar tells each module instance which output (monitor) its bar is on
    let output = std::env::var("WAYBAR_OUTPUT_NAME").unwrap_or_default();
    let mut last = dock_status(glyph, &classes, &output);
    emit(&last);
    loop {
        let Ok(stream) = UnixStream::connect(format!("{}/.socket2.sock", hypr_dir())) else {
            std::thread::sleep(Duration::from_secs(2));
            continue;
        };
        for line in BufReader::new(stream).lines().map_while(Result::ok) {
            if !relevant(&line) {
                continue;
            }
            // windows can be mid-teardown when the event fires; settle briefly before reading
            std::thread::sleep(Duration::from_millis(60));
            let now = dock_status(glyph, &classes, &output);
            if now != last {
                emit(&now);
                last = now;
            }
        }
        std::thread::sleep(Duration::from_secs(1)); // Hyprland restarted: reconnect
    }
}

// ---------------- dates (local time via libc; Bikram Sambat from an embedded table) ----------------

/// Days per month for BS 1975–2100 (from the nepali-datetime project's calendar_bs.csv).
const CALENDAR_BS: &str = include_str!("../data/calendar_bs.csv");
const BS_MONTHS: [&str; 12] = ["बैशाख", "जेठ", "असार", "साउन", "भदौ", "असोज", "कात्तिक", "मंसिर", "पुस", "माघ", "फागुन", "चैत"];

fn local_tm() -> libc::tm {
    unsafe {
        let t = libc::time(std::ptr::null_mut());
        let mut tm: libc::tm = std::mem::zeroed();
        libc::localtime_r(&t, &mut tm);
        tm
    }
}

fn strftime(tm: &libc::tm, fmt: &str) -> String {
    let mut buf = [0u8; 128];
    let f = std::ffi::CString::new(fmt).unwrap();
    let n = unsafe { libc::strftime(buf.as_mut_ptr() as *mut libc::c_char, buf.len(), f.as_ptr(), tm) };
    String::from_utf8_lossy(&buf[..n]).into_owned()
}

/// Days since 1970-01-01 for a proleptic Gregorian date (Howard Hinnant's algorithm).
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
}

/// Inverse of days_from_civil (Hinnant), used by the self-check.
fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (yoe + era * 400 + if m <= 2 { 1 } else { 0 }, m, d)
}

fn devanagari_digits(s: &str) -> String {
    s.chars()
        .map(|c| c.to_digit(10).and_then(|d| char::from_u32(0x0966 + d)).unwrap_or(c))
        .collect()
}

/// BS date for a Gregorian date: BS 1975/01/01 is AD 1918-04-13; walk the month table forward.
fn bs_from_ad(y: i64, m: i64, d: i64) -> Option<(i64, usize, i64)> {
    let mut left = days_from_civil(y, m, d) - days_from_civil(1918, 4, 13);
    if left < 0 {
        return None;
    }
    for row in CALENDAR_BS.lines().skip(1) {
        let cols: Vec<i64> = row.split(',').filter_map(|c| c.trim().parse().ok()).collect();
        let (year, months) = (cols[0], &cols[1..]);
        for (i, len) in months.iter().enumerate() {
            if left < *len {
                return Some((year, i, left + 1));
            }
            left -= len;
        }
    }
    None
}

fn bs_today() -> String {
    let tm = local_tm();
    match bs_from_ad(tm.tm_year as i64 + 1900, tm.tm_mon as i64 + 1, tm.tm_mday as i64) {
        Some((y, m, d)) => devanagari_digits(&format!("{d} {} {y}", BS_MONTHS[m])),
        None => String::new(),
    }
}

fn sh(cmd: &str) -> String {
    Command::new("sh")
        .args(["-c", cmd])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default()
}

// ---------------- clock ----------------

fn clock() {
    loop {
        let tm = local_tm();
        let bs = bs_today();
        // English date · weekday · Nepali (BS) date, then the time
        let text = format!(
            "<span color='#a277ff'>{}</span>  <span color='#6d6d6d'>·</span>  <span color='#61ffca' weight='bold'>{}</span>  <span color='#6d6d6d'>·</span>  <span color='#a277ff'>{bs}</span>   <b>{}</b>",
            strftime(&tm, "%-d %b %Y"),
            strftime(&tm, "%A"),
            strftime(&tm, "%H:%M")
        );
        emit(&json!({"text": text, "tooltip": format!("{} · {bs}", strftime(&tm, "%A, %-d %B %Y"))}));
        // sleep to the next minute boundary
        let secs = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs_f64()).unwrap_or(0.0);
        std::thread::sleep(Duration::from_secs_f64(60.0 - secs % 60.0 + 0.05));
    }
}

// ---------------- do not disturb ----------------

fn dnd() {
    let on = sh("makoctl mode").split_whitespace().any(|m| m == "do-not-disturb");
    emit(&if on {
        json!({"text": "󰂛", "class": "on", "tooltip": "Do Not Disturb: On — click to disable"})
    } else {
        json!({"text": "󰂚", "class": "off", "tooltip": "Do Not Disturb: Off — click to enable"})
    });
}

fn main() {
    // Die with the parent Waybar, so a Waybar restart never leaves event-driven helpers behind.
    unsafe {
        libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGTERM);
        if libc::getppid() == 1 {
            std::process::exit(0); // parent already gone before we got here
        }
    }
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("dock") if args.len() == 3 => dock(&args[1], &args[2]),
        Some("clock") => clock(),
        Some("dnd") => dnd(),
        Some("bsdate") => println!("{}", bs_today()),
        Some("panelstate") => {
            let open = std::path::Path::new(&format!("{}/aura-panel-open", std::env::var("XDG_RUNTIME_DIR").unwrap_or_default())).exists();
            emit(&json!({"text": "󰕮", "class": if open { "open" } else { "closed" },
                         "tooltip": if open { "Dashboard is open — click or Super+D to close" } else { "Dashboard (Super+D)" }}));
        }
        // hidden self-check: `bscheck <days>` prints "AD -> BS" (y m d) for each day from 1918-04-13
        Some("bscheck") => {
            let n: i64 = args.get(1).and_then(|v| v.parse().ok()).unwrap_or(1);
            let base = days_from_civil(1918, 4, 13);
            for i in 0..n {
                let (y, m, d) = civil_from_days(base + i);
                if let Some((by, bm, bd)) = bs_from_ad(y, m, d) {
                    println!("{y:04}-{m:02}-{d:02} {by} {} {bd}", bm + 1);
                }
            }
        }
        Some("lockdate") => println!("{}  ·  {}  ·  {}", strftime(&local_tm(), "%-d %B"), strftime(&local_tm(), "%A"), bs_today()),
        _ => {
            eprintln!("usage: aura-bar dock <glyph> <class[,class…]> | clock | dnd | bsdate | lockdate");
            std::process::exit(2);
        }
    }
}
