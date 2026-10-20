# Knowledge paid acceptance: proposed budget

**Historical proposal, 20 October 2026.** The user subsequently approved this
bounded run. [Completed acceptance evidence](evidence/knowledge/paid-pilot-2026-10-20/report.md)
records two jobs totaling **0.10 USDC** and **0.000015 SOL** in actual network
fees, with no duplicate executions or charges. The existing hold was preserved
and the original operator service restored. The paid client was the explicitly
hashed clean native 0.5.0 candidate; signed public archive installation is
verified separately. All observations and proposed gates below describe the
earlier research state, not current public remote availability.

Request approval for **at most 0.10 USDC in new gross job charges, plus a separate 0.0001 SOL transaction-fee ceiling**. This covers **two original Knowledge compilation jobs at no more than 0.05 USDC each**. Replaying either original job after a client restart authorizes **zero additional jobs**. Unused allowance is not a cost or permission for another test.

The 0.05 USDC pilot ask is a proposed operator configuration and hard acceptance ceiling, **not an observed live quote**. There is currently no advertised Knowledge operator. If the eventual exact compiler offer exceeds this ceiling, stop and request a revised budget before signing.

## Live observations

Read-only checks were made at **2026-10-20 15:51:29–15:54:25 UTC**. The complete sanitized observations, response hashes and arithmetic are in [the budget evidence JSON](evidence/knowledge/paid-acceptance-budget-2026-10-20.json).

