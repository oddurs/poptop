//! Temperatures and fans, grouped into what they are about.
//!
//! Every platform publishes sensors in its own vocabulary — hwmon driver names
//! on Linux, `PMU tdie6` and `NAND CH0 temp` on a Mac — and none of it is a
//! reading anyone can use as it stands. This is where the vocabulary becomes
//! six groups, and forty sensors become the hottest in each.

use crate::sample::{Fan, Gpu, Power, Temp};
use std::path::Path;
use std::sync::Arc;

/// The groups, in the order a reader asks about them. The first one present
/// is the header's figure.
pub const GROUPS: [&str; 6] = ["cpu", "gpu", "storage", "memory", "battery", "board"];

/// Readings outside this are a sensor reporting that it has no reading:
/// `-273`, `127`, `255` and `0x7fff` all turn up from real drivers.
const PLAUSIBLE: std::ops::Range<f32> = -40.0..150.0;

/// One raw reading, before grouping.
pub struct Reading<'a> {
    pub group: &'static str,
    pub sensor: &'a str,
    pub celsius: f32,
    pub crit: Option<f32>,
}

/// The hottest plausible reading in each group, in [`GROUPS`] order.
pub fn hottest<'a>(readings: impl IntoIterator<Item = Reading<'a>>) -> Vec<Temp> {
    let mut best: [Option<Reading<'a>>; GROUPS.len()] = Default::default();
    for r in readings {
        if !PLAUSIBLE.contains(&r.celsius) {
            continue;
        }
        let Some(i) = GROUPS.iter().position(|g| *g == r.group) else {
            continue;
        };
        if best[i].as_ref().is_none_or(|b| r.celsius > b.celsius) {
            best[i] = Some(r);
        }
    }
    best.into_iter()
        .flatten()
        .map(|r| Temp {
            group: Arc::from(r.group),
            celsius: r.celsius,
            sensor: Arc::from(r.sensor),
            crit: r.crit.filter(|c| PLAUSIBLE.contains(c) && *c > 0.0),
        })
        .collect()
}

/// Which group a hwmon device's readings belong to, from its driver name.
///
/// By driver rather than by label: labels are free text the driver chooses
/// (`Tctl`, `Package id 0`, `Composite`, `edge`), while the driver says what
/// the chip is. Anything unrecognised is the board's — the Super I/O chips
/// (`nct6775`, `it87`) and ACPI's thermal zone are exactly that.
pub fn hwmon_group(driver: &str) -> &'static str {
    match driver {
        "coretemp" | "k10temp" | "zenpower" | "cpu_thermal" | "cpu-thermal" | "x86_pkg_temp"
        | "via_cputemp" | "fam15h_power" => "cpu",
        "amdgpu" | "radeon" | "nouveau" | "i915" | "xe" | "gpu_thermal" | "gpu-thermal" => "gpu",
        "nvme" | "drivetemp" => "storage",
        "spd5118" | "jc42" | "ee1004" => "memory",
        "BAT0" | "BAT1" | "battery" | "sbs_battery" => "battery",
        _ => "board",
    }
}

/// Which group a macOS sensor belongs to, from its label.
///
/// Apple Silicon names its die sensors `PMU tdie*` and `PMU2 tdie*` — the SoC's
/// dies, CPU and GPU clusters together, so they are the CPU figure — and its
/// device sensors `tdev*`, which are the board. Intel Macs publish SMC names
/// like `CPU Proximity` and `GPU Die`. `tcal` is a calibration constant rather
/// than a temperature and is left out.
#[cfg_attr(target_os = "linux", allow(dead_code))]
pub fn label_group(label: &str) -> Option<&'static str> {
    let l = label.to_ascii_lowercase();
    if l.contains("tcal") {
        return None;
    }
    Some(
        if l.contains("tdie") || l.contains("cpu") || l.contains("core") {
            "cpu"
        } else if l.contains("gpu") {
            "gpu"
        } else if l.contains("nand")
            || l.contains("ssd")
            || l.contains("drive")
            || l.contains("disk")
        {
            "storage"
        } else if l.contains("dimm") || l.contains("memory") {
            "memory"
        } else if l.contains("batt") || l.contains("gas gauge") {
            "battery"
        } else {
            "board"
        },
    )
}

