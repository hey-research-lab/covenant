#!/usr/bin/env python3
"""Reproduce a retrieval-only comparison on the original knowledge example.

No repository code, model, wallet, daemon, provider, or paid job is executed.
The compiler receives a copied source corpus outside Git, and both retrieval
commands read exactly that corpus. Raw outputs and timings are retained.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import statistics
import subprocess
import tempfile
import time
from datetime import datetime, timezone

CASES = [
    {"id": "total-symbol", "question": "Where is calculate_total defined or called?", "query": "calculate_total",
     "target_paths": ["python/orders.py", "web/orders.ts", "web/OrderCard.tsx", "rust/src/lib.rs"]},
    {"id": "unit-price", "question": "Where does the unit_price spelling appear?", "query": "unit_price",
     "target_paths": ["python/orders.py", "python/pricing.py", "rust/src/lib.rs", "rust/src/pricing.rs"]},
    {"id": "typescript-price", "question": "Where does the TypeScript unitPrice spelling appear?", "query": "unitPrice",
     "target_paths": ["web/orders.ts", "web/pricing.ts"]},
    {"id": "format-units", "question": "Where is formatUnits declared?", "query": "formatUnits",
     "target_paths": ["web/format.js"]},
    {"id": "rust-qualified-call", "question": "Where is the qualified Rust price call?", "query": "pricing::unit_price",
     "target_paths": ["rust/src/lib.rs"]},
    {"id": "prose-literal", "question": "Where is the no-model-key limitation stated?", "query": "No model key",
     "target_paths": ["README.md"], "direct_search_case": True},
    {"id": "prose-preservation", "question": "Where are preserved source bytes described?", "query": "The source snapshot preserves the bytes",
     "target_paths": ["README.md"], "direct_search_case": True},
    {"id": "absent-symbol", "question": "Is a nonexistent billing scheduler present?", "query": "nonexistent_billing_scheduler",
     "target_paths": [], "expected_no_matches": True},
]


def digest(value: bytes) -> str:
    return hashlib.sha256(value).hexdigest()


def stats(values: list[float]) -> dict:
    return {"runs": len(values), "min": min(values), "median": statistics.median(values), "max": max(values)}


def observe(command: list[str], cwd: Path, env: dict, output: Path, stem: str,
            acceptable: tuple[int, ...] = (0,)) -> tuple[dict, bytes]:
    started = time.perf_counter_ns()
    result = subprocess.run(command, cwd=cwd, env=env, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                            timeout=120, check=False)
    elapsed = (time.perf_counter_ns() - started) / 1_000_000
    stdout_name = f"raw/{stem}.stdout"
    stderr_name = f"raw/{stem}.stderr"
    (output / stdout_name).write_bytes(result.stdout)
    (output / stderr_name).write_bytes(result.stderr)
    return {
        "wall_ms": round(elapsed, 6), "returncode": result.returncode,
        "process_ok": result.returncode in acceptable,
        "stdout_utf8_bytes": len(result.stdout), "stderr_utf8_bytes": len(result.stderr),
        "stdout_sha256": digest(result.stdout), "stderr_sha256": digest(result.stderr),
        "stdout_file": stdout_name, "stderr_file": stderr_name,
    }, result.stdout


def rg_paths(raw: bytes) -> tuple[set[str], int]:
    paths: set[str] = set()
    count = 0
    for line in raw.splitlines():
        row = json.loads(line)
        if row.get("type") == "match":
            paths.add(row["data"]["path"]["text"].removeprefix("./"))
            count += 1
    return paths, count


def knowledge_paths(raw: bytes) -> tuple[set[str], int]:
    result = json.loads(raw)
    rows = result["results"]
    for row in rows:
        # Presence of actual evidence is part of this method's acceptance. A
        # metadata-only search response is not silently counted as equivalent.
        evidence = row["evidence"]
        for required in ("snapshot_id", "path", "source_sha256", "start_line", "end_line", "fragment"):
            if required not in evidence:
                raise ValueError(f"missing evidence field {required}")
        if not isinstance(evidence["fragment"], str):
            raise ValueError("source fragment must be text")
    return {row["object"]["path"] for row in rows}, len(rows)


def evaluate(record: dict, raw: bytes, backend: str, case: dict) -> dict:
    try:
        paths, count = knowledge_paths(raw) if backend == "knowledge" else rg_paths(raw)
        targets = set(case["target_paths"])
        missing = targets - paths
        unexpected = paths - targets
        no_match_ok = not case.get("expected_no_matches") or count == 0
        return {**record, "target_paths_found": sorted(paths & targets), "missing_target_paths": sorted(missing),
                "additional_paths": sorted(unexpected), "result_paths": sorted(paths), "result_count": count,
                "target_path_coverage": None if not targets else len(paths & targets) / len(targets),
                "expected_no_matches_passed": no_match_ok if case.get("expected_no_matches") else None,
                "accepted": bool(record["process_ok"] and not missing and no_match_ok)}
    except (ValueError, KeyError, TypeError) as error:
        return {**record, "accepted": False, "parse_failure": str(error)}


def markdown(report: dict) -> str:
    lines = ["# Knowledge retrieval benchmark", "", "Local implementation measurement on the original `examples/knowledge` fixture.", "",
             "This is a retrieval-only comparison. It measures commands, returned bytes and predefined target-path coverage. It does not measure LLM answer correctness, model token use, production workloads, or paid execution.", "",
             f"- Captured: {report['captured_at']}",
             f"- Compiler executable SHA-256: `{report['binary']['sha256']}`",
             f"- Corpus SHA-256: `{report['corpus']['sha256']}` ({report['corpus']['files']} files, {report['corpus']['utf8_bytes']} UTF-8 bytes)",
             f"- Knowledge snapshot: `{report['snapshot_id']}`",
             f"- Repetitions: {report['configuration']['query_runs']} per query/backend; {report['configuration']['compile_runs']} per compile mode.", "",
             "## Compile", "", "| Mode | Median ms | Min–max ms | Reused source files per run |", "|---|---:|---:|---|"]
    for mode, result in report["compile"].items():
        timing = result["wall_ms"]
        reuse = ", ".join(str(run.get("reused_files", "?")) for run in result["observations"])
        lines.append(f"| {mode} | {timing['median']:.2f} | {timing['min']:.2f}–{timing['max']:.2f} | {reuse} |")
    lines += ["", "Cold means a new application store; filesystem/OS caches were not flushed. Warm uses the existing parsed-file cache. Full uses `--full` on the existing store. All modes include CLI process startup and source capture.", "",
              "## Retrieval", "", "| Identical query | Knowledge median ms | rg median ms | Knowledge UTF-8 bytes | rg UTF-8 bytes | Target paths: knowledge / rg |", "|---|---:|---:|---:|---:|---|"]
    for case in report["queries"]:
        k, r = case["backends"]["knowledge"], case["backends"]["rg"]
        target = len(case["target_paths"])
        kfound = len(k["observations"][0].get("target_paths_found", []))
        rfound = len(r["observations"][0].get("target_paths_found", []))
        coverage = f"{kfound}/{target} / {rfound}/{target}" if target else "No-match case"
        lines.append(f"| `{case['query']}` | {k['wall_ms']['median']:.2f} | {r['wall_ms']['median']:.2f} | {k['stdout_utf8_bytes']['median']:.0f} | {r['stdout_utf8_bytes']['median']:.0f} | {coverage} |")
    lines += ["", "The knowledge command returns structured source objects and stored fragments with immutable snapshot IDs, source hashes and line ranges. `rg --json --fixed-strings` returns matching lines and source paths directly. These output contracts have different provenance and context; byte counts are not interchangeable measures of answer quality.", "",
              "Both backends receive exactly the query string shown. The knowledge compiler uses tokenized full-text search; rg uses literal substring matching. Questions in the raw report explain the intended target paths, and are not sent to an LLM. Additional matching paths and every command failure are included in the JSON report.", "",
              "## Direct search wins", ""]
    wins = report["direct_search_wins"]
    for item in wins:
        lines.append(f"- `{item['query']}`: rg returned fewer bytes ({item['rg_bytes']:.0f} vs {item['knowledge_bytes']:.0f}) and completed faster ({item['rg_ms']:.2f} vs {item['knowledge_ms']:.2f} ms median), with required target-path coverage preserved.")
    if not wins:
        lines.append("No case met both the lower-byte and lower-latency direct-search definition on this run. Per-case measurements remain above.")
    lines += ["", "## Acceptance and limits", "",
              f"- Accepted retrieval runs: {report['acceptance']['accepted_runs']}/{report['acceptance']['total_runs']}.",
              f"- Recorded failures: {len(report['acceptance']['failures'])}.",
              f"- Cold/warm/full snapshot equivalence: {report['acceptance']['snapshot_equivalence']}.",
              "- Query order alternates by repetition; neither OS caches nor background system load are controlled.",
              "- This small, intentionally multilingual fixture is not representative of a large production repository. No extrapolation is claimed.",
              "- Timing includes process startup. No tokenizer, inference model, network compilation, or payment was used.",
              "- UTF-8 bytes are the complete stdout of each actual command, including each tool's JSON formatting and rg summary events. Raw outputs are in `raw/`.",
              "", "## Reproduce", "", "```sh", report["reproduce"], "```", "",
              "Choose a new output directory for each run. The harness refuses to overwrite a nonempty directory.", ""]
    return "\n".join(lines)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--source", type=Path, default=Path("examples/knowledge"))
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--runs", type=int, default=5)
    parser.add_argument("--compile-runs", type=int, default=3)
    args = parser.parse_args()
    if not 1 <= args.runs <= 30 or not 1 <= args.compile_runs <= 10:
        parser.error("--runs must be 1..30 and --compile-runs 1..10")
    binary = args.binary.resolve(strict=True)
    source = args.source.resolve(strict=True)
    rg = shutil.which("rg")
    if not rg:
        parser.error("rg is required")
    if args.out.exists() and any(args.out.iterdir()):
        parser.error("output directory is not empty; choose a new directory")
    output = args.out.resolve()
    (output / "raw").mkdir(parents=True, exist_ok=True)
    environment = {k: v for k, v in os.environ.items() if not k.startswith(("COVENANT_", "ANTHROPIC_", "OPENAI_"))}
    environment["LC_ALL"] = "C"
    manifest = []
    for file in sorted(source.rglob("*")):
        if file.is_symlink():
            parser.error("benchmark corpus must not contain symlinks")
        if file.is_file():
            raw = file.read_bytes()
            raw.decode("utf-8")
            manifest.append({"path": file.relative_to(source).as_posix(), "utf8_bytes": len(raw), "sha256": digest(raw)})
    corpus_digest = digest(json.dumps(manifest, sort_keys=True, separators=(",", ":")).encode())
    failures = []
    compile_results = {}
    snapshot_ids = []
    with tempfile.TemporaryDirectory(prefix="covenant-knowledge-benchmark-", dir="/tmp") as temporary:
        work = Path(temporary)
        fixed_binary = work / "covenant-compute"
        shutil.copy2(binary, fixed_binary)
        corpus = work / "source"
        shutil.copytree(source, corpus)
        warm_store = work / "store-0"
        project = None
        for mode in ("cold", "warm", "full"):
            observations = []
            for run in range(args.compile_runs):
                store = work / f"store-{run}" if mode == "cold" else warm_store
                command = [str(fixed_binary), "knowledge", "compile", str(corpus), "--store", str(store), "--json"]
                if mode == "full":
                    command.append("--full")
                record, raw = observe(command, corpus, environment, output, f"compile-{mode}-{run}")
                if not record["process_ok"]:
                    raise RuntimeError(f"compile failed; see {record['stderr_file']}")
                value = json.loads(raw)
                record.update({"snapshot_id": value["snapshot_id"], "coverage": value["coverage"], "reused_files": value["reused_files"]})
                observations.append(record)
                snapshot_ids.append(value["snapshot_id"])
                project = value["project"]["id"]
            compile_results[mode] = {"wall_ms": stats([r["wall_ms"] for r in observations]), "observations": observations}
        queries = []
        for case in CASES:
            by_backend = {"knowledge": [], "rg": []}
            for run in range(args.runs):
                order = ["knowledge", "rg"] if run % 2 == 0 else ["rg", "knowledge"]
                for backend in order:
                    if backend == "knowledge":
                        command = [str(fixed_binary), "knowledge", "query", case["query"], "--limit", "50", "--store", str(warm_store), "--project", project, "--json"]
                    else:
                        command = [rg, "--json", "--fixed-strings", "--", case["query"], "."]
                    record, raw = observe(command, corpus, environment, output, f"{case['id']}-{backend}-{run}", (0,) if backend == "knowledge" else (0, 1))
                    evaluated = evaluate(record, raw, backend, case)
                    by_backend[backend].append(evaluated)
                    if not evaluated["accepted"]:
                        failures.append({"case": case["id"], "backend": backend, "run": run, **evaluated})
            queries.append({**case, "backends": {backend: {
                "wall_ms": stats([r["wall_ms"] for r in rows]),
                "stdout_utf8_bytes": stats([r["stdout_utf8_bytes"] for r in rows]),
                "observations": rows,
            } for backend, rows in by_backend.items()}})
        direct_wins = []
        for case in queries:
            k, r = case["backends"]["knowledge"], case["backends"]["rg"]
            if all(row["accepted"] for row in r["observations"]) and r["wall_ms"]["median"] < k["wall_ms"]["median"] and r["stdout_utf8_bytes"]["median"] < k["stdout_utf8_bytes"]["median"]:
                direct_wins.append({"case": case["id"], "query": case["query"], "rg_bytes": r["stdout_utf8_bytes"]["median"], "knowledge_bytes": k["stdout_utf8_bytes"]["median"], "rg_ms": r["wall_ms"]["median"], "knowledge_ms": k["wall_ms"]["median"]})
        report = {
            "schema_version": 1, "captured_at": datetime.now(timezone.utc).isoformat(),
            "measurement": "retrieval-only; no LLM correctness or token claim",
            "binary": {"sha256": digest(fixed_binary.read_bytes()), "version": subprocess.check_output([str(fixed_binary), "--version"], env=environment, text=True).strip()},
            "rg_version": subprocess.check_output([rg, "--version"], text=True).splitlines()[0],
            "corpus": {"source": args.source.as_posix(), "sha256": corpus_digest, "files": len(manifest), "utf8_bytes": sum(f["utf8_bytes"] for f in manifest), "manifest": manifest},
            "configuration": {"query_runs": args.runs, "compile_runs": args.compile_runs, "query_limit": 50, "os_cache_flushed": False, "includes_process_startup": True, "backend_order": "alternating each repetition", "source_copied_outside_git": True},
            "commands": {"knowledge": "covenant-compute knowledge query QUERY --limit 50 --store STORE --project PROJECT --json", "rg": "rg --json --fixed-strings -- QUERY ."},
            "snapshot_id": snapshot_ids[0], "compile": compile_results, "queries": queries, "direct_search_wins": direct_wins,
            "acceptance": {"total_runs": len(CASES) * 2 * args.runs, "accepted_runs": len(CASES) * 2 * args.runs - len(failures), "failures": failures, "snapshot_equivalence": len(set(snapshot_ids)) == 1},
            "reproduce": f"python3 scripts/benchmark-knowledge.py --binary {args.binary.as_posix()} --source {args.source.as_posix()} --out docs/evidence/knowledge/benchmark-new-run --runs {args.runs} --compile-runs {args.compile_runs}",
        }
        (output / "benchmark.json").write_text(json.dumps(report, indent=2) + "\n")
        (output / "report.md").write_text(markdown(report))
        print(json.dumps({"report": str(output / "report.md"), "acceptance": report["acceptance"], "direct_search_wins": len(direct_wins)}, indent=2))
        return 0 if not failures and report["acceptance"]["snapshot_equivalence"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
