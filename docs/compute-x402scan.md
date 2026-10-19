# x402scan registration

Covenant Compute exposes `/openapi.json` for x402scan's OpenAPI-first discovery.
The existing `/.well-known/x402.json` manifest remains available to its existing
clients. OpenAPI includes only currently observed payable offers and uses the
same uncached, read-only availability checks. It never creates a quote or
dispatches inference.

## Compatibility

The integration was checked against
[x402scan source 5732bb83](https://github.com/Merit-Systems/x402scan/tree/5732bb83c8c4290f413f24318a838e3fee4ed2bf)
and its pinned `@agentcash/discovery` 1.7.5 package. See the upstream
[discovery spec](https://github.com/Merit-Systems/x402scan/blob/5732bb83c8c4290f413f24318a838e3fee4ed2bf/docs/DISCOVERY.md),
[probe fallback](https://github.com/Merit-Systems/x402scan/blob/5732bb83c8c4290f413f24318a838e3fee4ed2bf/apps/scan/src/lib/discovery/probe.ts),
and [sample builder](https://github.com/Merit-Systems/x402scan/blob/5732bb83c8c4290f413f24318a838e3fee4ed2bf/apps/scan/src/lib/discovery/utils/build-minimal-sample.ts).

The scanner first probes without a body. If that fails, it samples required
OpenAPI properties and probes again. Property-level examples and the first
pricing branch yield a valid text-chat request. The real gateway then issues its
normal body-bound HTTP 402 quote. OpenAPI supplies the invocation schema when the
402 response has no Bazaar extension. Request validation, quote persistence,
price calculation, payment authorization and settlement are unchanged.

The preferred example uses `model_tariff_v1` when available, with an output cap
of at most 256 tokens. Legacy requests remain valid by omitting `pricing`.
Each schema branch retains its own model and affordable output cap. Published
price bounds cover all current branches and use exact integer micro-USDC
conversion. Runtime quotes remain authoritative.

## Purchase clients

Catalog registration and x402scan's built-in wallet are separate compatibility
claims. The pinned scanner uses `@x402/svm` 2.11.0, which preserves the quoted
memo but sets a 20,000 compute-unit limit. Covenant's previously accepted payment
used 43,642 units; its client adapter uses 60,000. Therefore the scanner's stock
wallet is expected to fail payment simulation until its client supports the
higher limit. No new paid wallet test was performed for this listing change.
The OpenAPI document identifies the
[Covenant SDK](../sdk/compute-x402-client/README.md) as the supported purchase
client. See the [paid release evidence](compute-x402-release-evidence.md) for the
qualified payment path. This listing change does not modify either payment
client or claim paid invocation through x402scan.

## Release and listing acceptance

1. Run gateway tests, strict Clippy, public acceptance fixtures, and repository
   guards. Validate the generated OpenAPI and unchanged 402 serializer with the
   pinned indexer parser using only isolated synthetic responses.
2. Qualify an immutable image. Discovery smoke checks the empty OpenAPI paths,
   manifest and model catalog before and after persistent-volume restart, with
   admissions and paid dispatch disabled.
3. Follow the existing [image rollout gates](../deploy/compute-x402/DISCOVERY.md).
   The previous discovery rollout's allowance is closed; it does not authorize
   the new image's normal startup qualification probe. Preserve live policy,
   tariff validity, identities, accounting and retained financial state.
4. After the approved image is live, verify `/openapi.json` and the existing
   public discovery acceptance checks. Submit the inference origin through
   [Add API](https://www.x402scan.com/resources/register).
5. Registration can create unpaid quotes through method-aware probes. Reconcile
   their retained entries and normal expiry; verify that registration caused no
   provider execution or customer settlement. Inspect an uncertain response
   before resubmitting. Do not submit payment to obtain a listing.
6. Record the actual indexed server/resource URL and verify the public result.
   A passing local parser or accepted submission is not proof of a public
   listing, paid execution through x402scan, or ongoing indexer adoption.
