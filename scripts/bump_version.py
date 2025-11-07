#!/usr/bin/env python3
import re
import sys
from pathlib import Path

USAGE = "Usage: bump_version.py [major|minor|patch]"

def bump(ver: str, kind: str) -> str:
    m = re.match(r"^(\d+)\.(\d+)\.(\d+)$", ver.strip())
    if not m:
        raise ValueError(f"Unsupported version format: {ver}")
    major, minor, patch = map(int, m.groups())
    if kind == "major":
        major += 1; minor = 0; patch = 0
    elif kind == "minor":
        minor += 1; patch = 0
    elif kind == "patch":
        patch += 1
    else:
        raise ValueError(f"Invalid bump kind: {kind}")
    return f"{major}.{minor}.{patch}"

def patch_lockfile(lock_path: Path, crate_name: str, new_ver: str) -> bool:
    if not lock_path.exists():
        return False
    text = lock_path.read_text(encoding="utf-8")
    # Iterate over [[package]] blocks and update version for our crate
    pkg_iter = list(re.finditer(r"(?ms)^\[\[package\]\](.*?)(?=^\[\[|\Z)", text))
    changed = False
    new_text = text
    offset = 0
    for m in pkg_iter:
        block = m.group(1)
        block_start, block_end = m.span(1)
        if re.search(rf"(?m)^name\s*=\s*\"{re.escape(crate_name)}\"\s*$", block):
            # Replace version in this block
            def repl(ver_m):
                return f"{ver_m.group(1)}\"{new_ver}\""
            updated_block = re.sub(r"(?m)^(version\s*=\s*)\"([0-9]+\.[0-9]+\.[0-9]+)\"\s*$", repl, block, count=1)
            if updated_block != block:
                new_text = new_text[:block_start+offset] + updated_block + new_text[block_end+offset:]
                offset += len(updated_block) - len(block)
                changed = True
                # Do not break; handle duplicates if any
    if changed:
        lock_path.write_text(new_text, encoding="utf-8")
    return changed

def main():
    if len(sys.argv) != 2:
        print(USAGE, file=sys.stderr)
        sys.exit(2)
    kind = sys.argv[1].lower()
    root = Path(__file__).resolve().parents[1]
    cargo_toml = root / "Cargo.toml"
    text = cargo_toml.read_text(encoding="utf-8")

    # Find [package] section and version line
    pkg_match = re.search(r"(?ms)^\[package\](.*?)(^\[|\Z)", text)
    if not pkg_match:
        raise SystemExit("[package] section not found in Cargo.toml")
    pkg_block = pkg_match.group(1)
    ver_match = re.search(r"(?m)^version\s*=\s*\"([0-9]+\.[0-9]+\.[0-9]+)\"", pkg_block)
    if not ver_match:
        raise SystemExit("version field not found in [package]")
    current_ver = ver_match.group(1)
    new_ver = bump(current_ver, kind)

    # Replace only the matched version line inside the [package] block
    start, end = pkg_match.span(1)
    new_pkg_block = re.sub(r"(?m)^(version\s*=\s*)\"([0-9]+\.[0-9]+\.[0-9]+)\"",
                           rf"\\1\"{new_ver}\"", pkg_block, count=1)
    new_text = text[:start] + new_pkg_block + text[end:]
    cargo_toml.write_text(new_text, encoding="utf-8")

    # Print the new version (capturable by GitHub Actions)
    # Try to also patch Cargo.lock for this crate
    lock_changed = patch_lockfile(root / "Cargo.lock", crate_name="hashicorp-downloader", new_ver=new_ver)
    if lock_changed:
        print(f"Lockfile updated to {new_ver}")
    print(new_ver)

if __name__ == "__main__":
    main()
