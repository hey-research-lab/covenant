# Model-specific inference pricing: evidence and release gates

Status: release candidate with local validation, researched on 19 October 2026.
Production pricing has not been changed by this work. The final source revision,
deployment image, and paid acceptance must each be recorded separately.

## Decision and customer behavior

Offer an explicit `pricing: "model_tariff_v1"` path for the already qualified
`claude-haiku-4.5` text model. Native settlement is 10,000 micro-USDC plus the
requested output cap at 7,500,000 micro-USDC per million; retail is 12,000 plus
9,000,000 per million. Each variable term rounds upward to a whole microunit.

The retail coefficients are a deliberate 20% markup policy. They are not rates
discovered from an upstream invoice. At the maximum 2,048-token cap, retail is
0.030432 USDC, 39.136% below the existing 0.05 USDC flat offer. The fixed quote
settles in full even when the generated answer is shorter.

Omitting the new pricing field retains the existing 50,000 retail / 30,000 native
micro-USDC flat path. Existing client examples and persisted purchases must remain
valid. No change to the provider model, route, input-byte limit, output limit,
spending ceilings, or full-input reservation policy is included.

The gateway limits a fresh x402 payment quote to at most 75 seconds from its
prepared native envelope's issue time: the 120-second native freshness window
minus a 45-second delivery margin. A nearer tariff expiry can shorten it further.
Admission checks the remaining native deadline and tariff validity before any
new paid execution. Recovery uses the stored signed envelope and transaction.

Public tariff configuration also rejects values above JavaScript's maximum safe
integer (`9007199254740991`), including revision, base/rate, validity timestamps,
and maximum computed price. This prevents a valid Rust `u64` value from being
rounded when the supported clients verify the public JSON quote.

## Published references and provider observations

Read on 19 October 2026:

- [OpenRouter's exact Haiku 4.5 page](https://openrouter.ai/anthropic/claude-haiku-4.5)
  lists `anthropic/claude-haiku-4.5`, a 200,000-token context window, and standard
  input/output prices of 1 / 5 USD per million tokens. This is a published
  upstream reference, not evidence of a particular UsePod account debit.
- [UsePod's proxy reference](https://docs.usepod.ai/api/proxy/) documents the
  token-scoped `GET /v1/models` catalog and input/output price-ceiling headers in
  USDC microunits per million tokens.
- [UsePod's routing reference](https://docs.usepod.ai/marketplace/routing/)
  documents `X-Pod-Providers: openrouter` as a hard provider constraint and
  `centralized-only` routing. The catalog GET returned the exact namespaced Haiku
  model with `owned_by: openrouter`; a listing does not prove inference acceptance.
- [UsePod's spending controls](https://docs.usepod.ai/using/spend-controls/)
  describe refusal when no eligible provider fits both price ceilings. Covenant's
  existing bridge retains ceilings of 1,500,000 input and 7,500,000 output
  micro-USDC per million.

Six successful planner calls from the existing LangGraph and Microsoft Agent
Framework acceptance work reported the following usage and costs in their stored
upstream responses. These are the existing OpenRouter-pinned route's measurements;
they are not new pricing-path acceptance calls or independently audited invoices.

| Input tokens | Output tokens | Provider-reported USDC cost | Conservative accounted micro-USDC |
|---:|---:|---:|---:|
| 899 | 280 | 0.002299 | 3,449 |
| 854 | 231 | 0.002009 | 3,014 |
| 899 | 280 | 0.002299 | 3,449 |
| 875 | 238 | 0.002065 | 3,098 |
| 897 | 315 | 0.002472 | 3,709 |
| 750 | 181 | 0.001655 | 2,483 |

Each reported cost matches 1 micro-USDC per input token plus 5 per output token.
Conservative accounting uses the higher configured ceilings and independently
rounds input and output upward. The underlying local evidence is the existing
private `x402-launch/provider-qualification/calls.sqlite` ledger, with call groups
`langgraph` and `agent-framework`; response text and credentials are not published.
The older single qualification result is excluded from this price comparison
because it predates the current pinned response-identity path.

Four existing completed customer purchases bind `claude-haiku-4.5` to signed
provider metadata reporting `anthropic/claude-haiku-4.5`, `openrouter`, and
`centralized`. Their requests all used a 2,048-token output cap, with message-array
sizes of 574, 925, 1,451, and 1,342 bytes. Those customer responses do not expose
upstream usage. They establish previous flat-path payment and signed identity
behavior, not measured economics for the entire 4,096-byte input allowance.

## Limits of the economic argument

The 10,000-micro native base exceeds the measured input component of these small
samples and allows a commercial contribution toward input and request overhead.
At the configured input ceiling it corresponds to about 6,666 input tokens. A
4,096-byte message limit plus protocol framing appears compatible with that
allowance, but no tokenizer qualification or enforced upstream token-count limit
has established it. The tariff therefore does not guarantee a positive margin
for every accepted provider report.

Keep the 200,000-token provider input reservation. For a requested output cap N,
the reservation remains `300000 + ceil(N × 7500000 / 1000000)` micro-USDC,
or 315,360 at N = 2,048. The reservation protects the spending allocation against
uncertain outcomes; it is not an estimate of normal request cost.

No claim of net profitability is made. Prior customer settlement evidence records
10,001 lamports in Solana fees per transaction; hosting, failures, and other
operational costs have not been allocated to the proposed price. The samples do
not establish rates for other models, longer inputs, tools, images, or streaming.

## Read-only production baseline

At approximately 00:40 UTC on 19 October 2026, authenticated Render GETs showed:

| Service | Live deployment | Immutable image digest | Other state |
|---|---|---|---|
| Inference/x402 `srv-dba12mu1egvs738v5no0` | `dep-dbalu6k9v7es7391j2gg` | `sha256:d95c659e209829640c67ff7930fcfd66b2bd330c5e8c50f2542e6e155114a1ac` | 50 environment keys; watchdog enabled; no pending deploy |
| Coordinator `srv-d9agfv7aqgkc739jpcn0` | `dep-dbam8rs9v7es7392pb0g` | `sha256:046b7ce0bcea4b9814452908201a699b6b0ef0a828eec6a57d06b53a2c9682f8` | 78 environment keys; canary mode `once`; no pending deploy |

The public inference `/health` still reports core revision
`e52e2a790e1dbabf9a7e5018cdca99e82cdcea9e`; the newer immutable image includes the
watchdog overlay. Image identity and the core health revision are different
evidence. Reusing the earlier x402 image would discard the overlay. At this
baseline `/v1/models` returned 404, so the new gateway catalog was not yet live.

Both services use one Frankfurt image-runtime instance and an existing `/data`
disk. A pricing deployment must preserve their current environment, plans, disk,
watchdog, and concurrent rental-canary work. Refresh this baseline immediately
before any deployment because another chat is operating these services.

Existing private rollout helpers use authenticated Render GET snapshots, exact
environment-digest checks, an immutable image digest, and a durable intent before
one deployment POST. Ambiguous POST outcomes are reconciled through GETs, never
blindly submitted again. This research made no Render mutations.

## Historical acceptance headroom

The existing customer journal held six 50,000-micro reservations: four completed
and two uncertain attempts retained. Against its configured 500,000 total ceiling,
that leaves 200,000 micro-USDC of historical policy headroom. The wallet held
300,000 micro-USDC at finalized Solana slot 458637756, read at 00:42:25 UTC.

The separate local planner provider ledger accounted for 619,765 micro-USDC,
including 600,480 in previously abandoned full losses, with no unresolved calls.
Its 1,000,000 configured ceiling leaves 380,235 of local historical headroom.
The production provider journal is different: these local totals must not be used
as its current balance or remaining allowance.

An authenticated, read-only production Render-shell inspection at
00:50:54.581 UTC recorded 10 provider calls and 335,350 micro-USDC of lifetime
accounting. Nine completed calls account for 19,990; one unknown call retains its
entire 315,360 reservation under an existing abandoned-full-loss record. There
are zero unresolved calls: an abandoned loss is retained spending accounting,
not an outstanding dispatch. Rolling-day accounting was 335,292. Both configured
ceilings remained 1,000,000, giving 664,650 lifetime and 664,708 rolling-day
policy headroom at that observation. These are accounting limits, not a provider
account-balance statement or a new spending authorization.

The same read recorded gateway journal version 2, five entries (four complete,
one failed), and the existing 3,000,000-micro reserve. Its journal SHA-256 was
`dcc783aa2dcd0a3985f6ecec1c82fc4babe7c38ed6f55a1297a5f831970c1bed`.
The private observation is retained as
`model-pricing-launch/production-totals-20261019T005054Z.json`. It dispatched no
provider calls and changed no remote state.

Available balance and historical caps do not themselves authorize new acceptance
spending. Confirm the task's funded allowance before signing or dispatching new
paid requests. Read the production provider ledger and reconcile every unresolved
payment/provider outcome first. Never recycle the two uncertain customer IDs,
reset a ledger, or raise a budget as part of tariff activation.

## Local validation and package candidates

The release work validated the working tree locally on 19 October 2026. The
client suites passed with the real local Solana facilitator bridge enabled:

| Check | Result | Scope |
|---|---|---|
| Native protocol, buyer, coordinator, and node library checks | 1,244 passed, 0 failed, 20 ignored | Buyer 250/1 ignored; coordinator 369/0; node 322/19; protocol 303/0 |
| Gateway unit suite | 19 passed, 0 failed, 0 ignored | Local quote, payment-state, gateway behavior, freshness margin, and public safe-integer bounds |
| Actual gateway binary startup/recovery test | 1 passed, 0 failed, 0 ignored | Malformed-tariff recovery-only startup, version-2 journal migration, and settlement using the same transaction through a local mock payment service |
| x402 runtime and smoke helpers | 16 passed | Local runtime configuration and smoke-check behavior |
| JavaScript x402 client | 55 passed, 0 skipped | Local fixtures, signature/quote checks, payment journal and replay |
| Python x402 client | 90 passed | Local fixtures, signature/quote checks, payment journal and replay |

Commands used for these suites (the Rust invocations reuse the existing local
build cache through `CARGO_TARGET_DIR`):

```sh
# From the repository root.
CARGO_INCREMENTAL=0 CARGO_PROFILE_TEST_DEBUG=0 CARGO_PROFILE_DEV_DEBUG=0 \
  cargo test --locked --manifest-path agent-os/Cargo.toml \
  -p covenant-compute-protocol -p covenant-compute-buyer \
  -p covenant-compute-coordinator -p covenant-compute-node --lib --quiet

# From agent-os; CARGO_TARGET_DIR may reuse the existing local build cache.
CARGO_INCREMENTAL=0 CARGO_PROFILE_TEST_DEBUG=0 CARGO_PROFILE_DEV_DEBUG=0 \
  cargo test -p covenant-compute-gateway

# From the repository root.
CARGO_INCREMENTAL=0 CARGO_PROFILE_TEST_DEBUG=0 CARGO_PROFILE_DEV_DEBUG=0 \
  cargo test --locked --manifest-path agent-os/Cargo.toml \
  -p covenant-compute-gateway --test startup_recovery -- --nocapture

# From the repository root, using Node 24.
COVENANT_X402_BRIDGE_PATH="$PWD/services/compute-x402-facilitator/bridge.mjs" \
  node --test sdk/compute-x402-client/test/*.test.mjs

# From sdk/compute-x402-python, using the existing qualified Python environment.
PYTHONPATH="$PWD/src" python -m pytest -q
```

The primary suite results were captured in the task's execution output. Ignored
native tests were not exercised. After moving the unchanged 120-second native
freshness limit into a shared protocol constant, 15 protocol-envelope tests and
one node stale-admission test passed again; these repeat existing tests and are
not added to the 1,244 count. The binary recovery test uses an isolated local
journal and loopback payment fixtures; it is not a production restart or a chain
transaction.

`cargo fmt --all --check`, `git diff --check`, and the complete
`bash agent-os/scripts/validate.sh --scripts` gate passed. The scripts gate ran
223 checks, including the generated README metrics, after updating only its
generated metrics block with `node agent-os/scripts/metrics.mjs --write`.
Its local output is retained in
`/tmp/covenant-model-pricing-validation-20261019/scripts-pricing-ops.log`.

The JavaScript package was built with `npm pack --pack-destination dist --json`;
Python used `python -m build` to produce an isolated wheel and source distribution.
SHA-256 values were independently read back from these local package files:

| Local package candidate | SHA-256 |
|---|---|
| `sdk/compute-x402-client/dist/covenant-compute-x402-client-0.3.0.tgz` | `1ff1a48f7b81032cccfe7c70f658c6a9b6ca41d972f3105735af4df42ca31abb` |
| `sdk/compute-x402-python/dist/covenant_compute_x402-0.2.0-py3-none-any.whl` | `e2d12dade31a531064eac83bb99ad5961b764bbf2bca8e568bba314e3a7ee15d` |
| `sdk/compute-x402-python/dist/covenant_compute_x402-0.2.0.tar.gz` | `ecca00f8a85ab16e58b7e55cbbbaee355a94a4051cd4b6549007826cab091981` |

The extracted JavaScript archive and an installed Python wheel each passed a
local artifact-import smoke check: opt-in variable quote, a pending response,
client/process restart, and exact saved completion replay, with one quote and
one signature. The 1,200-micro fixture amount is synthetic test data, not a live
tariff. Rust-generated signed fixtures exercise both client verifiers.

The native buyer, node, and lease-signer manifests are aligned at `0.4.0`.
The archive README references only `compute-v0.4.0`, and matching notes exist at
`deploy/compute-release/notes/compute-v0.4.0.md`, satisfying the packaging metadata
preflight. This does not establish that all platform archives have been built,
signed, uploaded, or installed. No package was published by these checks.

## Required release evidence

1. Validate the final source: exact rounding; separate retail/native prices;
   explicit opt-in; unchanged flat requests; client quote/receipt verification;
   tariff expiry; idempotent retries; mismatched mode/model/cap rejection; and
   persistent-state compatibility. Record checks and the tested revision.
2. Inspect the final immutable image and its source metadata. Preserve the current
   watchdog overlay and verify the actual provider and gateway configuration;
   do not infer deployment from source presence or an image build.
3. Before deployment, refresh the read-only runtime totals, existing journals,
   pending work, service/image/environment baseline, and the approved spending
   allowance. Record exact tariff IDs, revisions, and validity windows.
4. After deployment, verify unpaid discovery and quote amounts for at least two
   caps, legacy flat quotes, unknown models, malformed/unsupported pricing modes,
   and over-limit caps. An unpaid quote is not provider acceptance.
5. Within the explicitly authorized allowance, complete the minimum paid
   acceptance needed to verify a tariff receipt, exact customer transfer/native
   settlement, and unchanged replay. Record genuine production hashes and chain
   finality. A release announcement must distinguish this from local tests.

No deployment, paid pricing request, budget increase, or publication is recorded
in this evidence draft.
