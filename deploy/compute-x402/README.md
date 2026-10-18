# Public inference deployment

This is the release container for the inbound x402 V2 Solana USDC seller. It
contains the Rust gateway, the official x402 verifier, a normal Covenant Compute
operator and its private inference provider adapter. Only the gateway binds the
public port. It uses the existing coordinator and staking/settlement rules.

Build `deploy/Dockerfile.compute-x402` from the repository root. The dedicated
image workflow publishes an immutable `x402-<revision>` tag inside the existing
public Compute node package. Deploy its resolved digest only after an anonymous
registry manifest/blob read succeeds. The runtime is Node24 plus the pinned Rust
build; no Docker socket or GPU access is required for this provider-backed node.

## State and secrets

Run one instance with a persistent volume at `/data`. The supervisor creates
separate gateway, operator, buyer-budget and provider directories, with private
permissions. Identity seeds are restored only onto an empty volume; conflicting
buyer/fee-payer seeds refuse startup. A conflicting optional operator seed locks
new admissions while payment recovery stays available. Fee-payer keys and all existing monetary journals must
survive redeployment. Do not scale this container to multiple independent disks.

Copy the names in `environment.example` to the deployment's secret store. The
example deliberately disables operator registration and public admissions.
Configure the real merchant, fee payer and payout identities. Create the merchant
USDC associated token account and fund the dedicated fee payer with a small,
explicit SOL allowance. Keep merchant, client and funding-wallet private keys
outside the public service. Only its dedicated fee payer, native buyer and node
signing seeds belong in this container.

The operator needs the deployment's existing CVNT stake before it can be matched.
Stake approval is independent of the USDC test/reserve cap. Do not disable the
coordinator's stake requirement or reuse an active operator identity for a second
service.

## Initial acceptance allowance

The approved aggregate allowance is **10 USDC** for live tests and initial gateway
reserve. The proposed initial allocation is deliberately smaller:

| Purpose | Maximum counted against approval |
| --- | ---: |
| Native gateway prefunding and lifetime compute ceiling | 3 USDC |
| Local provider qualification | 1 USDC |
| Deployed provider lifetime ceiling | 1 USDC |
| AgentKit test wallet/customer payments | 0.5 USDC |
| Transaction fees and account creation | At most 1 USDC equivalent |

Local and deployed provider journals use separate allocations; their combined
ceiling remains 2 USDC. Keep both as evidence. A response-usage upper bound is not
an observed provider debit. Existing unrelated provider-account activity is not
part of this release allowance. Reconcile unknown outcomes before any retry or
budget increase. Unspent gateway balance remains owned by Covenant; the gateway
does not automatically recycle customer receipts into a fresh spending ceiling.

## Promotion

1. Verify the image revision, dependency locks, private bridge tests, installed
   AgentKit package and Rust payment/recovery tests.
2. Provision the service with a persistent disk and admissions disabled. Confirm
   secrets survive a restart without changing identities or resetting journals.
3. Confirm the provider can serve the exact configured model within price and
   latency limits. Enable the staked operator and confirm live inference capacity.
4. Fund the gateway's dedicated buyer and read back its credited balance. Verify
   the merchant/asset/network/fee payer against the public 402 quote.
5. Under the remaining test cap, run the installed AgentKit demo. Retain signed
   request, native job/receipt, exact output, transaction readback and budget
   records. Repeat the original authorization and verify no second purchase.
6. Restart and inspect the same journals, balances and identities. Publish the
   page/video only with this evidence and the deployed revision.

For rollback, set `COVENANT_GATEWAY_ADMISSIONS_ENABLED=false`. Leave the gateway,
verifier and persistent volume running so pending settlement can recover. Do not
delete a journal, reset a volume, replace a payment authorization or reverse
already-earned operator pay. Process `/health` alone does not prove capacity or
successful paid inference.

## Recovery when an optional worker stops

The public gateway starts before the optional provider/operator in recovery-only
mode. `/data/gateway/admissions.starting` blocks quotes and new payment admissions
until the provider reports ready and the exact local operator has a fresh,
matchable registration. `/health` returns `admissions_enabled` separately from
process health; a recovery-only service still returns HTTP 200.

A failed provider or operator writes `/data/gateway/admissions.disabled` and
syncs it to the persistent disk before optional workers stop. Payment verification,
reconciliation and already-admitted result delivery keep running. The failure
latch survives redeployment and is never automatically removed. While it exists,
optional workers are not restarted and no new paid qualification probe runs.
Hung optional workers are terminated after a bounded shutdown window without
stopping payment recovery. After reviewing
and fixing the failure, explicitly remove that one latch file and restart the
service; readiness must pass again. Do not remove any payment/provider journal,
identity file or budget record. Disabling the local operator leaves admissions
closed while recovery remains available.

The private bridge authentication/seal secret is pinned in
`/data/gateway/bridge-secret` with mode 0600. Keep its configured value identical
across revisions: existing pending payments carry seals made with that key.
Changing it refuses startup, just like changing the buyer or fee-payer key.
