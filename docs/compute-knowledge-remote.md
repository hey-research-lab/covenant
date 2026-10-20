# Remote Knowledge compilation

**Limited public-source service; dedicated paid acceptance verified.** One
separately authorized job on the dedicated operator passed signed-receipt,
artifact, local-import and finalized-settlement checks. An identical-key settled
replay recovered the same job, receipt and snapshot without another job or
charge. See the [dedicated acceptance record](evidence/knowledge/dedicated-acceptance-2026-10-21/report.md).
The job cost **50,000 micro-USDC / 0.05 USDC**; this is a recorded charge, not a
standing quote. The service admits one job in flight. Check current capacity,
price and admission eligibility before buying; no uptime guarantee is implied.

The [signed Compute 0.5.1 release](https://github.com/open-covenant/covenant/releases/tag/compute-v0.5.1),
[two earlier paid pilot jobs](evidence/knowledge/paid-pilot-2026-10-20/report.md),
[dedicated activation](evidence/knowledge/remote-activation-2026-10-20/report.md)
and [finalized stake extension](evidence/knowledge/stake-extension-2026-10-21/report.md)
have separate evidence. The dedicated paid job tested normal completion and
settled replay; interrupted-client recovery remains evidence from the earlier pilot.

## What the command does

The buyer captures a public GitHub repository at a pinned commit, anonymously
compares every selected file with that public revision, and requests the exact
compiler through a Compute batch job. The operator runs the compiler in a bounded
container. The buyer verifies the signed receipt and artifact, recompiles it
locally, then imports the verified snapshot. Search, MCP and the viewer remain
local.

The accepted compiler model is:

```text
knowledge-compile-v1:d011ce03baa42e825e345bd028c4b9af82e210535958c4ce5b49eb8a0503745c
```

Requests accept **1–256 selected files and at most 512 KiB of serialized source
JSON**, including filenames, text and metadata. This is not an archive-size
allowance. The typed result is capped at 3 MiB. Private repositories, a shared
team index and confidential execution are outside this service. Full local
recompilation is part of verification; no speedup or token-savings claim follows.

## Preview a public example without buying

Install Compute 0.5.1 using the [website installer](https://opencovenant.org/install/compute.sh)
or its [signed release archives](https://github.com/open-covenant/covenant/releases/tag/compute-v0.5.1).
The same remote command is also supported by 0.5.0. Use a new empty example directory. These commands
fetch the pinned public commit without checking out or running repository code:

```sh
git init knowledge-public-example
git -C knowledge-public-example fetch --depth=1 \
  https://github.com/pallets/itsdangerous.git \
  672971d66a2ef9f85151e53283113f33d642dabd

COVENANT_COMPUTE_COORDINATOR_URL=https://compute-api.opencovenant.org \
COVENANT_COMPUTE_MAX_PRICE_MICRO_USDC=50000 \
  covenant-compute knowledge compile ./knowledge-public-example \
  --remote --dry-run \
  --repository https://github.com/pallets/itsdangerous \
  --commit 672971d66a2ef9f85151e53283113f33d642dabd \
  --compiler-image-sha256 d011ce03baa42e825e345bd028c4b9af82e210535958c4ce5b49eb8a0503745c \
  --deadline-ms 300000 \
  --json
```

Use the repository root with `--commit`. The repository URL omits `.git`; the
compiler flag accepts the digest alone, without `sha256:` or an image name.
The example's recorded source is 50 files, 294,697 normalized JSON bytes and
SHA-256 `9ce84a3e81b11f03398a63a9e0e2ade69618354c10e6fe25946db6f45baeba42`.
Those are fixture-specific values, not a whole-repository coverage guarantee.

The preview reports `dry_run: true`, model, commit, source digest/bytes, current
offered price, deadline and `submitted: false`. It does not sign or submit a
purchase, reserve budget or create a purchase entry. It loads or creates the
normal local buyer identity, so retain an existing `COVENANT_COMPUTE_MCP_HOME`.
Public GitHub or exact compiler capacity being unavailable stops the preview
before submission. A GitHub quota refusal does not establish an operator outage.

A preview does not reserve capacity or a price. Inspect the current
[capacity directory](https://compute-api.opencovenant.org/federation/capacity)
for an exact `batch_job` model row with `operators > 0` and `per_job` pricing.
The directory contains aggregate counts and asks, not operator identities.
The activation evidence separately records the deployed identity. Compute 0.5.x
selects the exact compiler model and has no buyer flag for choosing an operator
identity.

## Buy within an authorized budget

Keep the original buyer and budget homes. Confirm the current quote, available
credit and persisted allowance before purchasing. The following is a purchase
template; documentation and an available quote do not authorize spending:

```sh
: "${COVENANT_COMPUTE_DAILY_BUDGET_MICRO_USDC:?Set the authorized persisted daily allowance}"
: "${COVENANT_COMPUTE_MAX_PRICE_MICRO_USDC:?Set the authorized per-call ceiling}"
: "${KNOWLEDGE_PRICE_MICRO_USDC:?Set the approved current quote in micro-USDC}"
: "${KNOWLEDGE_IDEMPOTENCY_KEY:?Set and retain one durable unique request key}"
export COVENANT_COMPUTE_DAILY_BUDGET_MICRO_USDC COVENANT_COMPUTE_MAX_PRICE_MICRO_USDC

COVENANT_COMPUTE_COORDINATOR_URL=https://compute-api.opencovenant.org \
  covenant-compute knowledge compile ./knowledge-public-example \
  --remote \
  --repository https://github.com/pallets/itsdangerous \
  --commit 672971d66a2ef9f85151e53283113f33d642dabd \
  --compiler-image-sha256 d011ce03baa42e825e345bd028c4b9af82e210535958c4ce5b49eb8a0503745c \
  --price "$KNOWLEDGE_PRICE_MICRO_USDC" \
  --deadline-ms 300000 \
  --idempotency-key "$KNOWLEDGE_IDEMPOTENCY_KEY" \
  --json
```

The budget variables must be exported and agree with persisted state. A per-call
ceiling does not create a daily allowance. Use the guarded budget command for
any separately approved limit change; preserve existing reservations.

A successful result contains `import.project.id`, `import.snapshot_id` and the
verified `receipt`. Use the returned IDs:

```sh
covenant-compute knowledge verify SNAPSHOT_ID --project PROJECT_ID
covenant-compute knowledge query "Signer" --project PROJECT_ID
covenant-compute knowledge serve --project PROJECT_ID
```

For interruption or settled recovery, repeat the same paid command, key,
arguments and buyer identity. Settled recovery fetches the original receipt and
output without a new job, and does not require current compiler capacity.
Never replace a key or delete accounting to make an uncertain request proceed.

## Current stake window and operating limits

The dedicated operator identity is
`CfaV6nB59aDnTJccD5WXUiQejg9prMvtQrkFqQZRhWmf`.
The [finalized extension](evidence/knowledge/stake-extension-2026-10-21/report.md)
retains its **1,000,000 CVNT stake with zero added tokens**. Fixed lock expiry is
**19 November 2026 at 22:10:13 UTC**. The coordinator requires at least 48 hours
remaining, so current admission eligibility ends **17 November 2026 at
22:10:13 UTC**. This replaces the initial October 25 admission cutoff; the
original activation record remains a dated account of the initial stake.

Current capacity and operator health remain required. Any further extension
requires separate owner authorization; no automatic renewal or continuous
availability is promised. Admission eligibility ends before token unlock.

The live service admits one compilation at a time under a separate operator
identity and resource group, preserving Kani. The accepted cohosting test used
a small real Kani proof alongside a real near-limit Knowledge fixture. It does
not establish sustained peak-load behavior. The
[activation report](evidence/knowledge/remote-activation-2026-10-20/report.md)
records the tested scope, current service status and observed quote separately.

See the [operator deployment and recovery guide](compute-knowledge-remote-acceptance.md)
for container, source-verification, budget and reconciliation requirements.
