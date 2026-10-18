#!/usr/bin/env python3
"""Prepare an offline native-check record from the buyer's unchanged journal.

This adapter makes no network requests or purchases. It does not verify
signatures: pass its record to the public verify-public-receipt binary.
"""
import argparse
import base64
import json
import pathlib
import re
import subprocess
import tempfile
import uuid


def require(condition, message):
    if not condition:
        raise ValueError(message)


def read(path):
    return json.loads(path.read_text())


def write_new(path, value):
    with path.open("x", encoding="utf-8") as destination:
        json.dump(value, destination, indent=2)
        destination.write("\n")


def git(repo, *args):
    return subprocess.check_output(
        ["git", "-c", "core.hooksPath=/dev/null", "-c", "protocol.allow=never",
         "-c", "protocol.file.allow=always", "-C", str(repo), *args],
        text=True, stderr=subprocess.PIPE,
    ).strip()


def select_purchase(args):
    buyer = read(args.buyer)["pubkey"]
    # Pass the canonical coordinator URL used by the buyer, without a trailing /.
    expected_key = [buyer, args.coordinator.rstrip("/"), args.key]
    matches = []
    for path in sorted((args.buyer_home / "agent-purchases").glob("*/purchase.jsonl")):
        raw = path.read_bytes()
        require(not raw or raw.endswith(b"\n"), "incomplete purchase journal; preserve it and reconcile")
        selected = []
        for line in raw.splitlines():
            if not line.strip():
                continue
            entry = json.loads(line)
            if json.loads(entry["key"]) == expected_key:
                selected.append(entry)
        if selected:
            require(all(entry["envelope"] == selected[0]["envelope"] for entry in selected),
                    "journal changed this key's signed envelope")
            matches.append(selected[-1])
    require(len(matches) == 1, "expected exactly one journal for this buyer, endpoint and key")
    entry = matches[0]
    require(not entry.get("voided", False), "purchase was voided; do not infer a replacement")
    job_id = str(uuid.UUID(entry["envelope"]["payload"]["job_id"]))
    require(entry["envelope"]["payload"]["buyer"]["pubkey"] == buyer, "journal buyer differs")
    write_new(args.out, entry)
    print(job_id)


def prepare_record(args):
    purchase, buyer, preview, recovered = map(read, [args.purchase, args.buyer, args.preview, args.recovered])
    envelope = purchase["envelope"]
    payload = envelope["payload"]
    job_id = str(uuid.UUID(payload["job_id"]))
    key = json.loads(purchase["key"])
    require(isinstance(key, list) and len(key) == 3 and key[0] == buyer["pubkey"], "journal key differs")
    require(not purchase.get("voided", False), "purchase was voided")
    require(payload["buyer"]["pubkey"] == buyer["pubkey"], "buyer differs from saved identity")
    require(payload["kind"] == preview["kind"] == "agent_check", "unexpected job kind")
    require(preview["dry_run"] is True and preview["spends_nothing"] is True, "expected saved preview")
    require(payload["price_micro_usdc"] == preview["price_micro_usdc"] == 50000, "offer differs")
    require(payload["deadline_ms"] == preview["deadline_ms"] == 600000, "deadline differs")
    require(payload["capability_requirement"]["model_id"] == preview["checker_model"], "model differs")
    specs = [item["value"]["agent_check"] for item in payload["input"]
             if item.get("type") == "json" and "agent_check" in item.get("value", {})]
    require(len(specs) == 1, "expected exactly one native check specification")
    spec = specs[0]
    require(spec["task_job_id"] == job_id, "native check is not self-named")
    require(spec["repo"]["commit"] == preview["commit"], "source differs from preview")
    require(spec["acceptance"] == preview["acceptance"], "acceptance differs from preview")
    require(spec["patch_b64"] == "" and spec["patch_sha256"] == preview["patch_sha256"], "unexpected patch")
    require(spec.get("hidden") is None and spec.get("reproduction") is None, "unexpected overlay")
    require(recovered["job_id"] == job_id and recovered.get("receipt") is not None, "missing exact job receipt")
    if args.result is not None:
        result = read(args.result)
        require(result["job_id"] == job_id and result["envelope"] == envelope, "capture differs from journal")
        require(result["receipt"] == recovered["receipt"] and result["output"] == recovered["output"],
                "re-fetched signed result differs from original capture")
    commit = preview["commit"]
    require(re.fullmatch(r"[0-9a-f]{40}", commit) is not None, "invalid source commit")
    expected_tree = args.expected_tree.read_text().strip()
    require(expected_tree == "8c2ba88ee1f50b53462940d3f9f5e5457b319c6b", "unexpected public smoke tree")
    require(git(args.source_dir, "rev-parse", commit + "^{tree}") == expected_tree, "local source tree differs")
    source = spec["repo"]
    require(source["source"] == "bundle", "this tiny smoke expects an inline source bundle")
    require(len(source["bundle_b64"]) <= 16 * 1024 * 1024, "source bundle exceeds adapter limit")
    bundle = base64.b64decode(source["bundle_b64"], validate=True)
    with tempfile.TemporaryDirectory(prefix="covenant-smoke-source-") as temporary:
        folder = pathlib.Path(temporary)
        (folder / "source.bundle").write_bytes(bundle)
        git(folder, "init", "--bare", "--quiet", "repo.git")
        git(folder / "repo.git", "fetch", "--no-tags", str(folder / "source.bundle"), commit)
        require(git(folder / "repo.git", "rev-parse", commit + "^{tree}") == expected_tree,
                "signed bundle tree differs from prepared source")
        require(git(folder / "repo.git", "rev-list", "--count", commit) == "1", "snapshot includes history")
    write_new(args.out, {
        "envelope": envelope, "receipt": recovered["receipt"], "output": recovered["output"],
        "expected": {"job_id": job_id, "buyer_pubkey": buyer["pubkey"],
                     "signed_repo_commit": commit, "tree_hash": expected_tree, "role": "canary"},
    })
    print("Prepared unchanged signed data and expected metadata; run verify-public-receipt next.")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    purchase = commands.add_parser("purchase", help="read the exact existing purchase journal; print its job ID")
    purchase.add_argument("--buyer-home", type=pathlib.Path, required=True)
    purchase.add_argument("--buyer", type=pathlib.Path, required=True)
    purchase.add_argument("--coordinator", required=True)
    purchase.add_argument("--key", required=True)
    purchase.add_argument("--out", type=pathlib.Path, required=True)
    record = commands.add_parser("record", help="prepare input for the unchanged public offline verifier")
    for name in ["purchase", "buyer", "preview", "recovered", "source-dir", "expected-tree", "out"]:
        record.add_argument("--" + name, type=pathlib.Path, required=True)
    record.add_argument("--result", type=pathlib.Path, help="also compare complete original check stdout, if available")
    args = parser.parse_args()
    (select_purchase if args.command == "purchase" else prepare_record)(args)


if __name__ == "__main__":
    main()
