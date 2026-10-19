# Public x402 discovery acceptance

`GET /.well-known/x402.json` describes currently observed text inference offers.
`GET /v1/models` exposes the same models and retail schedules. Both are free,
uncached observations. They do not reserve capacity or promise that a later quote
will succeed. A closed admission latch returns an empty catalog; an unavailable
dependency returns an unavailable response rather than a stale payable offer.

The manifest separates the legacy request, which omits `pricing`, from the
`pricing: "model_tariff_v1"` request. The latter fixes its amount from the requested
output cap, including ceiling to a whole micro-USDC unit. It does not bill actual
generated tokens. `price.max_output_tokens` is the currently affordable cap;
the complete, versioned `price.tariff` remains unchanged. Streaming, tools and
vision are not advertised.

## Independent public policy

The standalone acceptance checker uses Node built-ins. It reads only the public
policy file explicitly named on its command line; it loads no wallet, SDK,
environment configuration or credentials. Prepare that file from the approved
deployment policy, independently of the endpoint being tested:

```json
{
  "payTo": "APPROVED_PUBLIC_MERCHANT_ADDRESS",
  "feePayer": "APPROVED_PUBLIC_FEE_PAYER_ADDRESS",
  "model": "claude-haiku-4.5",
  "legacyAmount": 50000,
  "maxInputBytes": 4096,
  "maxOutputTokens": 2048,
  "tariff": {
    "schema_version": 1,
    "id": "covenant-haiku-4.5-retail",
    "revision": 1,
    "model": "claude-haiku-4.5",
    "base_micro_usdc": 12000,
    "output_cap_micro_usdc_per_million": 9000000,
    "max_output_tokens": 2048,
    "valid_from_ms": 0,
    "valid_until_ms": 0
  }
}
```

Replace the address placeholders and both timestamps with the actual approved
values. The example deliberately cannot pass. Preserve the active tariff's ID,
revision and validity window; a discovery release does not renew or rotate it.
Only Solana mainnet, its exact USDC mint, six decimals and x402 version 2 are
accepted. This release check requires the full qualified 2048-token cap even
though the service can advertise a smaller affordable cap at other times.

## Commands and evidence

From the repository root, run local fixtures without external requests:

```sh
node --test deploy/compute-x402/accept-discovery.test.mjs deploy/compute-x402/smoke.test.mjs
```

After the deployment is authorized and the immutable image is live, substitute
its exact 40-character runtime source revision and an independently prepared
public policy file:

```sh
node deploy/compute-x402/accept-discovery.mjs \
  --base-url https://compute-inference.opencovenant.org \
  --revision FULL_RUNTIME_SOURCE_SHA \
  --expected /absolute/path/discovery-public-policy.json
```

The default makes exactly three GETs: health, manifest and models. Redirects are
errors. Requests bypass caches, manifest/catalog responses must prohibit caching,
and all bodies are bounded and parsed with duplicate-key rejection. Exact schemas
exclude unadvertised features and additional private fields. A full source
revision, current tariff window and independently pinned payment addresses are
required. A failed check does not automatically retry.

An explicit `--quotes` adds four unpaid quote requests: legacy and tariff output
caps 1, 256 and 2048. With the example rates, their retail amounts are 50000,
12009, 14304 and 30432 micro-USDC respectively. It then repeats the three GETs:

```sh
node deploy/compute-x402/accept-discovery.mjs \
  --base-url https://compute-inference.opencovenant.org \
  --revision FULL_RUNTIME_SOURCE_SHA \
  --expected /absolute/path/discovery-public-policy.json \
  --quotes
```

Every POST has a fresh `Idempotency-Key` and no payment authorization, wallet
signature, cookie or payment header. The checker requires HTTP 402 and verifies
the header/body agreement, exact request hash, quote ID, memo, current expiry,
payment rail and amount. It never submits a `Payment-Signature`, retries a POST,
or requests paid execution. The gateway creates retained unpaid quote entries
and may prepare signed native jobs; these are expected state changes. The 402
document itself is not a signed native receipt or proof of settlement.

Save the checker's JSON result with deployment evidence. Its pre/post public GETs
do **not** establish that private journals, provider accounting or settlement
state stayed unchanged. Collect those separately through the existing
authenticated, read-only operational reader. Inspect an interrupted run before
starting another; an unknown quote response is not a reason to reuse its ID.

## Deployment gates

1. Qualify an immutable full `compute-x402-image` build at the reviewed source.
   Run payment/readiness, gateway, public acceptance and runtime tests. Set
   `SMOKE_DISCOVERY=true` for image smoke: it checks empty manifest/models before
   and after a persistent-volume restart, with admissions, operator and provider
   disabled. Its private state checker verifies unchanged identities, seal and
   journal entries, an empty buyer budget directory and no provider/operator
   state. The recovery clock may advance. This smoke makes no paid dispatch.
   The optional flag preserves qualification of older observer overlay images;
   those images do not establish discovery support.
2. Refresh the current deployment, topology and environment fingerprint, plus
   authenticated gateway/provider and relevant native payment state. Resolve
   uncertain rollout outcomes before proceeding. Pin the exact live predecessor,
   candidate source/image, public policy and retained financial state in a new
   image-only rollout plan. Preserve the volume, all existing environment values,
   identities, journals, budgets, tariff window, watchdog and rental schedule.
   Do not reset or recycle financial entries or use an environment replacement.
3. A production full-image restart starts the normal provider/operator benchmark.
   It can incur provider cost even though discovery acceptance needs no paid
   customer requests. Account for that probe under the discovery rollout's
   explicit authorization and unchanged provider caps before deploying. The
   presently qualified input/output reservation is at most 315360 micro-USDC
   (0.315360 USDC) for one probe: 200000 input tokens at 1.50 USD per million plus
   2048 output tokens at 7.50 USD per million. This is a conservative reservation,
   not the expected actual charge; refresh these configured limits and available
   headroom before using the bound. The
   completed model-pricing release's previous 0.75-USDC allowance is closed and
   is not authority for this rollout. Do not skip the benchmark or replace a
   process inside the running image to avoid qualification.
4. Record durable deploy intent before one deploy POST. Reconcile an unknown
   response using GETs instead of issuing another deploy. Wait for the exact
   candidate deployment to become live, then check public health/revision and
   authenticated startup classification. Account for the new probe and verify
   retained loss, unresolved requests, journal entries and budget caps. Preserve
   the coordinator and scheduled rental workload; discovery alone requires no
   coordinator restart.
5. Run the default public checker, then the explicit unpaid quote checks. Verify
   the expected quote-only journal changes and absence of acceptance-triggered
   provider/native/retail dispatch through private state evidence. Record the
   distinction between locally tested source, qualified image, live deployment,
   public discovery observations and any separately established payment proof.

If rollout fails, retain the candidate's essential payment recovery workers and
financial state while closing new admissions. Never restore an older monetary
snapshot over newer records. This procedure does not authorize deployment or
spending by itself.
