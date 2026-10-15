//! CPU and GPU identification and GPU stats, for any mix of AMD, NVIDIA and Intel.
//!
//! GPUs are found from DRM cards in sysfs (no vendor-specific file required), split into discrete
//! and integrated, and read through the vendor's own interface:
//!   AMD (amdgpu)    gpu_busy_percent, mem_info_vram_*, pp_dpm_sclk/mclk
//!   NVIDIA          nvidia-smi (proprietary driver); nouveau has no load counters, so only the name shows
//!   Intel (i915/xe) no load counter in sysfs: the actual/max GT clock ratio stands in for load
//! Every file is optional, so a missing driver feature hides that value instead of failing.

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::Path;
use std::time::{Duration, Instant};

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Vendor {
    Amd,
    Nvidia,
    Intel,
}

#[derive(Clone, Debug)]
pub struct Gpu {
    /// PCI device dir, e.g. /sys/class/drm/card1/device
    pub dev: String,
    /// DRM card dir, e.g. /sys/class/drm/card1 (i915 keeps its GT clocks here)
    pub card: String,
    pub vendor: Vendor,
    /// PCI address, e.g. 0000:03:00.0 (how nvidia-smi picks a card)
    pub bus: String,
    pub name: String,
    pub discrete: bool,
    pub vram_total: u64,
    /// "GPU", "GPU 2", "iGPU", ...
    pub title: String,
}

#[derive(Clone, Copy, Default, Debug)]
pub struct GpuStats {
    /// 0..1; None when the driver has no way to tell
    pub busy: Option<f64>,
    pub vram_used: Option<u64>,
    pub vram_total: Option<u64>,
    pub core_mhz: Option<u64>,
    pub mem_mhz: Option<u64>,
}

/// AMD APUs carve out 512 MiB–2 GiB by default; anything with at least this much VRAM is a card.
const AMD_DISCRETE_VRAM: u64 = 3 << 30;

fn read(path: impl AsRef<Path>) -> Option<String> {
    std::fs::read_to_string(path).ok().map(|s| s.trim().to_string()).filter(|s| !s.is_empty())
}
fn read_u64(path: impl AsRef<Path>) -> Option<u64> {
    read(path)?.parse().ok()
}
fn hex_id(dev: &str, f: &str) -> String {
    read(format!("{dev}/{f}")).unwrap_or_default().trim_start_matches("0x").to_lowercase()
}

// ---------------------------------------------------------------- CPU

/// The CPU model, shortened. x86 has "model name" in /proc/cpuinfo; ARM and RISC-V usually don't,
/// so fall back to lscpu (which decodes ARM implementer/part IDs, e.g. "Cortex-A76"), then the
/// board name some kernels put in cpuinfo or the device tree.
pub fn cpu_name() -> String {
    let info = read("/proc/cpuinfo").unwrap_or_default();
    let field = |key: &str| info.lines().find_map(|l| {
        let (k, v) = l.split_once(':')?;
        (k.trim() == key).then(|| v.trim().to_string()).filter(|v| !v.is_empty())
    });
    let lscpu = || {
        let out = std::process::Command::new("lscpu").env("LC_ALL", "C").output().ok()?;
        let text = String::from_utf8_lossy(&out.stdout).to_string();
        // several "Model name:" lines on big.LITTLE (one per core type): join them, "Cortex-A76 + Cortex-A55"
        let names: Vec<String> = text.lines()
            .filter_map(|l| l.trim().strip_prefix("Model name:").map(|v| v.trim().to_string()))
            .filter(|v| !v.is_empty() && v != "-").collect();
        (!names.is_empty()).then(|| names.join(" + "))
    };
    let full = field("model name")
        .or_else(lscpu)
        .or_else(|| field("Hardware"))
        .or_else(|| field("Model"))
        .or_else(|| field("uarch"))
        .or_else(|| read("/sys/firmware/devicetree/base/model").map(|m| m.trim_end_matches('\0').to_string()))
        .unwrap_or_else(|| std::env::consts::ARCH.to_string());
    shorten_cpu(&full)
}

