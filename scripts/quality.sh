#!/usr/bin/env bash
set -euo pipefail
cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.."
./scripts/check-version.py
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked --all-targets
for script in scripts/*.sh; do bash -n "$script"; done
if command -v desktop-file-validate >/dev/null; then
    desktop-file-validate data/*.desktop
fi
