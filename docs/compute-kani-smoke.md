# A fresh-buyer Kani smoke through Covenant Compute

This walkthrough buys one tiny Kani check for **50,000 micro-USDC (0.05 USDC)**,
then independently verifies its saved signed evidence offline. It uses the public
CLI and source, without operator SSH or coordinator configuration changes.
The source is a local smoke fixture; buying its execution does not establish
Covenant authorship of the source or a Percolator proof.

Use Bash or Zsh, Git, Python 3 and Rust 1.98.1. Start in the Covenant checkout
containing this guide. Record its revision; it supplies the documentation adapter
and archived fixture. Build the compatible native buyer from the separate pinned
source below. The old released `compute-v0.2.0` binaries lack native Kani commands;
this is a source-install workflow, not a claim about those downloads.

## Install and isolate the buyer

```sh
set -e
KANI_GUIDE_ROOT="$(git rev-parse --show-toplevel)"
KANI_TEST_ROOT=/absolute/path/to/a/new-kani-test
mkdir -m 700 "$KANI_TEST_ROOT"
cd "$KANI_TEST_ROOT"
git -C "$KANI_GUIDE_ROOT" rev-parse HEAD > guide-source.txt
git clone --filter=blob:none --no-checkout https://github.com/open-covenant/covenant.git pilot
git -C pilot checkout --detach 832210f8239cd5bd52f0451246d80e3355e9e916
cargo +1.98.1 build --locked --manifest-path pilot/agent-os/Cargo.toml \
  --target-dir "$KANI_TEST_ROOT/target" -p covenant-compute-buyer --bin covenant-compute
KANI_CLI="$KANI_TEST_ROOT/target/debug/covenant-compute"
rustc +1.98.1 --version > compiler-version.txt
shasum -a 256 "$KANI_CLI" > cli-sha256.txt

export COVENANT_COMPUTE_COORDINATOR_URL=https://compute-api.opencovenant.org
export COVENANT_COMPUTE_MCP_HOME="$KANI_TEST_ROOT/buyer-home"
export COVENANT_COMPUTE_BUDGET_HOME="$KANI_TEST_ROOT/budget-home"
export COVENANT_COMPUTE_MAX_PRICE_MICRO_USDC=50000
export COVENANT_COMPUTE_DAILY_BUDGET_MICRO_USDC=50000
"$KANI_CLI" whoami --json > buyer.json
"$KANI_CLI" budget status --json > budget-before.json
"$KANI_CLI" balance --json > balance-before.json
"$KANI_CLI" capacity --json > capacity-before.json
```

Keep both homes private and durable, including `agent-purchases/`. Existing Cargo
dependency caches may be reused; record that if describing the test environment.
Buyer admission is a coordinator policy, separate from available capacity. An
access refusal is a blocker to resolve through the service's access process.

## Verify and reconstruct the public fixture

```sh
KANI_ARCHIVE="$KANI_GUIDE_ROOT/docs/compute-evidence/percolator/2026-10-18/517cd13898363b85acf4979a5e6f21519f0fef2327ae09ab070f2c8c14cf1e67"
KANI_ARCHIVED_JOB="$KANI_ARCHIVE/jobs/native-kani-post-stepdown-canary"
KANI_ADAPTER="$KANI_GUIDE_ROOT/docs/compute-kani-smoke-record.py"
python3 "$KANI_ARCHIVE/build-verifier.py" --checkout "$KANI_TEST_ROOT/pilot" \
  --project "$KANI_TEST_ROOT/receipt-verifier"
cp "$KANI_ARCHIVE/verifier.Cargo.lock" receipt-verifier/Cargo.lock
cargo +1.98.1 build --locked --manifest-path receipt-verifier/Cargo.toml \
  --target-dir "$KANI_TEST_ROOT/target"
KANI_VERIFIER="$KANI_TEST_ROOT/target/debug/verify-public-receipt"
python3 "$KANI_ARCHIVE/verify-archive.py" --archive "$KANI_ARCHIVE" \
  --verifier "$KANI_VERIFIER" > historical-verification.json
git init smoke-source
git -C smoke-source fetch --no-tags "$KANI_ARCHIVED_JOB/source.bundle" \
  c441cb62c506ef4e2921ab4944a87149320977f5
git -C smoke-source checkout --detach c441cb62c506ef4e2921ab4944a87149320977f5
git -C smoke-source rev-parse 'HEAD^{tree}' > expected-source-tree.txt
```

Inspect the fixture and plan. `receipt_smoke` selects an arbitrary `u8`, assumes
it is below 10, covers `value == 4` with the label `smoke reachable`, and asserts
`value < 10`. Its tree is `8c2ba88ee1f50b53462940d3f9f5e5457b319c6b`.
There are no dependencies or setup commands. Historical archive verification
checks previous evidence; it does not execute a new solver job.

The exact archived profile is **12,288 MiB memory / 6 CPUs / 4,096 MiB scratch**.
Its immutable image and resource tuple produce capability
`kani-v1:2bf73b40a77f602c334527fa79b6d1bc903c7a92394b4ab02e3889eeed9cda91`.
The capacity API currently exposes that hash without the image/resource
descriptor, so this example obtains the descriptor from the public archive.
Require a matching live row before paying; historical availability is not a
guarantee. Kani's minimum offer is 50,000 micro-USDC even if a row's ask is lower.

## Preview, then fund before signing

