# Batch inference release qualification

Status on 2026-10-21: **source candidate; production disabled**. This record does
not establish a paid batch, public SDK release or deployed image.

The candidate is based on deployment commit
`5146e46e6bae31bf6813ca85f099674e878a1ae9` and implements the
[bounded batch contract](compute-inference-batches.md). Fresh batches contain
one or two independent text requests. The generic 32-item proof/recovery format
does not activate larger fresh purchases.

## Local qualification

| Check | Result |
| --- | --- |
| Gateway | 58 unit tests and 1 binary startup/recovery test passed |
| Protocol | 308 tests passed, including ordered proof vectors |
| Rust quality | Gateway/protocol all-target Clippy with warnings denied; formatting passed |
| JavaScript SDK | 91 tests passed with the real official SVM verifier integration enabled |
| Python SDK | 148 tests passed, including Node/Python shared-journal recovery |
| Python packaging | Wheel and source distribution built; 53 batch tests passed from the extracted source distribution |
| JavaScript packaging | Dry run includes batch module and excludes local dependencies |
| Payment verifier | 17 tests passed: exact child-price sum, route/identity binding, default-off and sealed-payment recovery |
| Runtime and smoke harness | 24 tests passed, using isolated state and no paid dispatch |
| Repository guards | Passed; regenerated metrics and corrected the base branch's existing Knowledge-image knob inventory omission |

The wallet and operator keys used in these tests are ephemeral and unfunded.
Coordinator results and chain finality are fixture data. Actual SDK signing and
official x402 verification are exercised; live provider execution and blockchain
settlement are not established by these tests. No production identities, latches,
reserves, provider budgets or financial journals were changed.

## Remaining release gates

1. Exact-commit CI and immutable Linux image build, anonymous image retrieval,
   and persistent-volume restart smoke with paid admission disabled.
2. Fresh production journal/budget/identity review, including pending original
   payments and the v4 migration/rollback boundary.
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
