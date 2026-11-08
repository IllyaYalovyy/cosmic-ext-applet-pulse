# Manual Fedora releases

Start with an upstream RPM attached to a GitHub release. A COPR repository is a
useful next step for automatic DNF updates and builds on multiple Fedora releases;
no COPR project or CI/CD is configured here. The initial target is Fedora 45 x86_64.
Build on each supported Fedora release/architecture rather than assuming the
binary is compatible with older glibc releases. ARM64 is permitted by the spec but
must be built and tested separately before advertising support.

## Build

Install `rpm-build`, Rust/Cargo (at least the manifest's MSRV), Python 3.11+, Git,
GCC, pkg-config, and the native development packages listed in the main README
(on Fedora: `wayland-devel libxkbcommon-devel systemd-devel libinput-devel
libseat-devel mesa-libEGL-devel fontconfig-devel dbus-devel`).

1. Increment MAJOR for incompatible changes, MINOR for compatible features, or
   PATCH for maintenance/fixes. Update Cargo.toml, Cargo.lock, AppStream and
   CHANGELOG.md together. Include docs/packaging changes in this policy.
2. Review and commit the complete change set with the required sole author.
3. Run `./scripts/build-release.sh` (or `just release`).
4. Inspect `dist/<version>/`: the RPM, project source, dependency sources, and
   SHA256SUMS. The script runs the quality gate before building.

The RPM spec packages the locally compiled release executable and its metadata
under `/usr`, leaving user settings alone. RPM's ELF scanner generates explicit linked-library
requirements, including glibc symbol versions; the spec additionally requires cosmic-panel and the dynamically
loaded Wayland client library. NVIDIA's proprietary NVML library is optional,
so the RPM does not require a proprietary driver. This is an **upstream binary
RPM**, not a Fedora-reviewed dist-git package or source RPM. Build it only on the
Fedora release for which you will distribute it. No sudo or system installation
is needed for the build.

Cargo.lock fixes dependency revisions. A separate source archive preserves exact
Cargo dependency source trees and a provenance inventory; license notices are
included in the RPM. Because this libcosmic graph references the same git revision
through different source IDs, `cargo vendor` currently fails on duplicate crate
versions. The dependency archive is therefore **not** a Cargo vendor directory.
To rebuild normally, extract the project source and run `cargo build --release
--locked` with access to the locked dependencies. The separate dependency archive
provides the matching source for inspection and license compliance.

`packaging/pulse.spec.in` gets its version and aggregate license expression from
the build script. Generated files live in ignored `target/` and `dist/` directories.
SHA-256 hashes detect corruption; they do not replace a cryptographic signature.
These initial RPMs are unsigned. Repository signing and distribution review are
follow-up work for a COPR or official Fedora package.

## Verify and publish

```sh
rpm -qpi dist/<version>/*.rpm
rpm -qpl dist/<version>/*.rpm
rpm -qpR dist/<version>/*.rpm
(cd dist/<version> && sha256sum --check SHA256SUMS)
```

Extract the RPM into a temporary directory (`rpm2cpio ... | cpio -idm`) and run
its executable with `--sample`. Test panel/dock placement in a COSMIC session.
On a Fedora host, `sudo dnf install ./dist/<version>/*.rpm` installs it; users
must still add Pulse in COSMIC Settings. Installation and panel changes are not
performed by the build script.

Publish manually after the artifacts and release notes are reviewed:

```sh
git tag -a v<version> -m 'Pulse <version>'
git push origin HEAD
git push origin v<version>
gh release create v<version> dist/<version>/* \
  --verify-tag --title 'Pulse <version>' --notes-file /path/to/release-notes.md
```

Do not overwrite an existing tag or release. Correct a published build with a
new patch version. Never include credentials or local paths in release artifacts.
