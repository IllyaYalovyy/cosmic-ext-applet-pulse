#!/usr/bin/env bash
set -euo pipefail
cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.."
version=$(python3 -c 'import tomllib; print(tomllib.load(open("Cargo.toml", "rb"))["package"]["version"])')
output="$PWD/dist/$version"
(cd "$output" && sha256sum --check SHA256SUMS)
packages=("$output/"*.rpm)
[[ ${#packages[@]} == 1 && -f ${packages[0]} ]]
rpm -qpi "${packages[0]}"
rpm -qpl "${packages[0]}"
rpm -qpR "${packages[0]}"
[[ $(rpm -qp --queryformat '%{VERSION}' "${packages[0]}") == "$version" ]]
work=$(mktemp -d)
trap 'rm -rf -- "$work"' EXIT
rpm2cpio "${packages[0]}" | (cd "$work" && cpio -idm --quiet)
timeout 30 "$work/usr/bin/cosmic-ext-applet-pulse" --sample > "$output/sample.txt"
grep -q 'Snapshot {' "$output/sample.txt"
# Include evidence alongside the packages and cover every published asset.
cp -r target/graph-preview "$work/graph-preview"
tar -C "$work" -czf "$output/graph-preview.tar.gz" graph-preview
(cd "$output" && sha256sum ./*.rpm ./*.tar.gz sample.txt > SHA256SUMS && sha256sum --check SHA256SUMS)
