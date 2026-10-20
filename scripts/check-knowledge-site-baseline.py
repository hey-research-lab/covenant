#!/usr/bin/env python3
"""Fail closed unless a Knowledge landing candidate preserves the public site."""
import argparse
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
    if read(path).decode() not in (old, expected):
        errors.append("Unrelated shared site change: " + path)

# Existing installer pins, pages, lockfile and artwork are outside the allowlist.
print(json.dumps({
    "baseline": base, "candidate": candidate, "pass": not errors,
    "changed_files": len(paths), "paths": paths, "errors": errors,
}, indent=2))
raise SystemExit(1 if errors else 0)