/// "AMD Ryzen 7 7700X 8-Core Processor" -> "Ryzen 7 7700X",
/// "13th Gen Intel(R) Core(TM) i7-13700K" -> "Core i7-13700K",
/// "Intel(R) Core(TM) i7-8700 CPU @ 3.20GHz" -> "Core i7-8700".
pub fn shorten_cpu(full: &str) -> String {
    let mut n = full.replace("(R)", "").replace("(r)", "").replace("(TM)", "").replace("(tm)", "");
    // tails: clock speed, "w/ Radeon 780M Graphics", "with Radeon Graphics", "8-Core Processor"
    for cut in [" @", " w/ ", " with ", " Processor", " processor"] {
        if let Some(i) = n.find(cut) { n.truncate(i); }
    }
    let words: Vec<&str> = n.split_whitespace()
        .filter(|w| {
            // "8-Core", "64-Cores", "32-core"; keeps "Eight-Core" on old FX parts where it's the only hint
            let core_count = w.split_once('-').is_some_and(|(n, c)| n.chars().all(|ch| ch.is_ascii_digit())
                && c.to_lowercase().trim_end_matches('s') == "core");
            !core_count && *w != "CPU"
        })
        .collect();
    // drop the vendor and Intel's "13th Gen" prefix; keep "Core" so "Core i7" / "Core Ultra 7" still read right
    let mut i = 0;
    while i < words.len() {
        let w = words[i];
        if w == "AMD" || w == "Intel" { i += 1; }
        else if i + 1 < words.len() && words[i + 1] == "Gen" && w.chars().next().is_some_and(|c| c.is_ascii_digit()) { i += 2; }
        else { break; }
    }
    let out = words[i..].join(" ");
    if out.is_empty() { full.trim().to_string() } else { out }
}

// ---------------------------------------------------------------- GPU discovery

/// Every GPU under `<sys>/class/drm`, discrete first (largest VRAM first), then integrated,
/// each with its tile title. `sys` is "/sys" except in tests.
pub fn discover(sys: &str) -> Vec<Gpu> {
    let drm = format!("{sys}/class/drm");
    let mut cards: Vec<String> = std::fs::read_dir(&drm).into_iter().flatten().flatten()
        .map(|e| e.file_name().to_string_lossy().to_string())
        // card0, card1 ... (connectors are card0-DP-1 etc.; render nodes are renderD128)
        .filter(|n| n.strip_prefix("card").is_some_and(|r| !r.is_empty() && r.chars().all(|c| c.is_ascii_digit())))
        .collect();
    cards.sort_by_key(|n| n[4..].parse::<u32>().unwrap_or(0));
    let mut gpus: Vec<Gpu> = cards.into_iter().filter_map(|n| probe(&format!("{drm}/{n}"))).collect();
    // one physical GPU can register two cards (e.g. simpledrm handover leftovers): keep one per PCI address
    let mut seen = std::collections::HashSet::new();
    gpus.retain(|g| seen.insert(g.bus.clone()));
    gpus.sort_by(|a, b| b.discrete.cmp(&a.discrete).then(b.vram_total.cmp(&a.vram_total)));
    let (nd, ni) = (gpus.iter().filter(|g| g.discrete).count(), gpus.iter().filter(|g| !g.discrete).count());
    let (mut d, mut i) = (0, 0);
    for g in &mut gpus {
        g.title = if g.discrete {
            d += 1;
            if nd > 1 { format!("GPU {d}") } else { "GPU".into() }
        } else {
            i += 1;
            if ni > 1 { format!("iGPU {i}") } else { "iGPU".into() }
        };
    }
    gpus
}

fn probe(card: &str) -> Option<Gpu> {
    let dev = format!("{card}/device");
    // display controllers only (PCI class 0x03xxxx); skips virtual and USB display devices
    if !hex_id(&dev, "class").starts_with("03") { return None; }
    let vendor = match hex_id(&dev, "vendor").as_str() {
        "1002" => Vendor::Amd,
        "10de" => Vendor::Nvidia,
        "8086" => Vendor::Intel,
        _ => return None, // VM adapters (virtio, QXL, VMware, ...) have nothing worth graphing
    };
    let bus = read(format!("{dev}/uevent")).unwrap_or_default().lines()
        .find_map(|l| l.strip_prefix("PCI_SLOT_NAME=").map(str::to_string))
        .unwrap_or_else(|| card.to_string());
    let mut g = Gpu { dev, card: card.to_string(), vendor, bus, name: String::new(), discrete: false, vram_total: 0, title: String::new() };
    g.name = gpu_name(&g);
    let st = stats(&g);
    g.vram_total = st.vram_total.unwrap_or(0);
    g.discrete = match vendor {
        Vendor::Nvidia => true, // NVIDIA has never shipped an x86 iGPU
        Vendor::Amd => g.vram_total >= AMD_DISCRETE_VRAM,
        // Arc A/B-series cards; "Arc Graphics" is the Core Ultra iGPU
        Vendor::Intel => g.name.split_whitespace().collect::<Vec<_>>().windows(2)
            .any(|w| w[0] == "Arc" && w[1].len() > 1 && (w[1].starts_with('A') || w[1].starts_with('B')) && w[1][1..].chars().all(|c| c.is_ascii_digit())),
    };
    Some(g)
}

