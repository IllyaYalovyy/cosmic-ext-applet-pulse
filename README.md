# Pulse for COSMIC

A configurable Rust resource monitor for the COSMIC panel and dock. Each resource
has an icon and narrow vertical meters; CPU has one thin bar per logical core.
Click to open readings, graph controls, About, System Monitor and a donation link. All settings persist through cosmic-config
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

Green at or below 50%, yellow above 50% through 80%, red above 80%, configurable. Rates use configurable
full scales (default network 100 MiB/s, disk 500 MiB/s); red means high load relative
to your scale, not a hardware fault. Gray with a dash means unavailable. The fill
height carries the same information as color. Vertical panels wrap cores to fit the panel width. Very high core counts can occupy substantial horizontal panel space;
CPU can be hidden independently.

## Graphs (0.3.0)

Choose **Cores**, **History**, or **Both** from the popup's **CPU graph** controls.
Cores remains the default. At small panel sizes the SVG geometry matches its actual
widget size: thin core bars have a one-pixel gap and no hidden aspect-ratio padding.
History scrolls from right to left over the last two minutes:

- The filled area and solid trace show average utilization across logical CPUs.
- The thin dashed trace and latest-point marker show the busiest logical CPU.
  One core at 100% therefore produces a red high trace even if the average is low.
- The popup and hover summary include both average and busiest-core percentages.

**Peak afterglow** retains a faint colored trail and cap above falling RAM, swap,
network, disk and GPU bars. The previous peak fades over eight seconds; a new high
renews it. At slow polling rates, its lifetime is at least two sampling periods so
one falling sample still shows the trail without extra wakeups. Unavailable metrics clear their trails. Turning it off removes trails
immediately. Both popup controls save automatically.

Efficiency: graphs update only with sensor samples (two seconds by default), never
on an animation timer. Peak fading therefore advances in sample-sized steps.
There are no blur filters, background animation tasks, new crates, or extra sensor
reads. Peaks shorter than the sampling interval can still be missed. History is bounded to 121 points; SVG/icon handles are cached and reused
between changes. Sampling pauses, failures and CPU topology changes clear obsolete
history. Changing interface/mount/rate scales clears the old peak references.

Additional cosmic-config keys (same `v1` directory; existing preferences survive):

| Key | Default | Allowed values |
| --- | --- | --- |
| `cpu_graph` | `Cores` | `Cores`, `History`, `Both` (RON enum, unquoted) |
| `peak_glow` | `true` | `true`, `false` |
| `glow_seconds` | `8` | 1–60 seconds |
| `history_seconds` | `120` | 30–120 seconds |
| `history_width` | `64` | 32–160 logical pixels (horizontal panel) |

Vertical panels wrap core bars to their width and use a narrow, taller history
chart. Graph controls do not change the sampling cadence.

## Fedora RPM

Download the Fedora RPM and `SHA256SUMS` from
[GitHub Releases](https://github.com/IllyaYalovyy/cosmic-ext-applet-pulse/releases).
The initial RPM targets Fedora 45 x86_64. Verify the downloaded RPM's checksum,
then install it using DNF:

```sh
sha256sum --check --ignore-missing SHA256SUMS
sudo dnf install ./cosmic-ext-applet-pulse-0.3.0-1.fc45.x86_64.rpm
```

Add **Pulse** in COSMIC Settings → Desktop → Panel or Dock → Applets. An already
running instance may need to be removed from the panel and added again after an
upgrade. These are upstream RPMs, not packages from Fedora's official repositories.
See [the manual release guide](packaging/README.md) for building and publishing.

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

Every coherent change set increments the version: major for incompatible changes,
minor for new compatible features, patch for fixes and maintenance (including docs).
Update Cargo.toml, Cargo.lock, AppStream metadata and [CHANGELOG.md](CHANGELOG.md)
together. This policy also applies before 1.0.

GitHub Actions checks every branch push and pull request: formatting, strict Clippy,
all debug/release tests (including graph previews and timing), metadata/script
validation, a Fedora 45 x86_64 RPM build, and a packaged sensor smoke test. Build
artifacts are retained for 30 days. Successful main-branch pushes also publish
per-commit GitHub prereleases (`ci-<full SHA>`), without replacing stable releases.
A multi-commit push builds its tip commit. Stable versioned releases remain explicit.
See [packaging](packaging/README.md) for details and local reproduction.

For graph development, `cargo test benchmark_graph_preparation -- --ignored --nocapture`
measures CPU-side graph preparation and cached view lookup (not full renderer/GPU
or sensor cost). `PULSE_PREVIEW_DIR=/tmp/pulse-preview cargo test export_graph_previews
-- --ignored` exports SVG examples from the production renderer.
