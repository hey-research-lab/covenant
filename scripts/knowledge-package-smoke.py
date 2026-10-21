#!/usr/bin/env python3
"""Offline acceptance for an unpacked Covenant Compute knowledge bundle.

Uses Python's standard library and the packaged buyer. Source is copied into a
temporary directory and never executed. No wallet, daemon or model key is used.
This checks a local artifact; it does not establish publication or deployment.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import re
import shutil
import subprocess
import sys
import tempfile


TOOLS = {
    "knowledge.search", "knowledge.get", "knowledge.evidence",
    "knowledge.dependencies", "knowledge.changes", "knowledge.status",
}
PARSER_PACKAGES = {
    "tree-sitter", "tree-sitter-rust", "tree-sitter-javascript",
    "tree-sitter-typescript", "tree-sitter-python",
}


def require(condition: bool, message: str) -> None:
    if not condition:
        raise ValueError(message)


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def file_manifest(directory: Path) -> dict[str, str]:
    return {
        path.relative_to(directory).as_posix(): sha256(path.read_bytes())
        for path in sorted(directory.rglob("*")) if path.is_file()
    }


def check_licenses(directory: Path, lockfile: Path | None) -> list[str]:
    manifest = json.loads((directory / "sources.json").read_text())
    require(manifest["format_version"] == 1, "unsupported license manifest")
    rows = manifest["licenses"]
    names = {row["package"] for row in rows}
    require(PARSER_PACKAGES | {"tree-sitter Unicode data", "Archivo", "JetBrainsMono"}
            <= names, "parser, Unicode, or embedded font license is missing")
    require(len(names) == len(rows), "duplicate license manifest entry")
    checked = []
    for row in rows:
        path = directory / row["file"]
        require(path.resolve().parent == directory.resolve(), "license path leaves its directory")
        require(sha256(path.read_bytes()) == row["sha256"], f"license hash differs: {row['file']}")
        if row["package"] in PARSER_PACKAGES:
            require(re.fullmatch(r"[a-f0-9]{40}", row["revision"]) is not None,
                    f"license is not revision pinned: {row['package']}")
            require(row["revision"] in row["source_url"], "license URL is not revision pinned")
        checked.append(row["file"])
    if lockfile:
        # Cargo.lock uses fixed [[package]] records; avoid an extra TOML package
        # so the standalone smoke also works with Python 3.9 on developer Macs.
        locked = {}
        for block in lockfile.read_text().split("[[package]]")[1:]:
            name = re.search(r'^name = "([^"]+)"$', block, re.MULTILINE)
            version = re.search(r'^version = "([^"]+)"$', block, re.MULTILINE)
            if name and version:
                locked.setdefault(name[1], set()).add(version[1])
        for row in rows:
            if row["package"] in PARSER_PACKAGES:
                require(locked.get(row["package"]) == {row["version"]},
                        f"update packaged licenses for locked {row['package']}")
    return checked


def check_evidence(evidence: dict, source: Path, snapshot: str, object_id: str) -> None:
    require(evidence["snapshot_id"] == snapshot, "citation names a different snapshot")
    require(evidence["object_id"] == object_id, "citation names a different object")
    relative = PurePosixPath(evidence["path"])
    require(not relative.is_absolute() and ".." not in relative.parts,
            "citation path is not relative to the project")
    path = source.joinpath(*relative.parts)
    require(path.resolve().is_relative_to(source.resolve()), "citation leaves source root")
    data = path.read_bytes()
    start, end = evidence["start_byte"], evidence["end_byte"]
    require(0 <= start < end <= len(data), "citation byte range is invalid")
    require(not evidence.get("truncated", False), "example citation was unexpectedly truncated")
    require(sha256(data) == evidence["source_sha256"], "citation source hash differs")
    require(data[start:end].decode("utf-8") == evidence["fragment"], "citation bytes differ")
    require(evidence["start_line"] == data[:start].count(b"\n") + 1, "citation start line differs")
    require(evidence["end_line"] >= evidence["start_line"], "citation end line is invalid")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--package", type=Path, help="Unpacked release root")
    parser.add_argument("--binary", type=Path, help="Development buyer binary (instead of --package)")
    parser.add_argument("--examples", type=Path, help="Original examples/knowledge (with --binary)")
    parser.add_argument("--licenses", type=Path, help="License directory (with --binary)")
    parser.add_argument("--lockfile", type=Path, help="Check pinned license versions against Cargo.lock")
    parser.add_argument("--rosetta", action="store_true", help="Run only the buyer under arch -x86_64")
    args = parser.parse_args()
    if args.package:
        require(not any((args.binary, args.examples, args.licenses)),
                "--package cannot be combined with --binary, --examples or --licenses")
        package = args.package.resolve(strict=True)
        binary, examples = package / "covenant-compute", package / "examples/knowledge"
        licenses = package / "licenses/knowledge"
        for relative in ("docs/compute-knowledge.md", "docs/compute-knowledge-context-recipe.md",
                         "docs/compute-knowledge-remote-acceptance.md", "NOTICE.md",
                         "tools/knowledge-package-smoke.py"):
            require((package / relative).is_file(), f"missing packaged file: {relative}")
        mode = "unpacked-package"
    else:
        require(args.binary is not None and args.examples is not None and args.licenses is not None,
                "use --package, or supply --binary, --examples and --licenses")
        binary, examples, licenses = args.binary, args.examples, args.licenses
        mode = "development-binary"
    binary, examples = binary.resolve(strict=True), examples.resolve(strict=True)
    license_files = check_licenses(licenses.resolve(strict=True), args.lockfile)
    original_manifest = file_manifest(examples)
    require({"README.md", "python/orders.py", "rust/src/lib.rs", "web/orders.ts",
             "web/OrderCard.tsx", "web/format.js"} <= original_manifest.keys(),
            "original multi-language example is incomplete")
    checks = ["packaged-licenses", "original-example-files"]
    with tempfile.TemporaryDirectory(prefix="covenant-knowledge-smoke-") as temporary:
        root = Path(temporary)
        source, store = root / "source", root / "store"
        shutil.copytree(examples, source)
        # Deliberately do not inherit credentials, config or provider variables.
        # HOME and the buyer directory do not exist and must remain absent.
        env = {
            "PATH": os.environ.get("PATH", "/usr/bin:/bin"),
            "HOME": str(root / "unused-home"),
            "COVENANT_COMPUTE_MCP_HOME": str(root / "unused-buyer"),
            "COVENANT_COMPUTE_COORDINATOR_URL": "http://127.0.0.1:1",
            "LANG": "C.UTF-8",
        }
        prefix = (["/usr/bin/arch", "-x86_64"] if args.rosetta else []) + [str(binary)]

        def run(arguments: list[str], stdin: bytes | None = None) -> bytes:
            completed = subprocess.run(prefix + arguments, input=stdin, cwd=root, env=env,
                                       stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=30)
            require(completed.returncode == 0,
                    f"buyer failed for {arguments!r}: {completed.stderr.decode('utf-8', errors='replace')}")
            return completed.stdout

        def cli(*arguments: str) -> dict:
            return json.loads(run(["knowledge", *arguments, "--store", str(store), "--json"]))

        version = run(["--version"]).decode().strip()
        require(b"knowledge compile" in run(["knowledge", "--help"]), "knowledge help is absent")
        compiled = cli("compile", str(source))
        project, snapshot = compiled["project"]["id"], compiled["snapshot_id"]
        require(compiled["coverage"]["files"] == len(original_manifest), "example capture is incomplete")
        checks.extend(["offline-help", "offline-compile"])

        setup = cli("setup", str(source), "--query", "calculate_total")
        require(setup["status"] == "ready", "guided setup did not find a verified citation")
        require(setup["compile"]["project"]["id"] == project
                and setup["compile"]["snapshot_id"] == snapshot,
                "guided setup changed the project or unchanged snapshot")
        require(setup["configuration_written"] is False,
                "guided setup unexpectedly wrote client configuration")
        verified = setup["citation_verification"]
        require(verified["status"] == "verified", "setup citation is not verified")
        check_evidence(verified["citation"]["evidence"], source, snapshot,
                       verified["citation"]["object"]["id"])
        servers = setup["mcp_config"]["mcpServers"]
        require(len(servers) == 1, "guided setup must emit exactly one project-bound server")
        server = next(iter(servers.values()))
        require(Path(server["command"]).is_absolute()
                and Path(server["command"]).resolve() == binary,
                "MCP configuration does not name the installed executable")
        require(server["args"] == ["knowledge", "mcp", "--project", project,
                                    "--store", str(store.resolve())],
                "MCP configuration is not bound to the canonical store and project")
        checks.extend(["guided-setup", "guided-citation", "guided-mcp-config"])

        def query(*arguments: str) -> dict:
            return cli(*arguments, "--project", project)

        status = query("status")
        require(status["snapshot_id"] == snapshot, "status selected a different snapshot")
        require(status["freshness"]["state"] == "current", "fresh snapshot is not current")
        search = query("query", "calculate_total", "--limit", "50")
        require(search["results"], "example symbol search returned no objects")
        paths = {row["object"]["path"] for row in search["results"]}
        require({"python/orders.py", "rust/src/lib.rs", "web/orders.ts", "web/OrderCard.tsx"} <= paths,
                "search missed one of the example languages")
        for row in search["results"]:
            check_evidence(row["evidence"], source, snapshot, row["object"]["id"])
        selected = search["results"][0]["object"]["id"]
        detail = query("show", selected)
        check_evidence(detail["evidence"], source, snapshot, selected)
        require(query("verify", snapshot)["valid"] is True, "snapshot verification failed")
        warm = cli("compile", str(source))
        require(warm["snapshot_id"] == snapshot, "warm compile changed an unchanged source snapshot")
        require(warm["reused_files"] == len(original_manifest), "warm compile did not reuse example files")
        checks.extend(["status", "multi-language-search", "exact-source-citations", "show",
                       "snapshot-verification", "incremental-cache"])
        requests = [
            {"jsonrpc": "2.0", "id": 1, "method": "initialize",
             "params": {"protocolVersion": "2024-11-05", "capabilities": {},
                        "clientInfo": {"name": "package-smoke", "version": "1"}}},
            {"jsonrpc": "2.0", "method": "notifications/initialized"},
            {"jsonrpc": "2.0", "id": 2, "method": "tools/list"},
        ]
        calls = [
            ("knowledge.status", {}), ("knowledge.search", {"query": "calculate_total"}),
            ("knowledge.evidence", {"id": selected}), ("knowledge.get", {"id": selected}),
            ("knowledge.dependencies", {"id": selected}),
            ("knowledge.changes", {"from": snapshot, "to": snapshot}),
        ]
        for identifier, (name, arguments) in enumerate(calls, start=3):
            requests.append({"jsonrpc": "2.0", "id": identifier, "method": "tools/call",
                             "params": {"name": name, "arguments": arguments}})
        for identifier, (name, arguments) in enumerate([
            ("knowledge.compile", {}), ("knowledge.status", {"project": "other"}),
        ], start=9):
            requests.append({"jsonrpc": "2.0", "id": identifier, "method": "tools/call",
                             "params": {"name": name, "arguments": arguments}})
        # Execute the arguments emitted by setup, rather than reconstructing them.
        output = run(server["args"],
                     b"".join(json.dumps(request).encode() + b"\n" for request in requests))
        responses = [json.loads(line) for line in output.splitlines()]
        require([row["id"] for row in responses] == list(range(1, 11)),
                "MCP framing, request IDs or notification behavior differ")
        require(responses[0]["result"]["capabilities"]["tools"]["listChanged"] is False,
                "MCP initialization did not advertise fixed tools")
        tools = responses[1]["result"]["tools"]
        require({tool["name"] for tool in tools} == TOOLS and len(tools) == len(TOOLS),
                "MCP tool list differs from the six read-only tools")
        require(all(tool["annotations"]["readOnlyHint"] for tool in tools),
                "MCP tool is missing its read-only annotation")
        results = []
        for row in responses[2:8]:
            require(row["result"]["isError"] is False, f"MCP read failed: {row}")
            results.append(json.loads(row["result"]["content"][0]["text"]))
        require(results[0]["snapshot_id"] == snapshot, "MCP status selected a different snapshot")
        require(results[0]["freshness"]["state"] == "current", "MCP freshness differs")
        require(results[1]["results"], "MCP search returned no objects")
        check_evidence(results[2]["evidence"], source, snapshot, selected)
        check_evidence(results[3]["evidence"], source, snapshot, selected)
        require(results[4]["root"]["id"] == selected, "MCP dependency root differs")
        require(results[5]["modified"] == [] and results[5]["added"] == []
                and results[5]["removed"] == [], "unchanged snapshot diff is not empty")
        require(all(row["error"]["code"] == -32602 for row in responses[8:]),
                "MCP allowed mutation or a project override")
        checks.extend(["mcp-initialize", "mcp-notifications", "mcp-six-read-only-tools",
                       "mcp-all-six-reads", "mcp-citations", "mcp-no-mutation", "mcp-project-boundary"])
        require(file_manifest(source) == original_manifest, "read-only checks changed example source")
        changed_file = source / "python/pricing.py"
        original = changed_file.read_bytes()
        changed_file.write_bytes(original + b"\n# local freshness acceptance\n")
        stale = query("status")
        require(stale["freshness"]["state"] == "stale"
                and stale["freshness"]["changes"]["modified"] == 1,
                "status failed to detect an admitted source edit")
        require(stale["snapshot_id"] == snapshot, "status implicitly recompiled source")
        # Saved evidence is still bound to its captured bytes while source is stale.
        require(query("show", selected)["evidence"] == detail["evidence"],
                "live source edit changed saved evidence")
        changed_file.write_bytes(original)
        require(query("status")["freshness"]["state"] == "current",
                "restoring captured bytes did not restore current status")
        require(file_manifest(source) == original_manifest, "freshness test did not restore fixture")
        checks.extend(["mcp-emitted-config-execution", "freshness-current-stale-current",
                       "stale-source-preserves-citations"])
        require(not (root / "unused-home").exists() and not (root / "unused-buyer").exists(),
                "offline knowledge unexpectedly created buyer or HOME state")
        require(not any(root.rglob("identity.json")), "offline knowledge created an identity")
        checks.extend(["source-unchanged", "no-buyer-identity"])
        print(json.dumps({
            "result": "pass", "mode": mode, "buyer_version": version,
            "binary_sha256": sha256(binary.read_bytes()), "snapshot_id": snapshot,
            "example_files": len(original_manifest), "verified_citations": len(search["results"]) + 4,
            "license_files": license_files, "checks": checks,
            "scope": "Offline local artifact checks. No source execution, purchase, release or deployment.",
        }, indent=2))


if __name__ == "__main__":
    try:
        main()
    except (ValueError, KeyError, OSError, subprocess.SubprocessError) as error:
        print(f"knowledge package smoke failed: {error}", file=sys.stderr)
        sys.exit(1)
