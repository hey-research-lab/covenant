# Live x402 discovery release evidence

Package 9 (Prism discovery PR192) is live on 2026-10-19. The public
[manifest](https://compute-inference.opencovenant.org/.well-known/x402.json) and
[model catalog](https://compute-inference.opencovenant.org/v1/models) passed
production discovery and unpaid quote acceptance. This establishes current offer
availability and quote consistency; the earlier paid inference evidence remains
in the [model-pricing release record](compute-model-pricing-release-evidence.md).

## Deployed artifact

- Source: `21925eb644458f59bcc3c8272efd7ca370da8031`, [PR #364](https://github.com/open-covenant/covenant/pull/364).
- Image: `ghcr.io/open-covenant/covenant/compute-node-public@sha256:10bb42ef5d19d84a1435999ac652ba13938bdf18422d1ca2b603a2d4633541eb`.
- Render deployment: `dep-dbb769m7bikc73er302g`; one deployment POST.
- Both source CI runs and the full image workflow passed. The image smoke used
  512 MiB and verified closed discovery reads and retained state across restart
  with zero paid dispatches. Anonymous OCI manifest/config verification matched
  the source and immutable digest.
- All 51 environment values, one-instance topology and persistent volume were
  preserved. No coordinator restart, tariff rotation or budget increase occurred.

## Acceptance and accounting

The read-only checker passed three public GETs. The explicit unpaid check then
passed six GETs and four HTTP 402 quote requests, with matching payment headers,
bodies, request hashes, quote IDs and authenticated retained journal entries:

| Pricing | Output cap | Retail micro-USDC |
|---|---:|---:|
| Legacy | 1 | 50,000 |
| Model tariff | 1 | 12,009 |
| Model tariff | 256 | 14,304 |
| Model tariff | 2,048 | 30,432 |

The checker sent no payment authorization and loaded no wallet. Quote preparation
can create signed native envelopes; none was dispatched for acceptance.
Private provider and gateway-buyer native evidence stayed unchanged during the
checks. All five unpaid quotes across both attempts expired normally, leaving
exactly the seven historical gateway entries (six complete, one failed), with
unchanged entry fingerprints and a 3,000,000-micro-USDC gateway reserve.
The gateway-buyer native journal retained seven jobs, with no active job,
completed-unpaid job or open transfer attempt.

The deployment's normal startup made one completed provider call, using 13 input
and five output tokens. Actual additional provider-accounted cost was **58
micro-USDC (0.000058 USDC)** under the newly approved 750,000-micro-USDC allowance.
The 315,360-micro-USDC reservation was a per-probe maximum, not the actual charge.
Provider lifetime accounting moved from 335,634 to 335,692 micro-USDC; all 14
historical rows and the 315,360-micro-USDC historical abandoned loss were retained.
No provider outcome remained unresolved. The existing daily and lifetime caps
remain 1,000,000 micro-USDC. The rollout allowance is closed after this accepted
run; unused allowance is not permission for a subsequent paid operation.

## Acceptance checker correction

The first unpaid quote check stopped on its expiry upper bound. A contemporaneous
public timestamp showed the server 54 ms ahead of the client's response-end time.
The gateway correctly clamps quote expiry to native issue time plus 75 seconds
(120-second native freshness less the 45-second delivery margin). The checker
omitted the five-second clock tolerance already used for manifest timestamps.

The first unpaid quote was inspected and allowed to expire before a deliberate
new check with fresh IDs. The checker now uses that same bounded clock tolerance,
while preserving expired-quote rejection and the absolute tariff delivery margin.
All four accepted quotes were independently verified to expire exactly 75 seconds
after their native issue timestamp. A real HTTP regression with the server five
seconds ahead and boundary rejection tests passed: 24 acceptance/smoke tests.
This correction changes only local acceptance tooling and its tests; the live
runtime source and image above remain unchanged, with no second deployment.

The [sanitized evidence record](compute-evidence/discovery/2026-10-19/release.json)
contains exact quote observations, final GET acceptance and hashes of the private
financial continuity evidence. Credentials, key material, customer outputs and
private environment snapshots are not published. No new paid inference,
settlement acceptance or discovery-index adoption is claimed by this release.
