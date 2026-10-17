# Compute: Prism adoption ledger

Release branch: `codex/compute-price-history-release`, based on Covenant
`deploy/compute-coordinator` at `5b023331575da656da77edc8554213f58e99ad90`.
Comparison: Prism `main` at `4e32eb5eb1398fd9d11dcd29aa5a96e3f2f646ec`.
This release contains package 1 only. Packages 2–20 remain separate on
`codex/compute-prism-parity`; their presence there is not production evidence.

Solana USDC settlement, CVNT staking, admission and existing fees are unchanged.
Provider market sampling is read-only and cannot create rentals or move funds.

| # | Earliest upstream change | Local implementation | Local validation | Production evidence |
|---|---|---|---|---|
| 1 | Aug 12 [GPU prices](https://github.com/prismnetwork-tech/prism/pull/142) | Durable hourly provider samples, provisioning quotes, bound settled rates, public page/API | 1,395 affected-crate tests + 280 protocol tests; local provider/restart rehearsal; renderer and docs checks | Promotion pending; see release evidence below |
| 2 | Aug 13 [observer](https://github.com/prismnetwork-tech/prism/pull/160), [status](https://github.com/prismnetwork-tech/prism/pull/164) | Not included in this release | Separate parity branch | Not activated here |
| 3 | Aug 14 [workspaces](https://github.com/prismnetwork-tech/prism/pull/179) | Not included in this release | Separate parity branch | Not activated here |
| 4 | Aug 14 [canary](https://github.com/prismnetwork-tech/prism/pull/180) | Not included in this release | Separate parity branch | Not activated here |
| 5 | Aug 14–15 [SDKs](https://github.com/prismnetwork-tech/prism/pull/182) | Not included in this release | Separate parity branch | Not activated here |
| 6 | Aug 15 [x402 seller](https://github.com/prismnetwork-tech/prism/pull/187) | Not included in this release | Separate parity branch | Not activated here |
| 7 | Aug 15 [bounty](https://github.com/prismnetwork-tech/prism/pull/186) | Not included in this release | Separate parity branch | Not activated here |
| 8 | Aug 16 [tariffs](https://github.com/prismnetwork-tech/prism/pull/191) | Not included in this release | Separate parity branch | Not activated here |
| 9 | Aug 16 [discovery](https://github.com/prismnetwork-tech/prism/pull/192) | Not included in this release | Separate parity branch | Not activated here |
| 10 | Aug 16 [dashboard](https://github.com/prismnetwork-tech/prism/pull/193) | Not included in this release | Separate parity branch | Not activated here |
| 11 | Aug 17–24 [hardware trust](https://github.com/prismnetwork-tech/prism/commit/a066e1de717bd299a8ce942704f344bda8c31909) | Not included in this release | Separate parity branch | Not activated here |
| 12 | Aug 24 [batches](https://github.com/prismnetwork-tech/prism/commit/c5efc2823916a09b78008ec286b76029934136d0) | Not included in this release | Separate parity branch | Not activated here |
| 13 | Aug 24 [warm pool](https://github.com/prismnetwork-tech/prism/commit/4e52ebb42fd31f7f2f7b207ff7050b1a22d82ca6) | Not included in this release | Separate parity branch | Not activated here |
| 14 | Aug 25 [confidential inference](https://github.com/prismnetwork-tech/prism/commit/e7307a8e493eca00375f04c7d08ad565dc3e0008) and follow-up fixes | Not included in this release | Separate parity branch | Not activated here |
| 15 | Aug 29–30 [reproduction](https://github.com/prismnetwork-tech/prism/commit/740364facff6d07d1c9579881e39cbc014fcecc4) | Not included in this release | Separate parity branch | Not activated here |
| 16 | Aug 30 [host identity](https://github.com/prismnetwork-tech/prism/commit/20c2f58c96ab8b63946c8509b8f1e88cdcd290a4) | Not included in this release | Separate parity branch | Not activated here |
| 17 | Aug 30 [durable budgets](https://github.com/prismnetwork-tech/prism/commit/20c2f58c96ab8b63946c8509b8f1e88cdcd290a4) | Not included in this release | Separate parity branch | Not activated here |
| 18 | Sep 4 [billing](https://github.com/prismnetwork-tech/prism/pull/195), [readiness](https://github.com/prismnetwork-tech/prism/pull/196) | Not included in this release | Separate parity branch | Not activated here |
| 19 | Sep 16–26 [rails](https://github.com/prismnetwork-tech/prism/pull/199), [bridge](https://github.com/prismnetwork-tech/prism/pull/201) | Not included in this release | Separate parity branch | Not activated here |
| 20 | Sep 24–26 [decisions](https://github.com/prismnetwork-tech/prism/pull/200), [policy](https://github.com/prismnetwork-tech/prism/pull/201) | Not included in this release | Separate parity branch | Not activated here |

## 1. Historical GPU prices

`COVENANT_COMPUTE_PUBLIC_PRICE_HISTORY=1` enables the standalone `/prices`
page, `/prices/history` JSON API and an hourly read-only Vast market sampler.
The source is up to eight lowest whole-instance asks for each of eight named GPU
models, with 16 GB storage fixed in the query. Bandwidth and additional usage
charges are excluded. It is a limited market sample, not all provider supply or
proof that a listed machine meets Covenant admission policy.

Provider observations live in `provider-price-history.json` beside the existing
journal. Each snapshot is locked, bounded, fsynced and atomically replaced before
publication. Restart preserves observations and the hourly sampling boundary.
Failures retain prior data and report unavailable/stale rather than zero price;
an empty successful search is distinct. Retention is 366 days. Quotes that do not
change are sampled too, avoiding a mean biased toward frequently changing offers.

`LeaseAccess.source_price` is optional version-1 metadata. The Vast broker records
its selected offer's declared GPU model/count and whole-instance quote. The
coordinator stamps observation time and persists it in the existing journal.
Legacy messages and signing bytes remain valid. No old prices are backfilled.

`GET /prices/history?since_ms=…&until_ms=…` returns daily series, source/currency,
sample counts, observation bounds and explicit millionths per GPU-hour. Default
range is 30 days; maximum is 366 days. USD provider asks and gross USDC rental
charges stay separate. Multi-GPU rates use GPU-hours; settled rates are weighted
by metered duration. GPU identity is provider/operator-reported, not attested.

Settled series require a verified mainnet-USDC deployment pin in `price-history-identity.json`, and the same
pin durably recorded when access and payout are observed. Missing identity,
wrong chain/mint, old metadata, unknown-configuration periods, unpaid/refunded
jobs, probes and malformed receipts are excluded. Payout timestamps use the
observed payout time, not job completion or claimed block time. Signatures and
arithmetic are checked offline; transaction citations remain independently
checkable on chain and do not themselves prove finality.

All price surfaces are opt-in independently of the existing proof feed.
`/prices/proofs/:job_id` publishes only the exact qualified new lease records
represented in price history. The older full receipt feed stays private unless
a deployment separately enables it. The page
uses Covenant's published brand guide, separates sources/assets, shows gaps and
outages, and inserts provider labels as text. It exposes no buyer, prompt, SSH
endpoint or provider credentials. Range filtering precedes signature work;
blocking workers and a two-request concurrency limit keep public history off the
financial async path. Provider aggregation streams into daily counters instead
of duplicating a full year of offers.

## Release evidence and promotion

Local validation on 2026-10-17:

- 1,395 tests passed across coordinator, node, buyer and control (41 existing
  opt-in/ignored targets); 280 protocol tests passed separately.
- Eight page renderer tests passed, including hostile labels/links, separate
  sources/assets, missing days, exact small prices and outage handling.
- Both Compute documentation validators passed; affected coordinator/node Clippy
  (all targets, warnings denied) and formatting passed. Scoped price-proof and
  hostile-wire HTTP tests passed after the final privacy refinement.
- The repository-wide script gate has an inherited failure: 18 stale chain-CLI
  documentation line references. The script, cited CLI source and cited document
  are byte-identical to deployed `5b02333`; running the guard on that baseline
  produces the same errors. This is not a passing whole-repository CI claim.
- A real local coordinator queried the public provider with no credentials or
  paid operations: 53 offers across eight GPU models. Restart preserved the
  coordinator identity and exact persisted history without duplicate sampling.
- Identity, delayed payout, journal replay, unknown-network periods, source store
  corruption, interrupted writes and HTTP opt-in/privacy/range checks pass.

Production promotion is pending. The current public coordinator is
`https://compute-api.opencovenant.org`. Preserve the coordinator and broker
persistent `/data` disks and identities. Before promotion, zero jobs were offered/accepted,
six operators were live and journal health was good.

Publish immutable coordinator/node images from the reviewed deployment commit.
The existing private image names remain available to Render. Identical new public
mirror packages `covenant/compute-coordinator-public` and
`covenant/compute-node-public` allow anonymous pulls without changing visibility
of historical private packages. Then
verify registry pulls, deploy the accepting coordinator before the metadata-emitting
broker, and enable price history. Scoped price proofs require the verified mainnet/USDC binding;
keep the broader public-proof-feed setting unchanged. Verify `/health` names the exact build revision, provider
sampling becomes available, existing books/identity/capacity recover, and run:

```sh
node agent-os/scripts/accept-price-history-release.mjs \
  https://compute-api.opencovenant.org <40-character-image-revision> \
  3Z27s9cCPNWYg7y86PhGwHpEMV4crrY5vAwTwzQNbjjh
```

Disable `COVENANT_COMPUTE_PUBLIC_PRICE_HISTORY` to withdraw the public feature
while existing settlement and cleanup workers continue. Preserve the history
files and additive journal metadata. Do not replace the financial journal with
an older snapshot during rollback. No bounty, paid canary or later adoption
package is activated by this release.
