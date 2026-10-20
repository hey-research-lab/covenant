#!/usr/bin/env python3
"""Validate an actual local archive and both installer paths without publication.

The website installer's HTTPS transport is stubbed with this exact local archive;
its candidate-only copy receives the archive checksum. Published pins are untouched.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import re
import subprocess
import tarfile
import tempfile


def sha(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--archive", required=True, type=Path)
    parser.add_argument("--public-installer", required=True, type=Path)
    args = parser.parse_args()
    archive = args.archive.resolve(strict=True)
    public = args.public_installer.read_text()
    checks = []
    with tempfile.TemporaryDirectory(prefix="covenant-install-check-") as temporary:
        root = Path(temporary)
        extracted = root / "extracted"
        extracted.mkdir()
        with tarfile.open(archive) as tar:
            members = tar.getmembers()
            assert sum(m.size for m in members) <= 1024 * 1024 * 1024, "archive exceeds 1 GiB"
            for item in members:
                path = PurePosixPath(item.name)
                assert not path.is_absolute() and ".." not in path.parts and "\\" not in item.name
                assert item.isdir() or item.isfile(), "archive contains a link or special file"
            tar.extractall(extracted)
        packages = list(extracted.iterdir())
        assert len(packages) == 1 and packages[0].is_dir(), "archive must have one package root"
        package = packages[0]
        commands = ["covenant-compute", "covenant-compute-mcp", "covenant-compute-node", "covenant-compute-stake"]
        versions = {}
        binaries = {}
        env = {"PATH": os.environ.get("PATH", "/usr/bin:/bin"), "HOME": str(root / "unused-home"),
               "PYTHONDONTWRITEBYTECODE": "1"}
        for command in commands:
            result = subprocess.run([str(package / command), "--version"], env=env, check=True,
                                    capture_output=True, text=True, timeout=10)
            versions[command] = result.stdout.strip()
            binaries[command] = sha(package / command)
        version = versions["covenant-compute"].removeprefix("covenant-compute ")
        assert all(versions[name] == f"{name} {version}" for name in commands)
        build = json.loads((package / "BUILD_INFO.json").read_text())
        assert build["binary_sha256"] == binaries and build["version"] == version
        checks.append("four-packaged-binary-versions")
        checks.append("build-manifest-binary-hashes")
        bin_dir = root / "prefix with spaces" / "bin"
        data_dir = root / "prefix with spaces" / "share" / "covenant-compute"
        install_env = dict(env, COVENANT_COMPUTE_INSTALL_DIR=str(bin_dir),
                           COVENANT_COMPUTE_DATA_DIR=str(data_dir))

        def install(script: Path, selected_env: dict) -> subprocess.CompletedProcess:
            return subprocess.run(["/bin/sh", str(script)], env=selected_env, cwd=root,
                                  capture_output=True, text=True, timeout=30)

        first = install(package / "install.sh", install_env)
        assert first.returncode == 0, first.stderr
        assert {name: sha(bin_dir / name) for name in commands} == binaries
        installed_data = data_dir / version
        for name in ["LICENSE", "NOTICE.md", "SECURITY_AUDIT_LICENSE", "RELEASE_NOTES.md", "BUILD_INFO.json"]:
            assert sha(installed_data / name) == sha(package / name)
        checks.extend(["offline-install-with-spaces", "installed-binary-hashes", "installed-notices"])
        smoke = subprocess.run(
            ["python3", str(package / "tools/knowledge-package-smoke.py"),
             "--binary", str(bin_dir / "covenant-compute"),
             "--examples", str(installed_data / "examples/knowledge"),
             "--licenses", str(installed_data / "licenses/knowledge")],
            env=env, capture_output=True, text=True, check=True, timeout=60)
        smoke_result = json.loads(smoke.stdout)
        checks.append("installed-offline-knowledge-smoke")
        repeated = install(package / "install.sh", install_env)
        assert repeated.returncode == 0, repeated.stderr
        checks.append("identical-reinstall")
        (installed_data / "NOTICE.md").write_text("deliberately changed fixture data")
        rejected = install(package / "install.sh", install_env)
        assert rejected.returncode != 0 and "already exists and differs" in rejected.stderr
        assert {name: sha(bin_dir / name) for name in commands} == binaries
        checks.append("conflicting-data-reinstall-preserves-binaries")

        # Exercise the real public script with only its download transport and
        # candidate pin replaced in a temporary copy. No public pin is advanced.
        stubs = root / "transport"
        stubs.mkdir()
        curl = stubs / "curl"
        curl.write_text('#!/bin/sh\nset -eu\nwhile [ "$#" -gt 0 ]; do\n'
                        'if [ "$1" = -o ]; then shift; cp "$LOCAL_ARCHIVE" "$1"; exit 0; fi\n'
                        'shift\ndone\nexit 2\n')
        curl.chmod(0o755)
        candidate = re.sub(r"^version=.*$", f"version={version}", public, flags=re.MULTILINE)
        candidate = re.sub(r"^(sha256_[a-z0-9_]+)=.*$", rf"\g<1>={sha(archive)}",
                           candidate, flags=re.MULTILINE)
        website_script = root / "candidate-install.sh"
        website_script.write_text(candidate)
        website_bin, website_data = root / "website-bin", root / "website-data"
        website_env = dict(env, PATH=str(stubs) + ":" + env["PATH"], LOCAL_ARCHIVE=str(archive),
                           COVENANT_COMPUTE_INSTALL_DIR=str(website_bin),
                           COVENANT_COMPUTE_DATA_DIR=str(website_data))
        website = install(website_script, website_env)
        assert website.returncode == 0, website.stderr
        assert {name: sha(website_bin / name) for name in commands} == binaries
        assert (website_data / version / "licenses/knowledge/sources.json").is_file()
        checks.append("website-installer-local-transport-candidate")
        website_script.write_text(candidate.replace(sha(archive), "0" * 64))
        bad_bin = root / "checksum-rejection-bin"
        bad = install(website_script, dict(website_env, COVENANT_COMPUTE_INSTALL_DIR=str(bad_bin)))
        assert bad.returncode != 0 and "does not match its pinned checksum" in bad.stderr
        assert not bad_bin.exists()
        assert not (root / "unused-home").exists(), [
            str(path.relative_to(root)) for path in (root / "unused-home").rglob("*")
        ]
        checks.extend(["website-checksum-mismatch-rejected", "no-wallet-or-home-state"])
        print(json.dumps({
            "result": "pass", "version": version, "archive": archive.name,
            "build": build,
            "archive_sha256": sha(archive), "archive_bytes": archive.stat().st_size,
            "binary_sha256": binaries, "checks": checks, "installed_smoke": smoke_result,
            "scope": "Native local candidate. Website transport stubbed with the exact local archive. No HTTPS release download, public pin update, signing, publication or deployment.",
        }, indent=2))


if __name__ == "__main__":
    main()
