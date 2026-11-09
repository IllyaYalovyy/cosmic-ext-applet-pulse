#!/usr/bin/env bash
# Additional headless checks, including the normally ignored preview and timing tests.
set -euo pipefail
cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.."
export PULSE_PREVIEW_DIR="$PWD/target/graph-preview"
cargo test --locked --all-targets -- --include-ignored
cargo test --locked --release --all-targets -- --include-ignored
appstreamcli validate --no-net data/*.metainfo.xml
python3 - <<'PY'
from pathlib import Path
import xml.etree.ElementTree as ET
for path in Path('scripts').glob('*.py'):
    compile(path.read_text(), str(path), 'exec')
previews = list(Path('target/graph-preview').glob('*.svg'))
assert previews, 'No graph previews were produced'
for path in previews:
    root = ET.parse(path).getroot()
    assert root.tag == '{http://www.w3.org/2000/svg}svg', path
PY