/// Temperatures and fans from a hwmon tree: `/sys/class/hwmon` on a machine,
/// a directory of fixtures in a test.
///
/// A few dozen small reads on a desktop, which is why this is read every
/// sample on Linux: the whole walk costs less than one process's `stat`.
/// Either list is `None` when no device publishes any, so a machine without
/// fans says nothing about fans rather than claiming it has none.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
pub fn hwmon(root: &Path) -> (Option<Vec<Temp>>, Option<Vec<Fan>>) {
    let Ok(dir) = std::fs::read_dir(root) else {
        return (None, None);
    };
    let mut devices: Vec<_> = dir.flatten().map(|e| e.path()).collect();
    // hwmonN order is probe order, which differs between boots. Sorted so the
    // same machine lists its fans the same way every time.
    devices.sort();
    let read = |p: &Path| std::fs::read_to_string(p).ok();
    let milli = |p: &Path| -> Option<f32> { Some(read(p)?.trim().parse::<f32>().ok()? / 1000.0) };

    let mut temps: Vec<(&'static str, String, f32, Option<f32>)> = Vec::new();
    let mut fans = Vec::new();
    for dev in devices {
        // Older drivers put their attributes under `device/`.
        let base = if dev.join("name").exists() || !dev.join("device/name").exists() {
            dev.clone()
        } else {
            dev.join("device")
        };
        let Some(driver) = read(&base.join("name")).map(|s| s.trim().to_string()) else {
            continue;
        };
        let group = hwmon_group(&driver);
        let Ok(files) = std::fs::read_dir(&base) else {
            continue;
        };
        let mut names: Vec<String> = files
            .flatten()
            .filter_map(|e| e.file_name().into_string().ok())
            .collect();
        names.sort();
        for name in &names {
            if let Some(n) = attr(name, "temp") {
                let Some(celsius) = milli(&base.join(name)) else {
                    continue;
                };
                let label = read(&base.join(format!("temp{n}_label")))
                    .map(|l| format!("{driver} {}", l.trim()))
                    .unwrap_or_else(|| format!("{driver} temp{n}"));
                let crit = milli(&base.join(format!("temp{n}_crit")))
                    .or_else(|| milli(&base.join(format!("temp{n}_max"))));
                temps.push((group, label, celsius, crit));
            } else if let Some(n) = attr(name, "fan") {
                let Some(rpm) = read(&base.join(name)).and_then(|v| v.trim().parse::<u32>().ok())
                else {
                    continue;
                };
                let label = read(&base.join(format!("fan{n}_label")))
                    .map(|l| l.trim().to_string())
                    .unwrap_or_else(|| format!("{driver} fan{n}"));
                fans.push(Fan {
                    label: Arc::from(label),
                    rpm,
                });
            }
        }
    }
    let temps = hottest(temps.iter().map(|(g, s, c, crit)| Reading {
        group: g,
        sensor: s,
        celsius: *c,
        crit: *crit,
    }));
    (
        (!temps.is_empty()).then_some(temps),
        (!fans.is_empty()).then_some(fans),
    )
}

/// The battery, from a `power_supply` tree: `/sys/class/power_supply` on a
/// machine, a directory of fixtures in a test.
///
/// Every supply whose `type` is `Battery`, combined by energy where the driver
/// publishes it, so a ThinkPad's two cells read as one battery at their true
/// combined charge rather than as the first one. Peripherals — a mouse, a
/// headset — publish batteries too and say so in `scope`; they are not the
/// machine's.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
pub fn power_supply(root: &Path) -> Option<Power> {
    let read = |p: &Path| {
        std::fs::read_to_string(p)
            .ok()
            .map(|s| s.trim().to_string())
    };
    let num = |p: &Path| read(p)?.parse::<f64>().ok();
    let mut dirs: Vec<_> = std::fs::read_dir(root)
        .ok()?
        .flatten()
        .map(|e| e.path())
        .collect();
    dirs.sort();
    let cells: Vec<_> = dirs
        .into_iter()
        .filter(|d| read(&d.join("type")).as_deref() == Some("Battery"))
        .filter(|d| read(&d.join("scope")).as_deref() != Some("Device"))
        .filter(|d| read(&d.join("present")).as_deref() != Some("0"))
        .collect();
    let first = cells.first()?;

    // Energy in µWh, or charge in µAh — whichever the driver keeps.
    let pair = |d: &Path, now: &str, full: &str| Some((num(&d.join(now))?, num(&d.join(full))?));
    let stored = |d: &Path| {
        pair(d, "energy_now", "energy_full").or_else(|| pair(d, "charge_now", "charge_full"))
    };
    let (now, full) = cells
        .iter()
        .map(|d| stored(d))
        .collect::<Option<Vec<_>>>()
        .map(|v| v.iter().fold((0.0, 0.0), |(n, f), (a, b)| (n + a, f + b)))
        .unwrap_or((0.0, 0.0));
    let charge = if full > 0.0 {
        (now / full * 100.0) as f32
    } else {
        num(&first.join("capacity"))? as f32
    };

    let status = |d: &Path| read(&d.join("status")).unwrap_or_default();
    let state = if cells.iter().any(|d| status(d) == "Charging") {
        "charging"
    } else if cells.iter().any(|d| status(d) == "Discharging") {
        "discharging"
    } else {
        "charged"
    };

    // µW, or µA × µV. Drivers disagree about the sign; poptop's is fixed by
    // the state instead.
    let draw = |d: &Path| {
        num(&d.join("power_now"))
            .or_else(|| Some(num(&d.join("current_now"))? * num(&d.join("voltage_now"))? / 1e6))
    };
    // Summed over the cells that publish one: an idle second cell often
    // publishes nothing, and that is not an unknown draw for the pair.
    let draws: Vec<f64> = cells.iter().filter_map(|d| draw(d)).map(f64::abs).collect();
    let micro_w = (!draws.is_empty()).then(|| draws.iter().sum::<f64>());
    let watts = micro_w.filter(|w| *w > 0.0 && state != "charged").map(|w| {
        let w = (w / 1e6) as f32;
        if state == "charging" { -w } else { w }
    });
    // Hours are stored ÷ drawn, when both are in energy; charge units would
    // need the voltage and are left to the platform's own estimate, which
    // sysfs does not publish.
    let minutes = match (micro_w, full > 0.0 && stored(first).is_some()) {
        (Some(w), true) if w > 0.0 && state != "charged" => {
            let left = if state == "charging" { full - now } else { now };
            Some((left / w * 60.0) as u32)
        }
        _ => None,
    };
    Some(Power {
        charge: charge.clamp(0.0, 100.0),
        state: Arc::from(state),
        watts,
        minutes,
    })
}

/// GPU load from a `drm` tree: `/sys/class/drm` on a machine.
///
/// Only what the kernel publishes to an ordinary reader. amdgpu does, as
/// `gpu_busy_percent` beside its VRAM counters; i915 and nouveau do not, and
/// NVIDIA's needs its own library — so those machines report no GPU rather
/// than a GPU at zero.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
pub fn drm(root: &Path) -> Option<Vec<Gpu>> {
    let read = |p: &Path| {
        std::fs::read_to_string(p)
            .ok()
            .map(|s| s.trim().to_string())
    };
    let mut cards: Vec<_> = std::fs::read_dir(root)
        .ok()?
        .flatten()
        .filter_map(|e| e.file_name().into_string().ok())
        // `card0`, not its connectors `card0-DP-1`.
        .filter(|n| {
            n.strip_prefix("card")
                .is_some_and(|r| r.bytes().all(|b| b.is_ascii_digit()))
        })
        .collect();
    cards.sort();
    let gpus: Vec<Gpu> = cards
        .into_iter()
        .filter_map(|card| {
            let dev = root.join(&card).join("device");
            let util = read(&dev.join("gpu_busy_percent"))?.parse::<f32>().ok()?;
            Some(Gpu {
                name: Arc::from(card),
                util: util.clamp(0.0, 100.0),
                mem_used: read(&dev.join("mem_info_vram_used")).and_then(|v| v.parse().ok()),
                mem_total: read(&dev.join("mem_info_vram_total")).and_then(|v| v.parse().ok()),
            })
        })
        .collect();
    (!gpus.is_empty()).then_some(gpus)
}

/// Everything the hardware reports in one reading.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Hardware {
    pub temps: Option<Vec<Temp>>,
    pub fans: Option<Vec<Fan>>,
    pub power: Option<Power>,
    pub gpus: Option<Vec<Gpu>>,
}

