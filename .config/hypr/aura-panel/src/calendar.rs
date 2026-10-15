//! Month calendar popover (top bar clock): Gregorian and Bikram Sambat (Nepali) month grids,
//! prev/next navigation, today highlighted, and the other calendar's day under every date.
//! The BS table is the same one aura-bar uses (verified against nepali-datetime for 1975–2100 BS).
use gtk::prelude::*;
use std::cell::Cell;
use std::rc::Rc;

const CALENDAR_BS: &str = include_str!("../../aura-bar/data/calendar_bs.csv");
pub const BS_MONTHS: [&str; 12] = ["बैशाख", "जेठ", "असार", "साउन", "भदौ", "असोज", "कात्तिक", "मंसिर", "पुस", "माघ", "फागुन", "चैत"];
const BS_DAYS: [&str; 7] = ["आइत", "सोम", "मंगल", "बुध", "बिही", "शुक्र", "शनि"];
const AD_MONTHS: [&str; 12] = ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"];
const AD_DAYS: [&str; 7] = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
const BS_EPOCH_AD: (i64, i64, i64) = (1918, 4, 13); // BS 1975-01-01

/// Days since 1970-01-01 (Howard Hinnant's civil algorithms).
pub fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    era * 146097 + yoe * 365 + yoe / 4 - yoe / 100 + doy - 719468
}

pub fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (yoe + era * 400 + i64::from(m <= 2), m, d)
}

/// Weekday of a day number: 0 = Sunday.
fn weekday(days: i64) -> i64 {
    (days + 4).rem_euclid(7) // 1970-01-01 was a Thursday
}

fn bs_table() -> Vec<(i64, Vec<i64>)> {
    CALENDAR_BS
        .lines()
        .skip(1)
        .filter_map(|r| {
            let c: Vec<i64> = r.split(',').filter_map(|x| x.trim().parse().ok()).collect();
            (c.len() == 13).then(|| (c[0], c[1..].to_vec()))
        })
        .collect()
}

/// Day number of BS year/month/day 1 and that month's length.
fn bs_month_start(year: i64, month: usize) -> Option<(i64, i64)> {
    let mut day = days_from_civil(BS_EPOCH_AD.0, BS_EPOCH_AD.1, BS_EPOCH_AD.2);
    for (y, months) in bs_table() {
        for (i, len) in months.iter().enumerate() {
            if y == year && i == month {
                return Some((day, *len));
            }
            day += len;
        }
    }
    None
}

/// BS (year, month index, day) for a day number.
pub fn bs_from_days(days: i64) -> Option<(i64, usize, i64)> {
    let mut left = days - days_from_civil(BS_EPOCH_AD.0, BS_EPOCH_AD.1, BS_EPOCH_AD.2);
    if left < 0 {
        return None;
    }
    for (y, months) in bs_table() {
        for (i, len) in months.iter().enumerate() {
            if left < *len {
                return Some((y, i, left + 1));
            }
            left -= len;
        }
    }
    None
}

pub fn deva(n: i64) -> String {
    n.to_string().chars().map(|c| c.to_digit(10).and_then(|d| char::from_u32(0x0966 + d)).unwrap_or(c)).collect()
}

fn today_days() -> i64 {
    let t = unsafe { libc::time(std::ptr::null_mut()) };
    let mut tm: libc::tm = unsafe { std::mem::zeroed() };
    unsafe { libc::localtime_r(&t, &mut tm) };
    days_from_civil(tm.tm_year as i64 + 1900, tm.tm_mon as i64 + 1, tm.tm_mday as i64)
}

pub struct Calendar {
    pub widget: gtk::Box,
    nepali: Cell<bool>,
    // month shown: AD (year, month 1-12) and BS (year, month index 0-11)
    ad: Cell<(i64, i64)>,
    bs: Cell<(i64, usize)>,
    title: gtk::Label,
    grid: gtk::Grid,
    mode_ad: gtk::ToggleButton,
    mode_bs: gtk::ToggleButton,
}

