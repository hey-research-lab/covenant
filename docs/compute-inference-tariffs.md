# Versioned inference tariffs

Inference tariffs quote a fixed price before execution:

```text
base_micro_usdc + ceil(requested_max_tokens × output_cap_micro_usdc_per_million / 1,000,000)
```

The full agreed amount settles on successful execution. The output limit determines
the quote; a shorter answer does not reduce it. This is not billing for measured
input tokens or generated output tokens.

## Explicit x402 opt-in

A caller requests the model tariff by adding `"pricing": "model_tariff_v1"` to its
chat completion request, alongside the exact model and a positive `max_tokens`:

```json
{
  "model": "claude-haiku-4.5",
  "messages": [{"role": "user", "content": "Summarize this paragraph."}],
  "max_tokens": 256,
  "pricing": "model_tariff_v1"
}
```

Omitting `pricing` preserves the existing flat offer: 50,000 micro-USDC retail and
30,000 micro-USDC native settlement for the currently qualified Haiku model. This
keeps published clients' existing requests and persisted purchases compatible.
The new field requires a client version that accepts and verifies tariff metadata;
an earlier installed client is not evidence that the opt-in path is supported.

The tariff quote carries its identifier, revision, exact model, requested cap,
validity window, and exact price. The buyer still applies its own payment and total
spending ceilings. A ceiling is not the amount charged. Changing the pricing mode,
model, or requested cap changes the purchase and requires a new purchase UUID.
Idempotent retries retain their original signed request and tariff snapshot.

New x402 payment quotes remain valid for at most 75 seconds from the prepared
native envelope's issue time. This leaves a 45-second delivery margin inside the
native protocol's 120-second freshness window. A tariff nearing its own expiry
shortens the quote further. Payment admission rechecks both the native deadline
and tariff window before dispatch; the quoted expiry is an upper bound, not a
promise of capacity. Reconciliation of an already submitted payment continues
to use its original stored execution and transaction.

The release adds `GET /v1/models` for the gateway's currently valid tariff metadata.
It is an unpaid metadata read, not a capacity reservation or proof that a provider
will accept the next request. A missing, expired, incompatible, or over-ceiling
tariff fails the opt-in request; it does not silently switch to flat pricing.

## Initial commercial policy

The proposed first tariff covers only the existing qualified `claude-haiku-4.5`
text model, with output caps from 1 through 2,048. It adds no models or modalities.

| Price | Base, micro-USDC | Output-cap rate, micro-USDC per million |
|---|---:|---:|
| Native operator settlement | 10,000 | 7,500,000 |
| Customer retail quote | 12,000 | 9,000,000 |

The retail coefficients apply a deliberate 20% markup to the native coefficients;
integer rounding can make the final microunit ratio differ slightly. The base is
a commercial request fee with input and service headroom, not a measurement of
each request's input cost. The native output coefficient uses the existing upstream
price ceiling. See [the evidence and release gates](compute-model-pricing-release-evidence.md)
for measured samples, pricing references, and limitations.

| Requested `max_tokens` | Native USDC | Retail USDC |
|---:|---:|---:|
| 1 | 0.010008 | 0.012009 |
| 256 | 0.011920 | 0.014304 |
| 512 | 0.013840 | 0.016608 |
| 1,024 | 0.017680 | 0.021216 |
| 2,048 | 0.025360 | 0.030432 |

These are candidate release prices. Source files and local checks alone do not
establish that the gateway is serving them. Live deployment and paid acceptance
must be recorded separately in the release evidence.

## Native signed tariffs

An operator sets `COVENANT_COMPUTE_NODE_TARIFFS_FILE` to a JSON array of reviewed
tariff definitions. The protocol has no built-in model prices. Each definition
requires `schema_version: 1`, a stable `id`, positive `revision`, exact `model`,
`base_micro_usdc`, `output_cap_micro_usdc_per_million`, positive
`max_output_tokens`, and an explicit inclusive `valid_from_ms` / exclusive
`valid_until_ms` window. A price change requires a new revision. Operator
registration signs the definition.

Public x402 configuration additionally requires both retail and native tariff
revisions, base/rate values, validity timestamps, and maximum computed prices to
fit JavaScript's safe integer range: at most `9007199254740991`. The gateway
rejects out-of-range values instead of allowing client-side rounding. Native-only
tariffs retain the protocol's checked unsigned-64-bit price representation.

`GET /federation/capacity` includes live `inference_tariffs` offers. Each offer
keeps its tariff, GPU class, and VRAM together. Hardware remains operator-declared,
and a quote is not a reservation.

Native CLI callers use `infer --tariff-id ID --model MODEL --max-tokens N`.
MCP `compute.infer` and `compute.stream_start` use `tariff_id`, `model`, and
`max_tokens`; library clients resolve the tariff before signing. The signed
envelope binds the tariff, model, requested cap, price, and issue time. Routing
and node admission require the advertised snapshot and a valid window. Legacy
profiles omit the new empty field and legacy envelopes omit the optional
requirement, preserving their signed JSON and flat-offer behavior.

## Provider accounting stays separate

Provider spending limits and durable reservations are unchanged. The bridge still
reserves its configured full 200,000 input-token allowance plus the requested
output cap at the configured upstream price ceilings. A 4,096-byte message limit
has not been qualified as a tokenizer or a smaller upstream billing bound.

Provider usage is used for conservative internal cost accounting after a valid
response. It does not reprice the customer's signed quote. Unknown provider or
payment outcomes remain reserved and require reconciliation; a tariff update does
not reset budgets, release old reservations, or permit automatic retries.