// ---------------------------------------------------------------- GPU names

/// Marketing name: nvidia-smi for NVIDIA, libdrm's revision-aware table for AMD, else pci.ids.
pub fn gpu_name(g: &Gpu) -> String {
    let (ven, devid) = (hex_id(&g.dev, "vendor"), hex_id(&g.dev, "device"));
    match g.vendor {
        Vendor::Nvidia => {
            if let Some(n) = nvidia_query(&g.bus, "name") {
                return n.trim_start_matches("NVIDIA ").to_string();
            }
        }
        Vendor::Amd => {
            // keyed by device *and* revision, so a 9070 XT (C0) isn't shown as a 9070 (C3);
            // its generic "AMD Radeon Graphics" (most APUs) loses to the pci.ids codename
            let rev = hex_id(&g.dev, "revision");
            let ids = read("/usr/share/libdrm/amdgpu.ids").unwrap_or_default();
            let hit = ids.lines().filter(|l| !l.starts_with('#')).find_map(|l| {
                let mut f = l.splitn(3, ',').map(str::trim);
                let (d, r, name) = (f.next()?, f.next()?, f.next()?);
                (d.eq_ignore_ascii_case(&devid) && r.eq_ignore_ascii_case(&rev)).then(|| name.to_string())
            });
            if let Some(n) = hit.filter(|n| n != "AMD Radeon Graphics") {
                return n.trim_start_matches("AMD ").to_string();
            }
        }
        Vendor::Intel => {}
    }
    pci_ids_name(&ven, &devid).unwrap_or_else(|| match g.vendor {
        Vendor::Amd => "AMD GPU".into(),
        Vendor::Nvidia => "NVIDIA GPU".into(),
        Vendor::Intel => "Intel Graphics".into(),
    })
}

/// "Navi 48 [Radeon RX 9070/9070 XT/9070 GRE]" -> "Radeon RX 9070", "AD103 [GeForce RTX 4080]" -> "GeForce RTX 4080",
/// "Raptor Lake-S GT1 [UHD Graphics 770]" -> "UHD Graphics 770", "Raphael" -> "Raphael".
fn pci_ids_name(ven: &str, devid: &str) -> Option<String> {
    let db = ["/usr/share/hwdata/pci.ids", "/usr/share/misc/pci.ids"].iter().find_map(read)?;
    let mut in_vendor = false;
    for l in db.lines() {
        if !l.starts_with('\t') && !l.starts_with('#') && !l.is_empty() {
            in_vendor = l.starts_with(&format!("{ven} "));
        } else if in_vendor && !l.starts_with("\t\t") {
            if let Some(name) = l.trim_start().strip_prefix(&format!("{devid} ")) {
                return Some(shorten_pci_name(name));
            }
        }
    }
    None
}

pub fn shorten_pci_name(name: &str) -> String {
    let name = name.trim();
    // prefer the marketing name in brackets, first variant only
    let inner = name.rfind('[').and_then(|i| name[i + 1..].split_once(']')).map(|(m, _)| m);
    inner.map(|m| m.split('/').next().unwrap_or(m)).unwrap_or(name).trim().to_string()
}

// ---------------------------------------------------------------- GPU stats

