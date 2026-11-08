#!/usr/bin/env python3
"""Preserve exact dependency sources and license notices from Cargo metadata.

Unlike cargo vendor, this handles the same git revision resolved through two
source IDs. The result is a source/notice archive, not a Cargo source replacement.
"""
import hashlib
import json
from pathlib import Path
import re
import shutil
import sys

metadata = json.loads(Path(sys.argv[1]).read_text())
output = Path(sys.argv[2])
licenses = Path(sys.argv[3])
output.mkdir(parents=True, exist_ok=True)
licenses.mkdir(parents=True, exist_ok=True)
rows = []
roots = {}
expressions = set()


def license_files(directory):
    found = []
    for path in directory.iterdir():
        if path.name.upper().startswith(("LICENSE", "LICENCE", "COPYING", "NOTICE", "COPYRIGHT", "UNLICENSE")):
            if path.is_file():
                found.append(path)
            elif path.is_dir():
                found.extend(p for p in path.rglob("*") if p.is_file())
    return found


for package in metadata["packages"]:
    if package["source"] is None:
        continue
    package_root = Path(package["manifest_path"]).parent
    source_root = package_root
    if package["source"].startswith("git+"):
        # Preserve the whole git workspace, including sibling crates and submodules.
        source_root = next(
            p for p in (package_root, *package_root.parents)
            if p.parent.parent.name == "checkouts"
        )
    if source_root not in roots:
        key = hashlib.sha256(package["source"].encode()).hexdigest()[:12]
        folder = f"{source_root.name}-{key}"
        roots[source_root] = folder
        shutil.copytree(
            source_root, output / folder,
            ignore=shutil.ignore_patterns(".git", "target", "__pycache__"),
            symlinks=True,
        )
    folder = roots[source_root]
    notices = license_files(package_root)
    parent = package_root
    while parent != source_root:
        parent = parent.parent
        notices.extend(license_files(parent))
    if package.get("license_file"):
        notices.append(package_root / package["license_file"])
    notices = list(dict.fromkeys(notices))
    # Some crates keep the notice in their README rather than a separate license.
    if not notices:
        notices = sorted(p for p in source_root.glob("README*") if p.is_file())
        notices.append(package_root / "Cargo.toml")
        if package["license"] == "GPL-3.0-only":
            notices.append(Path(__file__).resolve().parent.parent / "LICENSE")
    expression = package["license"]
    if not expression:
        for notice in notices:
            text = notice.read_text(errors="replace")
            if "Mozilla Public License" in text:
                expression = "MPL-2.0"
            elif "GNU General Public License" in text:
                expression = "GPL-3.0-only"
            elif "Permission is hereby granted, free of charge" in text:
                expression = "MIT"
            elif "This is free and unencumbered software" in text:
                expression = "Unlicense"
            if expression:
                break
        if not expression:
            raise SystemExit(f"Review undeclared license for {package['name']}")
    expression = re.sub(r"\s*/\s*", " OR ", expression)
    expressions.add(expression)
    identity = f"{package['name']}-{package['version']}-{hashlib.sha256(package['id'].encode()).hexdigest()[:8]}"
    destination = licenses / identity
    destination.mkdir()
    for index, path in enumerate(notices):
        shutil.copy2(path, destination / f"{index}-{path.name}")
    rows.append({
        "name": package["name"], "version": package["version"],
        "source": package["source"], "license": expression,
        "declared_license": package["license"],
        "source_directory": str(Path(folder) / package_root.relative_to(source_root)),
        "notices_directory": identity,
    })

report = json.dumps(rows, indent=2) + "\n"
(output / "DEPENDENCIES.json").write_text(report)
(licenses / "DEPENDENCIES.json").write_text(report)
(licenses / "README.txt").write_text(
    "Dependency license notices and source provenance for this build.\n"
    "The matching dependency-sources release archive contains full source trees.\n"
    "The inventory conservatively includes build dependencies as well as runtime code.\n"
    "A null declared_license means the expression was identified from upstream notices.\n"
)
# Conservative expression covering the complete bundled dependency inventory.
expressions.discard("GPL-3.0-only")
print("GPL-3.0-only AND " + " AND ".join(f"({e})" for e in sorted(expressions)))
