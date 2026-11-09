//! Sampling is owned by one worker, never by the UI thread. No process scanning.
use std::{collections::BTreeMap, fs, time::Instant};
use sysinfo::{Disks, Networks, System};

#[derive(Debug, Clone, Default)]
/// GPU utilization and optional dedicated memory usage.
pub struct Gpu {
    pub name: String,
    pub busy: Option<f32>,
    pub memory: Option<(u64, u64)>,
}
#[derive(Debug, Clone, Default)]
/// Mount capacity and optional device throughput.
pub struct Disk {
    pub used: u64,
    pub total: u64,
    pub read_write: Option<(f64, f64)>,
}
#[derive(Debug, Clone, Default)]
/// Resource readings from a single collection cycle.
pub struct Snapshot {
    pub cpus: Vec<f32>,
    pub ram: (u64, u64),
    pub swap: (u64, u64),
    pub networks: BTreeMap<String, (f64, f64)>,
    pub disks: BTreeMap<String, Disk>,
    pub gpus: Vec<Gpu>,
}
/// Returns capacity usage, or unavailable for zero capacity.
pub fn percent(used: u64, total: u64) -> Option<f32> {
    (total > 0).then(|| (100.0 * used as f64 / total as f64).clamp(0.0, 100.0) as f32)
}
/// Converts a cumulative counter delta to a per-second rate.
pub fn rate(current: u64, previous: u64, seconds: f64) -> f64 {
    if !seconds.is_finite() || seconds <= 0.0 {
        return 0.0;
    }
    current.saturating_sub(previous) as f64 / seconds
}
/// A newly discovered device establishes a baseline instead of reporting its lifetime traffic.
fn counter_rates(current: (u64, u64), previous: Option<(u64, u64)>, seconds: f64) -> (f64, f64) {
    let previous = previous.unwrap_or(current);
    (
        rate(current.0, previous.0, seconds),
        rate(current.1, previous.1, seconds),
    )
}
/// Formats bytes using binary units.
pub fn bytes(value: f64) -> String {
    for (scale, unit) in [
        (1_099_511_627_776.0, "TiB"),
        (1_073_741_824.0, "GiB"),
        (1_048_576.0, "MiB"),
        (1024.0, "KiB"),
    ] {
        if value >= scale {
            return format!("{:.1} {unit}", value / scale);
        }
    }
    format!("{value:.0} B")
}
/// Sensor handles and previous counters for elapsed-time rates.
pub struct Collector {
    system: System,
    networks: Networks,
    disks: Disks,
    previous: BTreeMap<String, (u64, u64)>,
    disk_previous: BTreeMap<String, (u64, u64)>,
    last: Instant,
    nvml: Option<nvml_wrapper::Nvml>,
}
impl Collector {
    pub fn new() -> Self {
        let mut system = System::new();
        system.refresh_cpu_usage();
        let networks = Networks::new_with_refreshed_list();
        let previous = networks
            .iter()
            .map(|(n, d)| (n.clone(), (d.total_received(), d.total_transmitted())))
            .collect();
        let disks = Disks::new_with_refreshed_list();
        let disk_previous = disk_counters();
        Self {
            system,
            networks,
            previous,
            disks,
            disk_previous,
            last: Instant::now(),
            nvml: nvml_wrapper::Nvml::init().ok(),
        }
    }
    pub fn sample(&mut self) -> Snapshot {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last).as_secs_f64();
        self.last = now;
        self.system.refresh_cpu_usage();
        self.system.refresh_memory();
        self.networks.refresh(true);
        self.disks.refresh(true);
        let mut networks = BTreeMap::new();
        for (name, data) in &self.networks {
            let current = (data.total_received(), data.total_transmitted());
            if name != "lo" {
                networks.insert(
                    name.clone(),
                    counter_rates(current, self.previous.get(name).copied(), elapsed),
                );
            }
            self.previous.insert(name.clone(), current);
        }
        self.previous
            .retain(|name, _| self.networks.iter().any(|(n, _)| n == name));
        let counters = disk_counters();
        let disks = self
            .disks
            .iter()
            .map(|d| {
                let device = d.name().to_string_lossy();
                let resolved = fs::canonicalize(d.name())
                    .ok()
                    .map(|p| p.to_string_lossy().into_owned());
                let key = resolved.as_deref().unwrap_or(&device);
                let read_write = counters.get(key).map(|current| {
                    counter_rates(*current, self.disk_previous.get(key).copied(), elapsed)
                });
                (
                    d.mount_point().to_string_lossy().into_owned(),
                    Disk {
                        used: d.total_space().saturating_sub(d.available_space()),
                        total: d.total_space(),
                        read_write,
                    },
                )
            })
            .collect();
        self.disk_previous = counters;
        Snapshot {
            cpus: self.system.cpus().iter().map(|c| c.cpu_usage()).collect(),
            ram: (self.system.used_memory(), self.system.total_memory()),
            swap: (self.system.used_swap(), self.system.total_swap()),
            networks,
            disks,
            gpus: self.gpus(),
        }
    }
    fn gpus(&self) -> Vec<Gpu> {
        let mut result = Vec::new();
        if let Some(nvml) = &self.nvml {
            for index in 0..nvml.device_count().unwrap_or(0) {
                if let Ok(device) = nvml.device_by_index(index) {
                    result.push(Gpu {
                        name: device.name().unwrap_or_else(|_| format!("NVIDIA {index}")),
                        busy: device.utilization_rates().ok().map(|v| v.gpu as f32),
                        memory: device.memory_info().ok().map(|v| (v.used, v.total)),
                    });
                }
            }
        }
        if let Ok(entries) = fs::read_dir("/sys/class/drm") {
            let mut entries: Vec<_> = entries.flatten().collect();
            entries.sort_by_key(|e| e.file_name());
            for entry in entries {
                let name = entry.file_name().to_string_lossy().into_owned();
                if !name
                    .strip_prefix("card")
                    .is_some_and(|n| !n.is_empty() && n.chars().all(|c| c.is_ascii_digit()))
                {
                    continue;
                }
                let base = entry.path().join("device");
                if fs::read_to_string(base.join("vendor")).is_ok_and(|s| s.trim() == "0x10de") {
                    continue;
                }
                let read = |key: &str| {
                    fs::read_to_string(base.join(key))
                        .ok()?
                        .trim()
                        .parse::<u64>()
                        .ok()
                };
                let busy = read("gpu_busy_percent").map(|v| v.min(100) as f32);
                let memory = read("mem_info_vram_used").zip(read("mem_info_vram_total"));
                result.push(Gpu { name, busy, memory });
            }
        }
        result
    }
}
/// Linux diskstats sectors are always 512 bytes, regardless of physical sector size.
fn parse_diskstats(text: &str) -> BTreeMap<String, (u64, u64)> {
    text.lines()
        .filter_map(|line| {
            let columns: Vec<_> = line.split_whitespace().collect();
            let name = *columns.get(2)?;
            let read = columns.get(5)?.parse::<u64>().ok()?.checked_mul(512)?;
            let write = columns.get(9)?.parse::<u64>().ok()?.checked_mul(512)?;
            Some((format!("/dev/{name}"), (read, write)))
        })
        .collect()
}
fn disk_counters() -> BTreeMap<String, (u64, u64)> {
    let mut counters = fs::read_to_string("/proc/diskstats")
        .map(|s| parse_diskstats(&s))
        .unwrap_or_default();
    // /dev/mapper symlinks may be hidden in sandboxes. sysfs still identifies them.
    if let Ok(entries) = fs::read_dir("/sys/class/block") {
        for entry in entries.flatten() {
            if let Ok(name) = fs::read_to_string(entry.path().join("dm/name")) {
                let key = format!("/dev/{}", entry.file_name().to_string_lossy());
                if let Some(value) = counters.get(&key).copied() {
                    counters.insert(format!("/dev/mapper/{}", name.trim()), value);
                }
            }
        }
    }
    counters
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn counters_handle_reset_and_elapsed_time() {
        assert_eq!(rate(300, 100, 2.0), 100.0);
        assert_eq!(rate(10, 300, 2.0), 0.0);
        for elapsed in [0.0, -1.0, f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            assert_eq!(rate(300, 100, elapsed), 0.0);
        }
    }
    #[test]
    fn capacity_handles_disabled_swap() {
        assert_eq!(percent(0, 0), None);
        assert_eq!(percent(5, 10), Some(50.0));
        assert_eq!(percent(11, 10), Some(100.0));
        assert_eq!(bytes(1_048_576.0), "1.0 MiB");
    }
    #[test]
    fn diskstats_uses_sector_counters_not_request_counts() {
        let parsed = parse_diskstats("252 0 dm-0 100 0 200 4 50 0 300 8 0 5 6\nmalformed");
        assert_eq!(parsed["/dev/dm-0"], (102400, 153600));
        assert_eq!(parsed.len(), 1);
    }
    #[test]
    fn new_devices_start_at_zero_and_counters_reset_independently() {
        assert_eq!(counter_rates((1_000_000, 500_000), None, 2.0), (0.0, 0.0));
        assert_eq!(
            counter_rates((500, 900), Some((100, 300)), 2.0),
            (200.0, 300.0)
        );
        assert_eq!(
            counter_rates((10, 900), Some((500, 300)), 2.0),
            (0.0, 300.0)
        );
        assert_eq!(
            counter_rates((500, 10), Some((100, 300)), 2.0),
            (200.0, 0.0)
        );
    }
    #[test]
    fn malformed_diskstats_do_not_discard_other_devices() {
        let parsed = parse_diskstats(concat!(
            "8 0 sda 1 0 2 0 3 0 4 0 0 0 0 0 0 0\n",
            "8 1 sda1 1 0 nope 0 3 0 4\n",
            "8 2 sda2 1 0 18446744073709551615 0 3 0 4\n",
            "8 3 sda3 1 0 2 0 3 0 18446744073709551615\n",
            "8 4 sda4 1 0 2\n",
            "259 0 nvme0n1 0 0 0 0 0 0 0\n",
        ));
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed["/dev/sda"], (1024, 2048));
        assert_eq!(parsed["/dev/nvme0n1"], (0, 0));
    }
}
