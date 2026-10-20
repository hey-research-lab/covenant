# Knowledge retrieval benchmark

Optimized Apple Silicon preview build from source `fa8c47153c859fac092ab7397b32d61937e0efcf`, measured on the original `examples/knowledge` fixture. The run overlapped local regression tests, so its latency and ingestion variation must not be presented as an isolated performance trial.

This is a retrieval-only comparison. It measures commands, returned bytes and predefined target-path coverage. It does not measure LLM answer correctness, model token use, production workloads, or paid execution.

- Captured: 2026-10-20T15:16:04.527837+00:00
- Compiler executable SHA-256: `636e02fe8ffca61c2ce417fec91c45dac5c412f7f34abcc5af78e97fa0d50f36`
- Corpus SHA-256: `0526ea3105269cedeba997331c2e2c826bdfcb3f53b59e4ffc052a6d3eeab540` (9 files, 1813 UTF-8 bytes)
- Knowledge snapshot: `be3484a00bb2f03b9a30c1b3233f6788b8db16cd89deeb8389a82ae98526cc90`
- Repetitions: 5 per query/backend; 3 per compile mode.

## Compile

| Mode | Median ms | Min–max ms | Reused source files per run |
|---|---:|---:|---|
| cold | 18.09 | 17.47–590.73 | 0, 0, 0 |
| warm | 16.61 | 14.30–23.49 | 9, 9, 9 |
| full | 60.45 | 14.26–128.24 | 0, 0, 0 |

Cold means a new application store; filesystem/OS caches were not flushed. Warm uses the existing parsed-file cache. Full uses `--full` on the existing store. All modes include CLI process startup and source capture.

## Retrieval

| Identical query | Knowledge median ms | rg median ms | Knowledge UTF-8 bytes | rg UTF-8 bytes | Target paths: knowledge / rg |
|---|---:|---:|---:|---:|---|
| `calculate_total` | 4.44 | 5.42 | 21569 | 3523 | 4/4 / 4/4 |
| `unit_price` | 4.08 | 5.11 | 12704 | 2634 | 4/4 / 4/4 |
| `unitPrice` | 4.04 | 5.21 | 6408 | 1551 | 2/2 / 2/2 |
| `formatUnits` | 4.15 | 5.14 | 2633 | 797 | 1/1 / 1/1 |
| `pricing::unit_price` | 4.28 | 5.30 | 10279 | 816 | 1/1 / 1/1 |
| `No model key` | 4.30 | 5.30 | 5724 | 834 | 1/1 / 1/1 |
| `The source snapshot preserves the bytes` | 4.59 | 5.53 | 5901 | 851 | 1/1 / 1/1 |
| `nonexistent_billing_scheduler` | 4.03 | 5.20 | 134 | 263 | No-match case |

The knowledge command returns structured source objects and stored fragments with immutable snapshot IDs, source hashes and line ranges. `rg --json --fixed-strings` returns matching lines and source paths directly. These output contracts have different provenance and context; byte counts are not interchangeable measures of answer quality.

Both backends receive exactly the query string shown. The knowledge compiler uses tokenized full-text search; rg uses literal substring matching. Questions in the raw report explain the intended target paths, and are not sent to an LLM. Additional matching paths and every command failure are included in the JSON report.

## Direct search tradeoffs

Direct search returned fewer UTF-8 bytes in all seven matching cases with the same required target-path coverage. Knowledge had lower median command latency in this optimized-build run. These dimensions are different: neither establishes answer quality or token/cost savings. The [earlier unoptimized measurement](../benchmark-2026-10-20/report.md) recorded direct search winning both dimensions. Retaining both runs makes build and measurement conditions visible.

## Acceptance and limits

- Accepted retrieval runs: 80/80.
- Recorded failures: 0.
- Cold/warm/full snapshot equivalence: True.
- Query order alternates by repetition; neither OS caches nor background system load are controlled.
- This small, intentionally multilingual fixture is not representative of a large production repository. No extrapolation is claimed.
- Timing includes process startup. No tokenizer, inference model, network compilation, or payment was used.
- UTF-8 bytes are the complete stdout of each actual command, including each tool's JSON formatting and rg summary events. Raw outputs are in `raw/`.

## Reproduce

```sh
python3 scripts/benchmark-knowledge.py --binary agent-os/target/release/covenant-compute --source examples/knowledge --out docs/evidence/knowledge/benchmark-new-run --runs 5 --compile-runs 3
```

Choose a new output directory for each run. The harness refuses to overwrite a nonempty directory.

Raw outputs are distributed as `raw.tar.gz`; extract with `tar -xzf raw.tar.gz` to inspect the recorded `raw/` files.