/// Current load, VRAM and clocks. Values the driver doesn't expose are None.
pub fn stats(g: &Gpu) -> GpuStats {
    match g.vendor {
        Vendor::Amd => {
            let dpm = |f: &str| read(format!("{}/{f}", g.dev)).and_then(|t| {
                // active state is starred: "1: 871Mhz *"
                let l = t.lines().find(|l| l.trim_end().ends_with('*'))?;
                l.split_whitespace().nth(1)?.trim_end_matches(|c: char| !c.is_ascii_digit()).parse().ok()
            });
            GpuStats {
                busy: read_u64(format!("{}/gpu_busy_percent", g.dev)).map(|b| b as f64 / 100.0),
                vram_used: read_u64(format!("{}/mem_info_vram_used", g.dev)),
                vram_total: read_u64(format!("{}/mem_info_vram_total", g.dev)),
                core_mhz: dpm("pp_dpm_sclk"),
                mem_mhz: dpm("pp_dpm_mclk"),
            }
        }
        Vendor::Nvidia => nvidia_stats(&g.bus),
        Vendor::Intel => {
            // i915: card-level gt_*_freq_mhz; xe: per-GT freq files under the device
            let (act, max) = [
                (format!("{}/gt_act_freq_mhz", g.card), format!("{}/gt_max_freq_mhz", g.card)),
                (format!("{}/tile0/gt0/freq0/act_freq", g.dev), format!("{}/tile0/gt0/freq0/max_freq", g.dev)),
            ].into_iter().find_map(|(a, m)| Some((read_u64(a)?, read_u64(m)?))).unzip();
            GpuStats {
                busy: act.zip(max).filter(|(_, m)| *m > 0).map(|(a, m)| (a as f64 / m as f64).clamp(0.0, 1.0)),
                // discrete Arc on xe reports its VRAM like amdgpu does; iGPUs share system RAM
                vram_used: read_u64(format!("{}/mem_info_vram_used", g.dev)),
                vram_total: read_u64(format!("{}/mem_info_vram_total", g.dev)),
                core_mhz: act,
                mem_mhz: None,
            }
        }
    }
}

fn nvidia_query(bus: &str, fields: &str) -> Option<String> {
    let out = std::process::Command::new("nvidia-smi")
        .args([format!("--id={bus}"), format!("--query-gpu={fields}"), "--format=csv,noheader,nounits".into()])
        .output().ok().filter(|o| o.status.success())?;
    read_str(&String::from_utf8_lossy(&out.stdout))
}
fn read_str(s: &str) -> Option<String> {
    Some(s.trim().to_string()).filter(|s| !s.is_empty())
}

thread_local! {
    static NV_CACHE: RefCell<HashMap<String, (Instant, GpuStats)>> = RefCell::new(HashMap::new());
}

/// nvidia-smi is a process spawn, so its answer is reused for 2 s (the panel samples every 0.5 s).
fn nvidia_stats(bus: &str) -> GpuStats {
    if let Some(s) = NV_CACHE.with(|c| c.borrow().get(bus).filter(|(t, _)| t.elapsed() < Duration::from_secs(2)).map(|(_, s)| *s)) {
        return s;
    }
    let s = nvidia_query(bus, "utilization.gpu,memory.used,memory.total,clocks.gr,clocks.mem")
        .map(|line| parse_nvidia(&line))
        .unwrap_or_default();
    NV_CACHE.with(|c| c.borrow_mut().insert(bus.to_string(), (Instant::now(), s)));
    s
}

/// "37, 1024, 16376, 1950, 10501" (percent, MiB, MiB, MHz, MHz); "[N/A]" fields become None.
pub fn parse_nvidia(line: &str) -> GpuStats {
    let f: Vec<Option<u64>> = line.split(',').map(|v| v.trim().parse::<f64>().ok().map(|x| x as u64)).collect();
    let at = |i: usize| f.get(i).copied().flatten();
    GpuStats {
        busy: at(0).map(|p| p as f64 / 100.0),
        vram_used: at(1).map(|m| m << 20),
        vram_total: at(2).map(|m| m << 20),
        core_mhz: at(3),
        mem_mhz: at(4),
    }
}