/// The hardware, read on a thread of its own so no sample waits for it.
///
/// A sample takes the latest finished reading and asks for another, which is
/// taken while the machine gets on with everything else. Reading forty
/// sensors on a Mac is 1.4ms of CPU and 45ms of waiting on the sensor
/// service; a hwmon attribute backed by a drive's SMART log can take longer.
/// Temperatures, a battery and a GPU's load all move over seconds, so a
/// reading one interval old is the right trade for a sample that never waits
/// — and for the budget never seeing a wait it can do nothing about.
///
/// At most one reading a second, however short the interval, and only when
/// asked, so a poptop sampling once a minute reads once a minute.
pub struct Background {
    latest: std::sync::Arc<std::sync::Mutex<Option<Hardware>>>,
    ask: std::sync::mpsc::SyncSender<()>,
}

/// The shortest gap between two readings.
const READ_AT_MOST: std::time::Duration = std::time::Duration::from_secs(1);

impl Background {
    /// Start the reader. The thread ends when this is dropped.
    pub fn start(mut read: impl FnMut() -> Hardware + Send + 'static) -> Option<Self> {
        let latest = std::sync::Arc::new(std::sync::Mutex::new(None));
        // One slot: a request made while one is pending is the same request.
        let (ask, asked) = std::sync::mpsc::sync_channel::<()>(1);
        let into = latest.clone();
        std::thread::Builder::new()
            .name("sensors".into())
            .spawn(move || {
                let mut last: Option<std::time::Instant> = None;
                for () in asked {
                    if last.is_some_and(|t| t.elapsed() < READ_AT_MOST) {
                        continue;
                    }
                    last = Some(std::time::Instant::now());
                    let h = read();
                    *into.lock().unwrap_or_else(|p| p.into_inner()) = Some(h);
                }
            })
            .ok()?;
        Some(Background { latest, ask })
    }

