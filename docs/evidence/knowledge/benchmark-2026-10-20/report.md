# Knowledge retrieval benchmark

Local implementation measurement on the original `examples/knowledge` fixture.

This is a retrieval-only comparison. It measures commands, returned bytes and predefined target-path coverage. It does not measure LLM answer correctness, model token use, production workloads, or paid execution.

- Captured: 2026-10-20T07:27:41.635518+00:00
- Compiler executable SHA-256: `9f3d6f9d98ae3d3c2584e6fba276cad03e5f16444e4974a99112d6e46f184365`
- Corpus SHA-256: `0526ea3105269cedeba997331c2e2c826bdfcb3f53b59e4ffc052a6d3eeab540` (9 files, 1813 UTF-8 bytes)
- Knowledge snapshot: `be3484a00bb2f03b9a30c1b3233f6788b8db16cd89deeb8389a82ae98526cc90`
- Repetitions: 5 per query/backend; 3 per compile mode.

## Compile

| Mode | Median ms | Min–max ms | Reused source files per run |
|---|---:|---:|---|
| cold | 32.58 | 25.71–507.36 | 0, 0, 0 |
| warm | 27.00 | 25.84–43.69 | 9, 9, 9 |
| full | 36.35 | 28.59–48.68 | 0, 0, 0 |

Cold means a new application store; filesystem/OS caches were not flushed. Warm uses the existing parsed-file cache. Full uses `--full` on the existing store. All modes include CLI process startup and source capture.

## Retrieval

| Identical query | Knowledge median ms | rg median ms | Knowledge UTF-8 bytes | rg UTF-8 bytes | Target paths: knowledge / rg |
|---|---:|---:|---:|---:|---|
| `calculate_total` | 16.82 | 8.81 | 21569 | 3524 | 4/4 / 4/4 |
| `unit_price` | 11.56 | 7.60 | 12704 | 2636 | 4/4 / 4/4 |
| `unitPrice` | 13.47 | 8.39 | 6408 | 1551 | 2/2 / 2/2 |
| `formatUnits` | 7.81 | 6.72 | 2633 | 798 | 1/1 / 1/1 |
| `pricing::unit_price` | 17.71 | 7.88 | 10279 | 817 | 1/1 / 1/1 |
| `No model key` | 16.43 | 9.63 | 5724 | 834 | 1/1 / 1/1 |
| `The source snapshot preserves the bytes` | 14.13 | 9.88 | 5901 | 852 | 1/1 / 1/1 |
| `nonexistent_billing_scheduler` | 16.62 | 9.08 | 134 | 263 | No-match case |

The knowledge command returns structured source objects and stored fragments with immutable snapshot IDs, source hashes and line ranges. `rg --json --fixed-strings` returns matching lines and source paths directly. These output contracts have different provenance and context; byte counts are not interchangeable measures of answer quality.

Both backends receive exactly the query string shown. The knowledge compiler uses tokenized full-text search; rg uses literal substring matching. Questions in the raw report explain the intended target paths, and are not sent to an LLM. Additional matching paths and every command failure are included in the JSON report.

## Direct search wins

- `calculate_total`: rg returned fewer bytes (3524 vs 21569) and completed faster (8.81 vs 16.82 ms median), with required target-path coverage preserved.
- `unit_price`: rg returned fewer bytes (2636 vs 12704) and completed faster (7.60 vs 11.56 ms median), with required target-path coverage preserved.
- `unitPrice`: rg returned fewer bytes (1551 vs 6408) and completed faster (8.39 vs 13.47 ms median), with required target-path coverage preserved.
- `formatUnits`: rg returned fewer bytes (798 vs 2633) and completed faster (6.72 vs 7.81 ms median), with required target-path coverage preserved.
- `pricing::unit_price`: rg returned fewer bytes (817 vs 10279) and completed faster (7.88 vs 17.71 ms median), with required target-path coverage preserved.
- `No model key`: rg returned fewer bytes (834 vs 5724) and completed faster (9.63 vs 16.43 ms median), with required target-path coverage preserved.
- `The source snapshot preserves the bytes`: rg returned fewer bytes (852 vs 5901) and completed faster (9.88 vs 14.13 ms median), with required target-path coverage preserved.

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
python3 scripts/benchmark-knowledge.py --binary agent-os/target/debug/covenant-compute --source examples/knowledge --out docs/evidence/knowledge/benchmark-new-run --runs 5 --compile-runs 3
```

Choose a new output directory for each run. The harness refuses to overwrite a nonempty directory.