// ---------------------------------------------------------------- tests

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cpu_names() {
        for (full, short) in [
            ("AMD Ryzen 7 7700X 8-Core Processor", "Ryzen 7 7700X"),
            ("AMD Ryzen 9 7950X3D 16-Core Processor", "Ryzen 9 7950X3D"),
            ("AMD Ryzen 7 PRO 7840U w/ Radeon 780M Graphics", "Ryzen 7 PRO 7840U"),
            ("AMD Ryzen 5 5600G with Radeon Graphics", "Ryzen 5 5600G"),
            ("AMD Ryzen AI 9 HX 370 w/ Radeon 890M", "Ryzen AI 9 HX 370"),
            ("AMD EPYC 7763 64-Core Processor", "EPYC 7763"),
            ("13th Gen Intel(R) Core(TM) i7-13700K", "Core i7-13700K"),
            ("12th Gen Intel(R) Core(TM) i5-12400F", "Core i5-12400F"),
            ("Intel(R) Core(TM) i7-8700 CPU @ 3.20GHz", "Core i7-8700"),
            ("Intel(R) Core(TM) Ultra 7 155H", "Core Ultra 7 155H"),
            ("Intel(R) Core(TM) Ultra 9 285K", "Core Ultra 9 285K"),
            ("Intel(R) Xeon(R) CPU E5-2680 v4 @ 2.40GHz", "Xeon E5-2680 v4"),
            ("Intel(R) Xeon(R) w9-3495X", "Xeon w9-3495X"),
            ("Intel(R) Celeron(R) N4020 CPU @ 1.10GHz", "Celeron N4020"),
            ("Intel(R) Atom(TM) x5-Z8350  CPU @ 1.44GHz", "Atom x5-Z8350"),
            ("AMD Athlon Silver 3050U with Radeon Graphics", "Athlon Silver 3050U"),
            ("AMD Ryzen Threadripper PRO 5995WX 64-Cores", "Ryzen Threadripper PRO 5995WX"),
            ("AMD FX(tm)-8350 Eight-Core Processor", "FX-8350 Eight-Core"),
            ("Hygon C86 7285 32-core Processor", "Hygon C86 7285"),
            ("ZHAOXIN KaiXian KX-U6780A@2.7GHz", "ZHAOXIN KaiXian KX-U6780A@2.7GHz"),
            ("Cortex-A76 + Cortex-A55", "Cortex-A76 + Cortex-A55"),
            ("Neoverse-N1", "Neoverse-N1"),
            ("Apple M2 Pro", "Apple M2 Pro"),
            ("Raspberry Pi 5 Model B Rev 1.0", "Raspberry Pi 5 Model B Rev 1.0"),
            ("Snapdragon(R) X Elite - X1E78100 - Qualcomm(R) Oryon(TM) CPU", "Snapdragon X Elite - X1E78100 - Qualcomm Oryon"),
            ("", ""),
            ("Intel(R) Pentium(R) Gold G6400 CPU @ 4.00GHz", "Pentium Gold G6400"),
            ("Intel(R) N100", "N100"),
        ] {
            assert_eq!(shorten_cpu(full), short, "{full}");
        }
    }

    #[test]
    fn pci_names() {
        assert_eq!(shorten_pci_name("Navi 48 [Radeon RX 9070/9070 XT/9070 GRE]"), "Radeon RX 9070");
        assert_eq!(shorten_pci_name("AD103 [GeForce RTX 4080]"), "GeForce RTX 4080");
        assert_eq!(shorten_pci_name("Raptor Lake-S GT1 [UHD Graphics 770]"), "UHD Graphics 770");
        assert_eq!(shorten_pci_name("Raphael"), "Raphael");
        assert_eq!(shorten_pci_name("DG2 [Arc A770]"), "Arc A770");
    }

    #[test]
    fn nvidia_smi_lines() {
        let s = parse_nvidia("37, 1024, 16376, 1950, 10501");
        assert_eq!(s.busy, Some(0.37));
        assert_eq!(s.vram_total, Some(16376 << 20));
        assert_eq!(s.core_mhz, Some(1950));
        let na = parse_nvidia("[N/A], [N/A], 4096, [N/A], [N/A]");
        assert_eq!(na.busy, None);
        assert_eq!(na.vram_total, Some(4096 << 20));
    }

    /// A fake /sys with the given cards: (vendor, device, revision, vram bytes or 0).
    fn fake_sys(tag: &str, cards: &[(&str, &str, &str, u64)]) -> String {
        let root = std::env::temp_dir().join(format!("aura-hw-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        for (i, (ven, dev, rev, vram)) in cards.iter().enumerate() {
            let card = root.join(format!("class/drm/card{i}"));
            let d = card.join("device");
            std::fs::create_dir_all(&d).unwrap();
            let w = |f: &str, v: String| std::fs::write(d.join(f), v).unwrap();
            w("class", "0x030000\n".into());
            w("vendor", format!("0x{ven}\n"));
            w("device", format!("0x{dev}\n"));
            w("revision", format!("0x{rev}\n"));
            w("uevent", format!("DRIVER=x\nPCI_SLOT_NAME=0000:{:02x}:00.0\n", i + 1));
            if *vram > 0 {
                w("mem_info_vram_total", format!("{vram}\n"));
                w("mem_info_vram_used", "0\n".into());
                w("gpu_busy_percent", "5\n".into());
            }
            // a connector and a render node must not be counted as GPUs
            std::fs::create_dir_all(root.join(format!("class/drm/card{i}-DP-1"))).unwrap();
        }
        std::fs::create_dir_all(root.join("class/drm/renderD128")).unwrap();
        root.to_string_lossy().to_string()
    }

    fn titles(sys: &str) -> Vec<(String, bool)> {
        discover(sys).into_iter().map(|g| (g.title, g.discrete)).collect()
    }

    const GIB: u64 = 1 << 30;
    const RX9070XT: (&str, &str, &str, u64) = ("1002", "7550", "c0", 16 * GIB);
    const RAPHAEL: (&str, &str, &str, u64) = ("1002", "164e", "c3", 512 << 20);
    const RTX4080: (&str, &str, &str, u64) = ("10de", "2704", "a1", 0);
    const UHD770: (&str, &str, &str, u64) = ("8086", "a780", "04", 0);
    const ARC_A770: (&str, &str, &str, u64) = ("8086", "56a0", "08", 0);

    #[test]
    fn every_combination() {
        let t = |s: &str, b| (s.to_string(), b);
        let cases: Vec<(&str, Vec<_>, Vec<(String, bool)>)> = vec![
            ("none", vec![], vec![]),
            ("amd-d+amd-i", vec![RAPHAEL, RX9070XT], vec![t("GPU", true), t("iGPU", false)]),
            ("amd-d-only", vec![RX9070XT], vec![t("GPU", true)]),
            ("amd-apu-only", vec![RAPHAEL], vec![t("iGPU", false)]),
            ("nv+intel-i", vec![UHD770, RTX4080], vec![t("GPU", true), t("iGPU", false)]),
            ("nv+intel-F", vec![RTX4080], vec![t("GPU", true)]),
            ("intel-i-only", vec![UHD770], vec![t("iGPU", false)]),
            ("nv+amd-apu", vec![RAPHAEL, RTX4080], vec![t("GPU", true), t("iGPU", false)]),
            ("amd-d+intel-i", vec![UHD770, RX9070XT], vec![t("GPU", true), t("iGPU", false)]),
            ("arc+intel-i", vec![UHD770, ARC_A770], vec![t("GPU", true), t("iGPU", false)]),
            ("two-nv", vec![RTX4080, RTX4080], vec![t("GPU 1", true), t("GPU 2", true)]),
            ("nv+amd-d+intel-i", vec![UHD770, RX9070XT, RTX4080], vec![t("GPU 1", true), t("GPU 2", true), t("iGPU", false)]),
        ];
        for (tag, cards, want) in cases {
            let sys = fake_sys(tag, &cards);
            assert_eq!(titles(&sys), want, "{tag}");
            let _ = std::fs::remove_dir_all(&sys);
        }
    }

    #[test]
    fn skips_virtual_and_non_display() {
        let sys = fake_sys("virt", &[("1af4", "1050", "01", 0)]); // virtio-gpu
        assert!(discover(&sys).is_empty());
        let _ = std::fs::remove_dir_all(&sys);
    }

    #[test]
    fn names_from_tables() {
        let sys = fake_sys("names", &[RX9070XT, RAPHAEL, UHD770, ARC_A770, RTX4080]);
        let names: Vec<String> = discover(&sys).into_iter().map(|g| g.name).collect();
        // nvidia-smi can't answer for a fake bus, so the RTX falls back to pci.ids
        for want in ["Radeon RX 9070 XT", "Raphael", "GeForce RTX 4080", "Arc A770", "UHD Graphics 770"] {
            assert!(names.iter().any(|n| n == want), "missing {want} in {names:?}");
        }
        let _ = std::fs::remove_dir_all(&sys);
    }

    /// `cargo test this_machine -- --ignored --nocapture` prints what the panel will show here.
    #[test]
    #[ignore]
    fn this_machine() {
        println!("CPU  {}", cpu_name());
        for g in discover("/sys") {
            println!("{:<5} {:<20} {:?} {} discrete={} {:?}", g.title, g.name, g.vendor, g.bus, g.discrete, stats(&g));
        }
    }
}
