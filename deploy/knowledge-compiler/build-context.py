#!/usr/bin/env python3
"""Emit a bounded, tracked-source-only Docker context to stdout.

Keep the original workspace and lockfile: dropping workspace members changes
Cargo's locked resolution. Only Rust source and Cargo manifests are admitted;
local keys, environment files, build outputs and repository metadata never enter
the archive. Run from any directory; the script resolves its repository root.
"""
import io
import json
import pathlib
import subprocess
import sys
import tarfile


def main():
    root = pathlib.Path(__file__).resolve().parents[2]
    tracked = subprocess.check_output(
        ["git", "-C", str(root), "ls-files", "-z", "--", "agent-os"]
    ).decode("utf-8").split("\0")
    names = sorted(
        name
        for name in tracked
        if name
        and (
            name.endswith(".rs")
            or pathlib.PurePosixPath(name).name in ("Cargo.toml", "Cargo.lock")
        )
    )
    names.extend([
        "deploy/knowledge-compiler/Dockerfile",
        "deploy/knowledge-compiler/Dockerfile.dockerignore",
    ])
    total = 0
    inputs = []
    import hashlib

    with tarfile.open(fileobj=sys.stdout.buffer, mode="w|gz") as archive:
        for name in names:
            path = root / name
            if path.is_symlink() or not path.is_file():
                raise RuntimeError("build input is not a regular file: " + name)
            # A tracked source path cannot escape through a symlinked parent.
            if path.resolve() != path.absolute():
                raise RuntimeError("build input traverses a symlink: " + name)
            content = path.read_bytes()
            total += len(content)
            if len(content) > 4 * 1024 * 1024 or total > 32 * 1024 * 1024:
                raise RuntimeError("build source exceeds its size bound")
            info = tarfile.TarInfo(name)
            info.size = len(content)
            info.mode = 0o644
            info.mtime = 0
            archive.addfile(info, io.BytesIO(content))
            inputs.append({
                "path": name,
                "bytes": len(content),
                "sha256": hashlib.sha256(content).hexdigest(),
            })
    print(json.dumps({
        "schema": "covenant.knowledge.container-build-inputs.v1",
        "bytes": total,
        "inputs": inputs,
    }, sort_keys=True), file=sys.stderr)


if __name__ == "__main__":
    main()
