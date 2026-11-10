# Changelog

## 0.3.3 — 2026-11-09

- Fix missing afterglow on CPU core bars, sharing the existing sample-driven peak tracking and fade.
- Clear CPU peaks when the core count changes and test per-core fading, rendering, and the glow toggle.

## 0.3.2 — 2026-11-09

- Run quality checks and all headless debug/release tests in GitHub Actions.
- Build and verify Fedora 45 RPMs for every push and pull request.
- Publish immutable commit-named prereleases for successful main-branch pushes.
- Include graph previews, packaged sensor smoke-test output and checksums in CI artifacts.
- Fix shell syntax checks to inspect every script.

## 0.3.1 — 2026-11-08

- Clear sampler failure messages when readings resume without erasing other errors.
- Share network/disk counter rate handling and reject invalid elapsed times.
- Expand regression tests for counter resets, malformed diskstats, graph bounds/cache invalidation, missing sensors, and application state transitions.

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
