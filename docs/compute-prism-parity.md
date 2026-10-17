# Compute: Prism adoption ledger

Implementation branch: `codex/compute-prism-parity`, based on Covenant
`deploy/compute-coordinator` at `5b023331575da656da77edc8554213f58e99ad90`.
Comparison: Prism `main` at `4e32eb5eb1398fd9d11dcd29aa5a96e3f2f646ec`.
These are comparison pins, not deployment attestations. Implementations are
integrated in the chronological order below. Parallel preparation does not
constitute release acceptance.

Solana USDC settlement, CVNT staking and existing fees remain the economic
baseline. External rails are gateway revenue, with no implicit conversion.
The optional bounty must remain unfunded. No paid production tests or deployment
have been performed by this implementation session.

| # | Earliest upstream change | Local implementation | Local validation | Production evidence |
|---|---|---|---|---|
| 1 | Aug 12 [GPU prices](https://github.com/prismnetwork-tech/prism/pull/142) | Implemented: durable provisioning quotes and sourced/settled daily series | 3 focused tests passed (units, series boundaries, replay/idempotency) | None |
| 2 | Aug 13 [observer](https://github.com/prismnetwork-tech/prism/pull/160), [status](https://github.com/prismnetwork-tech/prism/pull/164) | In progress | Pending | None |
| 3 | Aug 14 [workspaces](https://github.com/prismnetwork-tech/prism/pull/179) | In progress | Pending | None |
| 4 | Aug 14 [canary](https://github.com/prismnetwork-tech/prism/pull/180) | In progress | Pending | None |
| 5 | Aug 14–15 [SDKs](https://github.com/prismnetwork-tech/prism/pull/182) | Pending | Pending | None |
| 6 | Aug 15 [x402 seller](https://github.com/prismnetwork-tech/prism/pull/187) | Pending | Pending | None |
| 7 | Aug 15 [bounty](https://github.com/prismnetwork-tech/prism/pull/186) | Pending | Pending | Unfunded; inactive |
| 8 | Aug 16 [tariffs](https://github.com/prismnetwork-tech/prism/pull/191) | Pending | Pending | None |
| 9 | Aug 16 [discovery](https://github.com/prismnetwork-tech/prism/pull/192) | Pending | Pending | None |
| 10 | Aug 16 [dashboard](https://github.com/prismnetwork-tech/prism/pull/193) | Pending | Pending | None |
| 11 | Aug 17–24 [hardware trust](https://github.com/prismnetwork-tech/prism/commit/a066e1de717bd299a8ce942704f344bda8c31909) | Pending | Pending | None |
| 12 | Aug 24 [batches](https://github.com/prismnetwork-tech/prism/commit/c5efc2823916a09b78008ec286b76029934136d0) | Pending | Pending | None |
| 13 | Aug 24 [warm pool](https://github.com/prismnetwork-tech/prism/commit/4e52ebb42fd31f7f2f7b207ff7050b1a22d82ca6) | Pending | Pending | None |
| 14 | Aug 25 [confidential inference](https://github.com/prismnetwork-tech/prism/commit/e7307a8e493eca00375f04c7d08ad565dc3e0008) and follow-up fixes | Pending | Pending | None |
| 15 | Aug 29–30 [reproduction](https://github.com/prismnetwork-tech/prism/commit/740364facff6d07d1c9579881e39cbc014fcecc4) | Pending | Pending | None |
| 16 | Aug 30 [host identity](https://github.com/prismnetwork-tech/prism/commit/20c2f58c96ab8b63946c8509b8f1e88cdcd290a4) | Pending | Pending | None |
| 17 | Aug 30 [durable budgets](https://github.com/prismnetwork-tech/prism/commit/20c2f58c96ab8b63946c8509b8f1e88cdcd290a4) | Pending | Pending | None |
| 18 | Sep 4 [billing](https://github.com/prismnetwork-tech/prism/pull/195), [readiness](https://github.com/prismnetwork-tech/prism/pull/196) | Pending | Pending | None |
| 19 | Sep 16–26 [rails](https://github.com/prismnetwork-tech/prism/pull/199), [bridge](https://github.com/prismnetwork-tech/prism/pull/201) | Pending | Pending | None |
| 20 | Sep 24–26 [decisions](https://github.com/prismnetwork-tech/prism/pull/200), [policy](https://github.com/prismnetwork-tech/prism/pull/201) | Pending | Pending | None |

## 1. Historical GPU prices

`LeaseAccess.source_price` is an optional version-1 metadata object; old wire
messages remain valid. The Vast broker records its selected offer's GPU model,
GPU count and whole-instance quote. The coordinator's existing durable job
journal retains observations across restarts and compaction. These are quotes
sampled at successful provisioning, not a complete provider market feed or
provider invoices. No historical samples are fabricated for old jobs.

`GET /prices/history?since_ms=…&until_ms=…` is enabled by the existing
`COVENANT_COMPUTE_PUBLIC_PROOF_FEED` opt-in. It returns daily per-GPU series,
source/currency, sample count, observation bounds and explicit millionths per
GPU-hour units. The default range is 30 days, maximum 366 days. Settled samples
require a verified signed settlement proof, actual payout transaction reference,
known GPU count and pinned nonzero metered duration. Quote USD and settled USDC
remain separate. Rates are duration-weighted, including multi-GPU time. Unpaid,
mock, probe and metadata-less historical jobs cannot inflate settled history.
Proof links expose no input, buyer identity or access endpoint.

Files: `agent-os/crates/covenant-compute-protocol/src/price_history.rs`,
`covenant-compute-node/src/broker.rs`,
`covenant-compute-coordinator/src/prices.rs` (crate paths under `agent-os/crates`).

## Acceptance and rollback

1. Run focused tests and the existing Compute suites and both documentation validators.
2. Rehearse additive journal migrations/recovery against copied state; preserve old signing bytes.
3. Publish immutable artifacts, test clean npm/wheel installs and anonymous image pulls.
4. Run capped funded staging transactions, real GPU access, cleanup and vendor evidence checks.
5. Verify the deployed revision and public canary before enabling dependent services.

Disable new admissions on rollback while settlement and cleanup continue. Storage,
provider credentials, gateway reserve, alert destination and real hardware evidence
are separate release gates; code availability alone does not satisfy them.
