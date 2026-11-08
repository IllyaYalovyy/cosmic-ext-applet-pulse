# Pulse for COSMIC

A configurable Rust resource monitor for the COSMIC panel and dock. Each resource
has an icon and narrow vertical meters; CPU has one thin bar per logical core.
Click to open text readings, About, System Monitor and a donation link. All settings persist through cosmic-config
and external changes apply live. No root privileges or monitoring daemon needed.

## Readings

- **CPU:** logical CPUs in OS order; independent load bars, average in the popup.
- **RAM:** used / total memory (available reclaimable memory is not counted as used).
- **Swap:** used / total; disabled swap is gray, never a divide-by-zero or false alarm.
- **Network:** download then upload in bytes/second, measured over actual elapsed
  time. Select an interface or aggregate all except loopback. Aggregation includes
  VPNs/bridges, so select a physical interface to avoid counting traffic twice.
- **Disk:** selected mount's used space, read rate and write rate, in that order.
  I/O is device activity from Linux block counters; mounts on the same device share
  activity, not just writes to files under the chosen directory.
- **GPU:** utilization then VRAM per detected GPU. NVIDIA uses dynamically loaded
  NVML (no build-time NVIDIA SDK); AMD/other DRM drivers use exported sysfs
  counters. Intel and other drivers without these counters show unavailable;
  no privileged perf access is attempted. Shared GPU memory is not invented as VRAM.

Green below 70%, yellow at 70%, red at 90%, configurable. Rates use configurable
full scales (default network 100 MiB/s, disk 500 MiB/s); red means high load relative
to your scale, not a hardware fault. Gray with a dash means unavailable. The fill
height carries the same information as color. Vertical panels wrap cores to fit the panel width. Very high core counts can occupy substantial horizontal panel space;
CPU can be hidden independently.

## Build and install

Requires Linux, Rust 1.95+, a COSMIC session, and the development packages for
Wayland, xkbcommon, udev, input, seat, EGL, fontconfig and D-Bus. On Ubuntu/Pop!_OS:

```sh
sudo apt install build-essential pkg-config libwayland-dev libxkbcommon-dev \
  libudev-dev libinput-dev libseat-dev libegl1-mesa-dev libfontconfig1-dev libdbus-1-dev
cargo build --release --locked
./scripts/install.sh
```

Installs to `~/.local` by default; `PREFIX=/usr/local ./scripts/install.sh` selects
another prefix if writable. In **COSMIC Settings → Desktop → Panel (or Dock) →
Applets**, add **Pulse**. Restart the panel/session if its applet catalog is stale.
Running `cargo run` alone does not reproduce panel embedding.

```sh
cargo run --locked -- --sample  # one real sensor sample, no UI/session required
./scripts/quality.sh           # formatting, Clippy, unit tests, desktop validation
```

Configuration lives in `~/.config/cosmic/io.github.illyayalovyy.Pulse/v1/` (or
`$XDG_CONFIG_HOME/cosmic/...`). Settings include resource visibility, icons, 1–30 s
refresh, warning/critical thresholds, rate scales, interface and mount point.
Edit these cosmic-config entries to customize the applet. Changes apply live.
Invalid numeric values are normalized.

## Development

Based on the [official applet lifecycle](https://github.com/pop-os/cosmic-applet-template)
and pinned to the same libcosmic revision as the workspace's Claudometer. Cargo.lock
is tracked. UI (`view.rs`), application lifecycle (`main.rs`), config (`config.rs`)
and sampling (`metrics.rs`) are separate. Blocking reads run on a worker; only one
sample is in flight, and the channel is bounded. CPU/process enumeration is limited
to CPU statistics; the applet does not scan processes. GPU support is optional at
runtime. Native English UI strings currently live in the views.

Tests exercise thresholds, invalid configuration, byte formatting, disabled swap,
counter reset behavior and elapsed-time rates. Hardware-specific GPU behavior and
panel placement require a real COSMIC session on the target hardware.

Support development through [GitHub Sponsors](https://github.com/sponsors/IllyaYalovyy).
