#!/usr/bin/env python3
"""Check that release metadata agrees with the authoritative Cargo version."""
from pathlib import Path
import re
import tomllib
import xml.etree.ElementTree as ET

root = Path(__file__).resolve().parent.parent
manifest = tomllib.loads((root / "Cargo.toml").read_text())["package"]
version = manifest["version"]
assert re.fullmatch(r"\d+\.\d+\.\d+", version), "Expected major.minor.patch"
lock = tomllib.loads((root / "Cargo.lock").read_text())
package = next(p for p in lock["package"] if p["name"] == manifest["name"])
assert package["version"] == version, "Cargo.lock version differs"
release = ET.parse(root / "data/io.github.illyayalovyy.Pulse.metainfo.xml").find("releases/release")
assert release is not None and release.attrib["version"] == version, "AppStream version differs"
heading = next(line for line in (root / "CHANGELOG.md").read_text().splitlines() if line.startswith("## "))
assert heading.startswith(f"## {version} "), "Changelog version differs"
print(f"Release metadata agrees: {version}")