| Observation | Result |
|---|---|
| Applicable coordinator | `https://compute-api.opencovenant.org`, identified by the deployment guides and checked against the live public health endpoint |
| [Current health](https://compute-api.opencovenant.org/health) | HTTP 200; `ok`; revision `b3e4c3de460703410675e30e429cf52e3ae779d3` |
| [Current capacity](https://compute-api.opencovenant.org/federation/capacity) | Seven matchable operators; **zero** `batch_job` rows whose model begins `knowledge-compile-v1:` |
| Exact Knowledge ask | **Unavailable** until an operator advertises the accepted immutable compiler digest |
| [Deposit instructions](https://compute-api.opencovenant.org/federation/deposit-info) | Prefunding enforced; Solana RPC rail; USDC mint `EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v` |
| Deposit owner | `EQpsU4i6g9dFru72bcLcg8fjYWsWLGhMPQFor2nAruPv`; required memo `compute-buyer:<buyer_pubkey_b58>` |
| Deposit token account | `5xwduCSqtPDDXhoUzdZVVRRncYYYQbz5sM6vp14wgZGd` exists and is initialized for the advertised owner and USDC mint |
| Mainnet identity | Public RPC returned genesis `5eykt4UsFv8P8NJdTREpY1vzqKqZKvdpKuc147dw2N9d` |
| Marketplace fee rate | `/federation/fees` returned HTTP 403 to an unsigned GET; current basis points are **not established** |
| Buyer available credit | **Not freshly read.** The balance endpoint requires a signed request, which was outside this read-only, no-signature research |

The advertised `imd-swarm` batch price and other inference/check prices are not Knowledge quotes. Health and public capacity identify the current service, but do not establish that its code or an operator is ready for the new capability. Require the updated coordinator and exact compiler image to pass the deployment guide before funding a test.

No secret key was loaded or printed. No buyer identity, budget or purchase journal was created. No request was signed. The private balance endpoint was not called, and the refused fee read was not retried with credentials.

## What the approval amount includes

| Item | Proposed maximum | Treatment |
|---|---:|---|
| Job A: installed CLI happy path | 50,000 micro-USDC = **0.05 USDC** | One original pinned-source job |
| Job B: client restart/recovery | 50,000 micro-USDC = **0.05 USDC** | A second original job; recover that same job after restart |
| Replays, receipt reads, local import/query | **0 new job charges** | Existing request identity only; any attempted replacement aborts acceptance |
| Total new gross jobs | **100,000 micro-USDC = 0.10 USDC** | Hard allowance for this acceptance only |
| New USDC top-up | **At most 0.10 USDC**, reduced by freshly verified available credit | Prepaid principal for the two jobs, **not an additional 0.10 USDC cost** |
| Solana transaction fees | **100,000 lamports = 0.0001 SOL total** | Separate ceiling for incremental funding/payout/unused-credit withdrawal fees attributable to this acceptance; exact payer and fee estimates must be recorded |
| New token-account rent | **Not authorized** | Existing deposit account needs no creation. A new account requires separate review |
| New hosting, operator rental or deployment spend | **Not authorized by this budget** | Use an already approved operator host or quote these costs separately |

If the available credit is `B` and the two approved exact offers total `P`, the required top-up is `max(0, P − B)`, with `P ≤ 100,000` micro-USDC. Establish available credit through a separately authorized signed balance read before calculating funding. Fund and confirm credit **before the first signed job**, rather than creating an unfunded envelope and trying to fund it later.

Any deposited amount that remains available after settlement is unused buyer credit. It can be withdrawn through the existing signed withdrawal flow once outstanding holds and transfer states permit; it is not automatically returned, guaranteed liquid during an unresolved outcome, or an additional fee. Retain an uncertain withdrawal's original identity and reconcile it before any retry.

### Marketplace rake is inside the gross offer

The [fee implementation](../agent-os/crates/covenant-compute-protocol/src/fees.rs) computes `floor(gross × fee_bps / 10,000)`. The coordinator [withholds it from the operator payout](../agent-os/crates/covenant-compute-coordinator/src/main.rs); it is not added on top of the buyer's offered amount. Therefore the requested 0.10 USDC gross allowance already covers the marketplace take.

The current fee rate was not observable through the public endpoint. Record it during authorized deployment verification before running the pilot. For illustration only, a 1,000-bps rate would split a 50,000-micro-USDC gross job into 45,000 for the operator and 5,000 for the marketplace; **that is not a claim about today's configured rate**.

### Solana fees and rent

Solana's [official fee reference](https://solana.com/docs/core/fees) currently states a 5,000-lamport base fee per signature plus optional prioritization fees. The 0.0001 SOL figure is a conservative approval ceiling, not a transaction quote. The funding wallet and coordinator payout wallet may pay different parts of it. Record all relevant fee payers and obtain fresh fee estimates before sending; do not raise priority fees or retry transfers beyond the total ceiling.

The existing deposit associated-token account was verified through a finalized `getAccountInfo` read, so new recipient-account rent is not expected for the top-up. A separate finalized `getMinimumBalanceForRentExemption(165)` read returned **1,488,440 lamports (0.00148844 SOL)** at this observation. That is a current example rent reserve for a 165-byte account, not a fee or an authorization to create one. A new sender/recipient account, different account size, unsupported wallet or changed rail is an abort-and-review condition. Recoverable account rent is also distinct from coordinator USDC credit.

## Preserve the selected buyer and holds

Before approving a paid run, privately inspect the selected buyer's original identity, canonical coordinator endpoint, persisted budget namespace, pending reservations, rolling settled charges and purchase journal. Keep buyer-specific financial state, state hashes and namespace associations out of public repository evidence. This public proposal deliberately contains no buyer identity or actual balance/reservation details.

A pending reservation does not expire just because it is old. Preserve every existing hold until its original outcome is explicitly reconciled. The new acceptance allowance is **100,000 micro-USDC**, separate from earlier commitments. If pending commitments are `P` and settled charges within the current rolling window are `S`, a ceiling admitting this full allowance must be at least `P + S + 100000`. Verify the existing ceiling and any clock-watermark change before proposing a guarded update.

Only an explicitly approved `budget set-limit NEW --expect-limit OLD` may change the persisted ceiling. This research has not run that command. Use the same original buyer and budget homes and canonical endpoint; do not edit budget JSON, delete locks, switch identities/homes, or set an environment-only limit to bypass persisted state. The per-call cap for this proposal is `COVENANT_COMPUTE_MAX_PRICE_MICRO_USDC=50000`; the daily environment limit must match the separately approved persisted ceiling.

A higher rolling ceiling that preserves previous commitments does not expand this pilot's two-job allowance or authorize additional funding. After acceptance, inspect current commitments before any separately approved limit reduction. Review the concrete buyer-specific arithmetic privately with the user immediately before approval.

## Two original jobs, one durable identity per job

Prerequisites for both jobs: published installed CLI provenance; real accepted OCI compiler digest; updated exact-match coordinator/operator; committed public fixture whose selected source bytes pass anonymous GitHub comparison; reviewed source limits; and approved funding/budget state.

1. **Job A — installed CLI happy path.** Save the installed binary hash/version, exact repository commit, compiler digest, source digest, offer, deadline and one new idempotency key. Perform a dry run against the exact live model and require a per-job ask no greater than 50,000 micro-USDC. Confirm credited funds, then submit the intended original request once. Independently verify the signed receipt/output, import validation, source citation and actual settled gross charge. Repeat the identical command/key only to prove recovery of the same job and receipt, with no second execution or debit.
2. **Job B — client process restart.** Choose a second original key within the remaining allowance. Preserve the same buyer, endpoint, source, digest, price and deadline. Once its durable request identity is recorded and submission has been attempted, stop only the controlled buyer process, then restart with the **identical** command/key and the original journal. Verify one job ID, one compiler execution, one settlement and a valid local import. A client restart is not permission to restart unrelated production services or purchase another job.

The [remote purchase implementation](../agent-os/crates/covenant-compute-buyer/src/knowledge_remote.rs) binds an idempotency key to the original envelope and arguments. If a response is lost, inspect/reconcile that original job. Do not create a third key, re-sign a fresh replacement, change the price or deadline, or repeat an uncertain funding transfer. A stale or rejected envelope remains evidence; an authorized new job would require a new budget decision.

The prior buyer guide documents a 120-second operator admission freshness window. Reconfirm the actual operator policy before the run and recover promptly; a longer execution deadline does not extend admission freshness. A completed replay should be an authenticated recovery of the original result. A failure of the replay/restart acceptance assertion stops the pilot rather than triggering replacement spending.

## Abort gates

Stop before signing if the exact Knowledge capability is absent or stale; the ask/unit exceeds the approved per-job terms; the compiler digest, source commit or anonymous public-source match changes; deployment verification is incomplete; the coordinator/rail/mint/owner differs from the reviewed identity; the balance or persisted budget cannot be established; or an outstanding commitment cannot be accounted for.

Stop before any transfer if the wallet fee estimate would exceed the total SOL ceiling, a new account/rent deposit is required, or a previous transfer has an unknown outcome. Hosting/provider charges are outside this proposal.

Stop after a purchase if receipt/source/compiler/snapshot verification fails, settlement is ambiguous, a replay produces another job/debit, or the remaining allowance cannot cover the second original job. Preserve the original purchase and pending reservation. A failed paid computation may still consume its approved job amount; failure is not a refund guarantee.

Before any spending, report the final exact operator ask, gross two-job ceiling, existing available credit, required top-up, current persisted commitments, fee estimates and selected identities to the user. Proceed only after the user's budget approval. This read-only research performed **zero signatures, transfers, purchases, budget changes or deployments**.