```sh
kani_check_args=(
  check --repo "$KANI_TEST_ROOT/smoke-source"
  --commit c441cb62c506ef4e2921ab4944a87149320977f5
  --kani-plan "$KANI_ARCHIVED_JOB/kani/plan.json"
  --check-image ghcr.io/open-covenant/covenant/compute-node-public@sha256:5648c867341b054aebf3570266200cd49766caea7f5119d2735dc437fcfe3f7d
  --memory-mib 12288 --cpus 6 --scratch-mib 4096
  --check-timeout 300 --deadline-ms 600000 --price 50000
)
"$KANI_CLI" "${kani_check_args[@]}" --dry-run --json > preview.json
```

Review the preview's exact image, resources, capability, 50,000 offer, harness,
expected `proven` outcome and required cover. It spends nothing and does not
sign an order. The CLI creates a deterministic history-free source snapshot;
its preview commit can differ from the archived commit while preserving its tree.

**Fund before the first paid command.** A real command journals and signs its
order before submission, including when an unfunded request returns HTTP 402.
The operator's admission freshness window is 120 seconds. Funding afterward
and replaying an old order can therefore produce a terminal rejection; the
ten-minute job deadline does not extend this admission window.

Read the live deposit mint, recipient and `compute-buyer:<buyer_pubkey>` memo
from `balance-before.json`. Transfer the intended USDC funding using your wallet
or the official Solana/SPL-token CLI with that memo, then claim its transaction
signature. The Covenant CLI claims the transfer; it does not make it. Record
the wallet method and transaction. Resolve uncertain transfers before resending.

```sh
"$KANI_CLI" deposit "$KANI_DEPOSIT_SIGNATURE" --json > deposit-claim.json
"$KANI_CLI" balance --json > balance-funded.json
"$KANI_CLI" capacity --json > capacity-funded.json
```

Confirm available funds cover the offer and the exact capability remains live.
Keep machine time synchronized and submit promptly after this check.

## Submit once and retrieve the exact purchase

Choose a new stable key for this intended purchase. Do not change keys to recover
an uncertain order. Save the command's exit status and complete output; failed
acceptance still pays for checker execution.

```sh
KANI_PURCHASE_KEY=fresh-kani-smoke-v1
if "$KANI_CLI" "${kani_check_args[@]}" --idempotency-key "$KANI_PURCHASE_KEY" --json \
  > check-result.json 2> check-stderr.txt; then
  KANI_CHECK_EXIT=0
else
  KANI_CHECK_EXIT=$?
fi
printf '%s\n' "$KANI_CHECK_EXIT" > check-exit.txt
KANI_JOB_ID="$(python3 "$KANI_ADAPTER" purchase \
  --buyer-home "$COVENANT_COMPUTE_MCP_HOME" --buyer buyer.json \
  --coordinator "$COVENANT_COMPUTE_COORDINATOR_URL" --key "$KANI_PURCHASE_KEY" \
  --out purchase.json)"
"$KANI_CLI" output "$KANI_JOB_ID" --json > recovered-output.json
"$KANI_CLI" budget reconcile --json > budget-after.json
"$KANI_CLI" balance --json > balance-after.json
```

The adapter reads the normal journal and selects this buyer, endpoint and key.
It copies the existing envelope; it never signs, submits or edits the journal.
The `output` and budget commands are read-only network recovery. If a job remains
in flight, re-fetch that same ID into a new file. Preserve every original capture.
If no journal exists, no signed purchase is established by these instructions.

`output` includes the receipt and output but omits the buyer envelope. The saved
journal restores that envelope for offline verification. `evidence` is for
authorship chains; `verify` means on-chain payout verification. Neither is the
native offline-verifier command used below.

## Verify the new result offline

```sh
python3 "$KANI_ADAPTER" record --purchase purchase.json --buyer buyer.json \
  --preview preview.json --recovered recovered-output.json --result check-result.json \
  --source-dir smoke-source --expected-tree expected-source-tree.txt \
  --out native-verifier-record.json
"$KANI_VERIFIER" native-verifier-record.json > independent-verification.json
```

The adapter binds the expected job to the journal, the buyer to the saved
identity, and source/plan/resources/price to the pre-purchase preview. It checks
the signed bundle against the independently recorded local tree and compares
the recovered receipt/output with the original capture. If original stdout was
lost or incomplete, omit only `--result`; the journal and recovered signed data
remain required. Existing output files are never overwritten by the adapter.

The unchanged public verifier checks buyer/operator signatures, envelope/output
commitments, the typed Kani report and complete raw evidence, including required
covers. Require exit status zero, `acceptance_passed: true`, `kani.accepted: true`
and `kani.outcome: "proven"`. Verification needs no credentials, coordinator,
RPC or payment. It authenticates an execution report; it does not establish
hardware isolation, complete job inventory or blockchain settlement.

## Preserve failures and budget reservations

An unsigned `operator_rejected` or refund status can show zero coordinator charge
while its offer remains reserved by the conservative budget. Reconciliation
does not release that reservation without the required signed evidence. Preserve
the failed order and key; a new attempt is a separate spending decision.

The pinned buyer source above includes `budget set-limit NEW --expect-limit OLD`.
If an explicitly authorized limit change is needed, use that guarded update, then set
`COVENANT_COMPUTE_DAILY_BUDGET_MICRO_USDC` to the new value in every sharing process.
The setter preserves all pending/settled entries; it neither resets spending nor
releases a hold or authorizes a retry. See the [buyer budget documentation](../agent-os/crates/covenant-compute-buyer/README.md#authorship-and-kani-verification-with-receipts).

Stop after one accepted smoke and independent verification. Keep failures,
funding records, previews, journals and full signed results distinct. Review
private paths and source contents before publishing an evidence archive.
