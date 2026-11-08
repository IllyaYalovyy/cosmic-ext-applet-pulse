# Pulse development rules

- Every coherent change set must increment the package version before completion,
  including code, dependencies, packaging, documentation, and tooling changes.
  Bump once per change set, not once per edited file or intermediate correction.
- Use MAJOR for incompatible behavior or configuration changes, MINOR for
  backward-compatible features, and PATCH for fixes or maintenance. Reset lower
  components when increasing MAJOR or MINOR. Apply this policy before 1.0 too.
- Keep Cargo.toml, Pulse's entry in Cargo.lock, the latest AppStream release, and
  CHANGELOG.md synchronized. The RPM version must come from Cargo.toml.
- Run checks appropriate to the change. Run scripts/quality.sh for Rust changes
  and before a release. Build and inspect release artifacts before publishing.
- Releases are manual. Do not introduce CI/CD unless explicitly requested.
- Commit only as Illya Yalovyy
  <1623643+IllyaYalovyy@users.noreply.github.com>. Never add Co-authored-by trailers
  or bypass hooks. Do not commit generated artifacts, credentials, or build caches.
