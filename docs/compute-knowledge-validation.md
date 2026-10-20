# Knowledge candidate validation

Recorded 20 October 2026 on Apple Silicon macOS with Rust 1.98.1. This is an
independent implementation based on the refreshed deployment source
`72f66142f407690024c6a0ca4a3bdaf148ef8766`. It does not replace existing production
acceptance records or authorize spending, deployment, release publication, or posts.

## Local results

The following command completed with **1,236 passed, zero failed, 40 ignored**
across 26 test binaries. Ignored environment-dependent tests were not accepted
as evidence; no real OCI runtime or funded job was exercised.
The [machine-readable result](evidence/knowledge/validation-2026-10-20.json)
records tested source `fa8c47153c859fac092ab7397b32d61937e0efcf`.

```sh
cd agent-os
CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 CARGO_INCREMENTAL=0 \
  cargo test -p covenant-knowledge -p covenant-compute-protocol \
  -p covenant-compute-node -p covenant-compute-buyer --lib --tests --quiet
```

The core suite includes 22 original extraction/lifecycle tests and three concurrency tests for all supported language families,
exact captured source spans, dirty and committed revisions, exclusions,
symlinks and invalid paths, malformed syntax, ambiguous calls, multiple imports,
Markdown sections, incremental/full equivalence, deletion/rename history,
immutable rows, cache corruption, index reconstruction, project isolation,
schema upgrade, and rollback after an injected transaction failure. The rollback
test simulates commit failure; it is not a power-loss test.

The four standalone CLI integration tests cover offline local operation, bounded
MCP frames and project selection, mutation refusal, snapshot selection, and
loopback viewer session, origin, host, proxy-header and body-size restrictions.
They were rerun successfully after the final CLI deadline validation change.

Remote tests cover exact capability matching (including rejection of `any` and
`:latest` aliases), typed contracts, source/result tampering, zero-spend previews,
durable request identity, verified receipt replay, node admission, and resource
arguments and cleanup using a fake container runtime. The HTTP test uses a real
local coordinator and signatures, with mock payment, source verification and
container execution explicitly identified in its exported evidence.

The signed HTTP fixture was rerun successfully after adding an opt-in evidence
export. The export contains no private signing keys. It is **local test evidence**,
not a production payment, a GitHub visibility attestation or a real OCI run.

## Interface and measurement evidence

- [Viewer acceptance](evidence/knowledge/viewer-acceptance.md): actual saved
  snapshot search, evidence, dependency, diff and verify flows at desktop,
  tablet and mobile widths.
- The public console proxy regression checks reject knowledge access before
  reading its shared operator token or making an upstream request. Run
  `node agent-os/covenant-web/scripts/check-knowledge-proxy.mjs`.
- [Reproducible retrieval measurements](evidence/knowledge/benchmark-2026-10-20/report.md)
  retain raw commands, stdout, timings, corpus and executable hashes. All 80
  retrieval runs completed as expected. On this nine-file, 1,813-byte fixture,
  direct `rg` search was faster and smaller in the initial unoptimized run.
  The [optimized preview run](evidence/knowledge/benchmark-release-2026-10-20/report.md)
  had lower Knowledge command latency while direct search still returned fewer
  bytes in every matching case; background test load was not isolated. No speed, token savings or LLM answer-quality claim is supported.
- [Behavioral research](compute-knowledge-research.md) distinguishes observed
  behavior of the pinned reference application from Covenant's original tests.

## Separate release statuses

| Gate | Status |
|---|---|
| Local compiler and interfaces | Implemented; relevant suites and responsive viewer checks passed |
| Local package candidate | Optimized, unsigned Apple Silicon archive built; [19 extracted-package checks passed](evidence/knowledge/package-release-preview-2026-10-20.json), including 18 exact citations and all six MCP reads |
| Signed public macOS/Linux release | Not published; existing release workflow remains the publishing path |
| Actual OCI compiler image | Docker is unavailable in this environment; image execution unverified |
| Public operator and routing rollout | Not deployed |
| Installed client paid acceptance | Not performed; requires an explicit budget and deployed exact capability |
| Real process restart / financial reconciliation | Local durable replay tested; production restart remains unaccepted |
| Shared authenticated daemon access | Not exposed; public proxy refuses knowledge access; project authorization adapter remains a separate gate |
| Private remote repositories | Refused by the public-source pilot; operator-access policy remains undefined |
| Feature page and campaign | Prepared locally; no deployment, publication or scheduling |

Follow the [remote acceptance guide](compute-knowledge-remote-acceptance.md) for
the coordinator → compiler operator → client order and the evidence that must
exist before enabling purchases. Source citations identify captured content.
Signed receipts identify execution and its output. Neither proves correctness
or confidential execution.