impl Calendar {
    pub fn new() -> Rc<Self> {
        let widget = gtk::Box::new(gtk::Orientation::Vertical, 10);
        widget.add_css_class("panel");
        widget.add_css_class("calendar");
        widget.set_size_request(380, -1);
        let mode_ad = gtk::ToggleButton::with_label("Gregorian");
        let mode_bs = gtk::ToggleButton::with_label("नेपाली");
        mode_bs.set_group(Some(&mode_ad));
        mode_ad.set_active(true);
        for b in [&mode_ad, &mode_bs] {
            b.add_css_class("cal-mode");
        }
        let c = Rc::new(Self {
            widget,
            nepali: Cell::new(false),
            ad: Cell::new((0, 0)),
            bs: Cell::new((0, 0)),
            title: gtk::Label::new(None),
            grid: gtk::Grid::new(),
            mode_ad,
            mode_bs,
        });
        let modes = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        modes.add_css_class("cal-modes");
        modes.set_homogeneous(true);
        modes.append(&c.mode_ad);
        modes.append(&c.mode_bs);
        c.widget.append(&modes);

        let head = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        let prev = gtk::Button::with_label("󰅁");
        let next = gtk::Button::with_label("󰅂");
        let today = gtk::Button::with_label("Today");
        for b in [&prev, &next, &today] {
            b.add_css_class("cal-nav");
        }
        c.title.add_css_class("cal-title");
        c.title.set_hexpand(true);
        c.title.set_xalign(0.0);
        head.append(&c.title);
        head.append(&today);
        head.append(&prev);
        head.append(&next);
        c.widget.append(&head);

        c.grid.set_column_homogeneous(true);
        c.grid.set_row_spacing(2);
        c.grid.set_column_spacing(2);
        c.widget.append(&c.grid);

        let me = c.clone();
        prev.connect_clicked(move |_| me.step(-1));
        let me = c.clone();
        next.connect_clicked(move |_| me.step(1));
        let me = c.clone();
        today.connect_clicked(move |_| me.reset());
        let me = c.clone();
        c.mode_bs.connect_toggled(move |b| {
            me.nepali.set(b.is_active());
            me.render();
        });
        c.reset();
        c
    }

    /// Jump to the current month (called each time the popover opens).
    pub fn reset(&self) {
        let t = today_days();
        let (y, m, _) = civil_from_days(t);
        self.ad.set((y, m));
        if let Some((by, bm, _)) = bs_from_days(t) {
            self.bs.set((by, bm));
        }
        self.render();
    }

    fn step(&self, delta: i64) {
        if self.nepali.get() {
            let (y, m) = self.bs.get();
            let idx = y * 12 + m as i64 + delta;
            let (ny, nm) = (idx.div_euclid(12), idx.rem_euclid(12) as usize);
            if bs_month_start(ny, nm).is_some() {
                self.bs.set((ny, nm));
            }
        } else {
            let (y, m) = self.ad.get();
            let idx = y * 12 + (m - 1) + delta;
            self.ad.set((idx.div_euclid(12), idx.rem_euclid(12) + 1));
        }
        self.render();
    }

    fn render(&self) {
        while let Some(ch) = self.grid.first_child() {
            self.grid.remove(&ch);
        }
        let today = today_days();
        let nepali = self.nepali.get();
        // first day number of the month and its length
        let (start, len) = if nepali {
            let (y, m) = self.bs.get();
            self.title.set_text(&format!("{} {}", BS_MONTHS[m], deva(y)));
            match bs_month_start(y, m) {
                Some(v) => v,
                None => return,
            }
        } else {
            let (y, m) = self.ad.get();
            self.title.set_text(&format!("{} {y}", AD_MONTHS[(m - 1) as usize]));
            let start = days_from_civil(y, m, 1);
            let (ny, nm) = if m == 12 { (y + 1, 1) } else { (y, m + 1) };
            (start, days_from_civil(ny, nm, 1) - start)
        };
        for (i, d) in (if nepali { BS_DAYS } else { AD_DAYS }).iter().enumerate() {
            let l = gtk::Label::new(Some(d));
            l.add_css_class("cal-dow");
            if i == 6 {
                l.add_css_class("cal-sat"); // Saturday is the weekly holiday in Nepal
            }
            self.grid.attach(&l, i as i32, 0, 1, 1);
        }
        let lead = weekday(start);
        for i in 0..len {
            let day = start + i;
            let cell_idx = lead + i;
            let (col, row) = ((cell_idx % 7) as i32, (cell_idx / 7) as i32 + 1);
            let cell = gtk::Box::new(gtk::Orientation::Vertical, 0);
            cell.add_css_class("cal-day");
            if day == today {
                cell.add_css_class("cal-today");
            }
            if col == 6 {
                cell.add_css_class("cal-sat");
            }
            let (main, sub) = if nepali {
                let (_, _, ad_d) = civil_from_days(day);
                (deva(i + 1), ad_d.to_string())
            } else {
                let sub = bs_from_days(day).map(|(_, _, d)| deva(d)).unwrap_or_default();
                ((i + 1).to_string(), sub)
            };
            let m = gtk::Label::new(Some(&main));
            m.add_css_class("cal-num");
            let s = gtk::Label::new(Some(&sub));
            s.add_css_class("cal-sub");
            cell.append(&m);
            cell.append(&s);
            cell.set_tooltip_text(Some(&{
                let (y, mo, d) = civil_from_days(day);
                let bs = bs_from_days(day).map(|(by, bm, bd)| format!("{} {} {}", deva(bd), BS_MONTHS[bm], deva(by))).unwrap_or_default();
                format!("{d} {} {y} · {bs}", AD_MONTHS[(mo - 1) as usize])
            }));
            self.grid.attach(&cell, col, row, 1, 1);
        }
    }
}

