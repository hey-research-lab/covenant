#!/usr/bin/env python3
"""Fail closed unless a Knowledge landing candidate preserves the public site."""
import argparse
import hashlib
import json
import subprocess
from pathlib import Path

parser = argparse.ArgumentParser()
parser.add_argument("--baseline", required=True)
parser.add_argument("--candidate", help="Commit; omit for working tree and untracked files")
args = parser.parse_args()

def git(*argv):
    return subprocess.check_output(["git", *argv])

base = git("rev-parse", args.baseline + "^{commit}").decode().strip()
if args.candidate:
    candidate = git("rev-parse", args.candidate + "^{commit}").decode().strip()
    changes = git("diff", "--name-status", base, candidate).decode().splitlines()
    def read(path):
        return git("show", candidate + ":" + path)
else:
    candidate = "working-tree"
    changes = git("diff", "--name-status", base).decode().splitlines()
    changes += [
        "A\t" + path
        for path in git("ls-files", "--others", "--exclude-standard").decode().splitlines()
    ]
    def read(path):
        return Path(path).read_bytes()

prefixes = (
    "landing/app/compute-knowledge/",
    "landing/public/compute-knowledge/",
    "landing/app/og/compute-knowledge.png/",
    "docs/evidence/knowledge/site-preview-2026-10-20/",
)
exact = {
    "landing/app/_brand.tsx", "landing/app/sitemap.ts",
    "scripts/check-knowledge-site-baseline.py",
    "package.json", "pnpm-lock.yaml",
    "landing/public/install/compute.sh",
}
errors, paths = [], []
for line in changes:
    status, path = line.split("\t", 1)
    paths.append(path)
    if status not in ("A", "M"):
        errors.append("Deletion/rename/unexpected status: " + line)
    if path not in exact and not path.startswith(prefixes):
        errors.append("Outside Knowledge page scope: " + path)

shared = {
    "landing/app/_brand.tsx": (
        '      { label: "Compute", href: "/compute" },',
        '\n      { label: "Compute Knowledge (preview)", href: "/compute-knowledge" },',
    ),
    "landing/app/sitemap.ts": (
        '    { url: `${SITE}/compute`, lastModified, changeFrequency: "weekly", priority: 0.9 },',
        '\n    { url: `${SITE}/compute-knowledge`, lastModified: "2026-10-20", changeFrequency: "monthly", priority: 0.6 },',
    ),
}
for path, (anchor, addition) in shared.items():
    old = git("show", base + ":" + path).decode()
    expected = old if addition.strip() in old else old.replace(anchor, anchor + addition)
    if path == "landing/app/_brand.tsx" and 'href: "/compute-knowledge"' in old:
        expected = old
    allowed = (old, expected)
    if path == "landing/app/_brand.tsx":
        allowed += (expected.replace("Compute Knowledge (preview)", "Compute Knowledge"),)
    if read(path).decode() not in allowed:
        errors.append("Unrelated shared site change: " + path)

# This installer body and its four archive pins were independently checked
# against the signed compute-v0.5.0 and compute-v0.5.1 releases. Other installers
# are rejected; the 0.5.1 change alters only the five release-pin assignments.
installer = "landing/public/install/compute.sh"
installer_hash = hashlib.sha256(read(installer)).hexdigest()
baseline_installer_hash = hashlib.sha256(git("show", base + ":" + installer)).hexdigest()
if installer_hash not in (
    baseline_installer_hash,
    "18883da2515ea243ef7c95a92c1fac3a60f8c30f55d25df17c17c688efa0b0ef",
    "74fdf1d23c38597d0ada3015f47a71a86e87cc07c43ff1e1e89a72627dffc9dc",
):
    errors.append("Installer differs from baseline or verified Compute 0.5.0/0.5.1")

# Production's workspace lock omitted landing's direct sharp dependency, which
# already resolves to 0.35.5 through the existing root override and snapshot.
# Preserve that importer repair, and admit the exact reviewed paired dependency
# fix for GHSA-q3jh-wrc6-jgq8 / GHSA-8r9p-f939-6h3c (follow-redirects 1.16.1).
lock = git("show", base + ":pnpm-lock.yaml").decode()
before, landing = lock.split("\n  landing:\n", 1)
landing, after = landing.split("\n  packages/config:\n", 1)
anchor = "      tailwind-merge:\n"
addition = (
    "      sharp:\n"
    "        specifier: 0.35.5\n"
    "        version: 0.35.5(@types/node@25.6.0)\n"
)
fixed_landing = landing if addition in landing else landing.replace(anchor, addition + anchor, 1)
fixed_lock = before + "\n  landing:\n" + fixed_landing + "\n  packages/config:\n" + after
def digest(data):
    return hashlib.sha256(data).hexdigest()

baseline_package_hash = digest(git("show", base + ":package.json"))
dependency_pair = (digest(read("package.json")), digest(read("pnpm-lock.yaml")))
allowed_dependency_pairs = {
    (baseline_package_hash, digest(lock.encode())),
    (baseline_package_hash, digest(fixed_lock.encode())),
    (
        "8dcf7c3461dc2b43b6c6b3ebc7e1fb0b5776212d9bac2ab2020bb4f336f08f20",
        "a635a61c17d0ef9d45125b589dae81446fbdc210832b24f025cdceb96c8c3720",
    ),
}
if dependency_pair not in allowed_dependency_pairs:
    errors.append("Unreviewed package/lock pair")

# Existing pages and artwork remain outside the allowlist.
print(json.dumps({
    "baseline": base, "candidate": candidate, "pass": not errors,
    "changed_files": len(paths), "paths": paths, "errors": errors,
}, indent=2))
raise SystemExit(1 if errors else 0)
