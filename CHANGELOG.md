# Changelog

## 0.3.0 — 2026-11-08

- Fit CPU core bars to the actual panel size, removing excess SVG padding and tightening gaps.
- Add CPU history and combined graph modes, with average load and an independent busiest-core trace.
- Add fading peak trails for RAM, swap, network, disk and GPU meters.
- Cache graph/icon SVG handles and bound history to 121 samples; no new dependencies or animation timer.
- Add persistent popup controls for CPU graph mode and peak afterglow.

## 0.2.0 — 2026-11-08

- Add a manual Fedora RPM release workflow with source archives and SHA-256 checksums.
- Require a major, minor, or patch version bump for every coherent change set.
- Remove the scaffold CI workflow; builds and releases are manual for now.

## 0.1.0

- Initial COSMIC panel and dock resource monitor, including per-core CPU, memory,
  swap, network, disk, GPU and VRAM meters, configurable colors and persistent settings.
