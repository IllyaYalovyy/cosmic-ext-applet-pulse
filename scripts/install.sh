#!/usr/bin/env bash
set -euo pipefail
cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.."
prefix=${PREFIX:-"$HOME/.local"}
appid=io.github.illyayalovyy.Pulse
cargo build --release --locked
install -Dm755 "${CARGO_TARGET_DIR:-target}/release/cosmic-ext-applet-pulse" "$prefix/bin/cosmic-ext-applet-pulse"
# Absolute Exec also works when the panel does not inherit ~/.local/bin in PATH.
python3 - "$prefix" "$appid" <<'PY'
import pathlib, sys
prefix = pathlib.Path(sys.argv[1]).resolve()
appid = sys.argv[2]
binary = str(prefix / 'bin/cosmic-ext-applet-pulse')
escaped = binary.replace('\\', '\\\\').replace('"', '\\"').replace('`', '\\`').replace('$', '\\$').replace('%', '%%')
text = pathlib.Path(f'data/{appid}.desktop').read_text().replace('Exec=cosmic-ext-applet-pulse', f'Exec="{escaped}"')
dest = prefix / f'share/applications/{appid}.desktop'
dest.parent.mkdir(parents=True, exist_ok=True)
dest.write_text(text)
PY
install -Dm644 "data/$appid-symbolic.svg" "$prefix/share/icons/hicolor/scalable/apps/$appid-symbolic.svg"
install -Dm644 "data/$appid.metainfo.xml" "$prefix/share/metainfo/$appid.metainfo.xml"
printf 'Installed Pulse. Add it in COSMIC Settings → Desktop → Panel or Dock → Applets.\n'