    /// The latest finished reading, if there has been one, and a request for
    /// the next.
    pub fn take(&self) -> Option<Hardware> {
        let _ = self.ask.try_send(());
        self.latest
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .clone()
    }
}

/// `N` from `{kind}N_input`.
fn attr<'a>(file: &'a str, kind: &str) -> Option<&'a str> {
    let n = file.strip_prefix(kind)?.strip_suffix("_input")?;
    (!n.is_empty() && n.bytes().all(|b| b.is_ascii_digit())).then_some(n)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r(group: &'static str, sensor: &'static str, celsius: f32) -> Reading<'static> {
        Reading {
            group,
            sensor,
            celsius,
            crit: None,
        }
    }

    #[test]
    fn each_group_keeps_its_hottest_in_a_fixed_order() {
        let t = hottest([
            r("storage", "nvme Composite", 41.0),
            r("cpu", "PMU tdie2", 66.4),
            r("cpu", "PMU tdie1", 77.1),
            r("board", "PMU tdev1", 58.4),
            r("cpu", "junk", 127.0 + 100.0),
        ]);
        let got: Vec<(&str, &str, f32)> = t
            .iter()
            .map(|t| (&*t.group, &*t.sensor, t.celsius))
            .collect();
        assert_eq!(
            got,
            [
                ("cpu", "PMU tdie1", 77.1),
                ("storage", "nvme Composite", 41.0),
                ("board", "PMU tdev1", 58.4),
            ]
        );
    }

    #[test]
    fn a_sensor_with_no_reading_is_not_the_hottest() {
        // -273 and 255 are what drivers report for "no reading".
        let t = hottest([
            r("cpu", "a", 255.0),
            r("cpu", "b", -273.0),
            r("cpu", "c", 50.0),
        ]);
        assert_eq!(t.len(), 1);
        assert_eq!(&*t[0].sensor, "c");
    }

    #[test]
    fn a_slow_reading_never_holds_up_a_sample() {
        let reads = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0));
        let counted = reads.clone();
        let bg = Background::start(move || {
            counted.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            std::thread::sleep(std::time::Duration::from_millis(200));
            Hardware {
                fans: Some(vec![Fan {
                    label: "f".into(),
                    rpm: 1,
                }]),
                ..Hardware::default()
            }
        })
        .expect("no thread");
        let t0 = std::time::Instant::now();
        // Nothing yet, and asking did not wait for the 200ms read.
        assert_eq!(bg.take(), None);
        assert!(t0.elapsed() < std::time::Duration::from_millis(50));
        let end = std::time::Instant::now() + std::time::Duration::from_secs(3);
        let got = loop {
            if let Some(h) = bg.take() {
                break h;
            }
            assert!(std::time::Instant::now() < end, "no reading arrived");
            std::thread::sleep(std::time::Duration::from_millis(10));
        };
        assert!(got.fans.is_some());
        // Asked dozens of times in that loop, read once or twice: at most
        // once a second.
        assert!(reads.load(std::sync::atomic::Ordering::Relaxed) <= 2);
    }

    #[test]
    fn mac_labels_group_as_the_hardware_they_measure() {
        assert_eq!(label_group("PMU tdie6"), Some("cpu"));
        assert_eq!(label_group("PMU2 tdie1"), Some("cpu"));
        assert_eq!(label_group("PMU tdev3"), Some("board"));
        assert_eq!(label_group("NAND CH0 temp"), Some("storage"));
        assert_eq!(label_group("PMU tcal"), None);
        assert_eq!(label_group("GPU Die"), Some("gpu"));
        assert_eq!(label_group("Battery"), Some("battery"));
    }

    #[test]
    fn a_hwmon_tree_is_read_into_groups_and_fans() {
        let root = std::env::temp_dir().join(format!("poptop-hwmon-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let dev = |n: &str, files: &[(&str, &str)]| {
            let d = root.join(n);
            std::fs::create_dir_all(&d).expect("mkdir");
            for (f, v) in files {
                std::fs::write(d.join(f), v).expect("write");
            }
        };
        dev(
            "hwmon0",
            &[
                ("name", "coretemp\n"),
                ("temp1_input", "71000\n"),
                ("temp1_label", "Package id 0\n"),
                ("temp1_crit", "100000\n"),
                ("temp2_input", "65000\n"),
                ("temp2_label", "Core 0\n"),
            ],
        );
        dev(
            "hwmon1",
            &[
                ("name", "nvme\n"),
                ("temp1_input", "38850\n"),
                ("temp1_label", "Composite\n"),
                ("temp1_crit", "84850\n"),
            ],
        );
        dev(
            "hwmon2",
            &[
                ("name", "thinkpad\n"),
                ("fan1_input", "2350\n"),
                ("fan2_input", "0\n"),
                ("fan2_label", "gpu fan\n"),
                // An attribute that is not a reading.
                ("temp1_input_bogus", "1\n"),
            ],
        );
        // An old driver, attributes under `device/`.
        dev(
            "hwmon3/device",
            &[("name", "k10temp\n"), ("temp1_input", "80125\n")],
        );

        let (temps, fans) = hwmon(&root);
        let temps = temps.expect("no temperatures");
        assert_eq!(temps[0].group.as_ref(), "cpu");
        assert_eq!(temps[0].celsius, 80.125);
        assert_eq!(temps[0].sensor.as_ref(), "k10temp temp1");
        assert_eq!(temps[1].group.as_ref(), "storage");
        assert_eq!(temps[1].crit, Some(84.85));
        let fans = fans.expect("no fans");
        assert_eq!(
            fans.iter().map(|f| (&*f.label, f.rpm)).collect::<Vec<_>>(),
            [("thinkpad fan1", 2350), ("gpu fan", 0)]
        );

        assert_eq!(hwmon(&root.join("absent")), (None, None));
        let _ = std::fs::remove_dir_all(&root);
    }

    fn tree(name: &str, files: &[(&str, &str)]) -> std::path::PathBuf {
        let root = std::env::temp_dir().join(format!("poptop-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        for (f, v) in files {
            let p = root.join(f);
            std::fs::create_dir_all(p.parent().expect("a parent")).expect("mkdir");
            std::fs::write(p, v).expect("write");
        }
        root
    }

    #[test]
    fn two_cells_read_as_one_battery_at_their_combined_charge() {
        let root = tree(
            "ps",
            &[
                ("AC/type", "Mains\n"),
                ("AC/online", "0\n"),
                ("BAT0/type", "Battery\n"),
                ("BAT0/status", "Discharging\n"),
                ("BAT0/energy_now", "20000000\n"),
                ("BAT0/energy_full", "40000000\n"),
                ("BAT0/power_now", "10000000\n"),
                ("BAT1/type", "Battery\n"),
                ("BAT1/status", "Unknown\n"),
                ("BAT1/energy_now", "20000000\n"),
                ("BAT1/energy_full", "20000000\n"),
                // A mouse, which is not the machine's battery.
                ("hidpp_battery_0/type", "Battery\n"),
                ("hidpp_battery_0/scope", "Device\n"),
                ("hidpp_battery_0/capacity", "5\n"),
            ],
        );
        let p = power_supply(&root).expect("no battery");
        assert!((p.charge - 66.67).abs() < 0.01, "{p:?}");
        assert_eq!(&*p.state, "discharging");
        assert_eq!(p.watts, Some(10.0));
        // 40Wh left at 10W.
        assert_eq!(p.minutes, Some(240));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_charging_battery_draws_negative_and_a_desktop_has_none() {
        let root = tree(
            "ps2",
            &[
                ("BAT0/type", "Battery\n"),
                ("BAT0/status", "Charging\n"),
                ("BAT0/capacity", "71\n"),
                ("BAT0/current_now", "2000000\n"),
                ("BAT0/voltage_now", "12000000\n"),
            ],
        );
        let p = power_supply(&root).expect("no battery");
        assert_eq!(p.charge, 71.0);
        assert_eq!(p.watts, Some(-24.0));
        assert_eq!(p.minutes, None, "charge units carry no time estimate");
        let _ = std::fs::remove_dir_all(&root);

        let desk = tree("ps3", &[("AC/type", "Mains\n")]);
        assert_eq!(power_supply(&desk), None);
        let _ = std::fs::remove_dir_all(&desk);
    }

    #[test]
    fn only_a_gpu_that_publishes_its_load_is_reported() {
        let root = tree(
            "drm",
            &[
                ("card0/device/gpu_busy_percent", "37\n"),
                ("card0/device/mem_info_vram_used", "1073741824\n"),
                ("card0/device/mem_info_vram_total", "8589934592\n"),
                ("card0-DP-1/status", "connected\n"),
                // An Intel card: no load published, so not reported at zero.
                ("card1/device/vendor", "0x8086\n"),
            ],
        );
        let g = drm(&root).expect("no gpu");
        assert_eq!(g.len(), 1);
        assert_eq!((&*g[0].name, g[0].util), ("card0", 37.0));
        assert_eq!(g[0].mem_total, Some(8 << 30));
        let _ = std::fs::remove_dir_all(&root);
        let none = tree("drm2", &[("card0/device/vendor", "0x8086\n")]);
        assert_eq!(drm(&none), None);
        let _ = std::fs::remove_dir_all(&none);
    }
}
