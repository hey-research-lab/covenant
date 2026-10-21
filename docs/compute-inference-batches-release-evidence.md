# Batch inference release qualification

Status on 2026-10-21: **source candidate; production disabled**. This record does
not establish a paid batch, public SDK release or deployed image.

The candidate is based on deployment commit
`5146e46e6bae31bf6813ca85f099674e878a1ae9`, includes the standard-wallet fix
`5fefb03d8eed2c1b48acfb8042cf8fd0a101cb55`, and implements the
[bounded batch contract](compute-inference-batches.md). Fresh batches contain
one or two independent text requests. The generic 32-item proof/recovery format
does not activate larger fresh purchases.

## Local qualification

The combined source passed the checks below after integrating the
standard-wallet update; its immutable image still requires qualification.
The earlier batch image must not replace the wallet-compatible production image:
both original branches used journal v4 for different additions. The combined
runtime uses v5 and retains frozen short and long memo bindings, the standard
client settlement wait, and existing financial records.

| Check | Result |
| --- | --- |
| Gateway | 63 unit tests and 1 binary startup/recovery test passed; v4 migration preserves frozen short and long memos |
| Protocol | Unchanged from the original batch candidate: 308 tests passed, including ordered proof vectors |
| Rust quality | Gateway/protocol all-target Clippy with warnings denied; formatting passed |
| JavaScript SDK | 111 tests passed with the real official SVM verifier integration enabled, including short and long batch memos |
| Python SDK | 172 tests passed, including Node/Python shared-journal recovery with both memo forms |
| Client packaging | Original batch archives qualified; new archives must be pinned to the combined source before live acceptance |
| Payment verifier | 22 tests passed: exact child-price sum, short/long memo and route binding, default-off and sealed-payment recovery |
| Discovery harness | 16 tests passed; single-chat discovery remains separate from batch activation |
| Runtime and smoke harness | 24 tests passed, using isolated state and no paid dispatch |
| Repository guards | Passed; regenerated metrics and corrected the base branch's existing Knowledge-image knob inventory omission |

The wallet and operator keys used in these tests are ephemeral and unfunded.
Coordinator results and chain finality are fixture data. Actual SDK signing and
official x402 verification are exercised; live provider execution and blockchain
settlement are not established by these tests. No production identities, latches,
reserves, provider budgets or financial journals were changed.

The inherited CLI packaging check also needed a PR-only snapshot mode: the
deployment branch has a 0.5.0 node alongside 0.4.1 buyer/staking tools. Snapshot
archives validate each extracted binary against its own manifest and include
source and binary provenance. Tag releases still require aligned versions and
release notes; this change does not publish or rename an official release.

## Remaining release gates

1. Exact-commit CI and immutable Linux image build, anonymous image retrieval,
   and persistent-volume restart smoke with paid admission disabled.
2. Fresh production journal/budget/identity review, including pending original
   payments and the v5 migration/rollback boundary.
3. Explicit deployment/test allowance covering provider startup probes, one
   two-child native batch, the single customer USDC transfer and sponsored fees.
4. Installed-client paid acceptance: both child receipts, complete ordered
   proof, original transaction finalized/read back, measured execution latency,
   original-authorization recovery and zero-call completed replay.
5. Before/after accounting and retained journal checks; then separately publish
   immutable SDK versions and decide public batch discovery/indexing.

The deployment workflow now runs both client suites before building the image.
No new purchase, replacement authorization or automatic retry is authorized by
this source qualification record.
