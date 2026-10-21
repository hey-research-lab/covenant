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

Being listed in x402scan's catalog is a separate question from whether
x402scan's built-in wallet can pay for Covenant Compute. That wallet uses the
official x402 client (`@x402/svm`), which copies the quote's memo into the payment
and always sets a compute-unit limit of 20,000. Quotes carry a 32-character memo,
and a payment built this way used 13,180 units in a read-only mainnet simulation.
Covenant's verifier tests accept a payment built by the unmodified official
client; as of 21 October 2026, no paid purchase has been made through x402scan's
own wallet.

Earlier gateway versions issue a 118-byte memo instead. Paying it needs 43,642
compute units, so x402scan's wallet cannot pay those quotes.

The supported purchase clients are Covenant's SDKs for
[JavaScript](../sdk/compute-x402-client/README.md) and
[Python](../sdk/compute-x402-python/README.md); the
[release test record](compute-x402-release-evidence.md) lists the paid purchases
made with them.

[x402scan PR #1297](https://github.com/Merit-Systems/x402scan/pull/1297), which
raised the scanner's limit for long memos, is superseded by the short memo.

## Logo and banner metadata

The gateway's root page publishes the merchant title, description, favicon and
Open Graph image at the same origin as inference. The PNG icon and banner have
fixed, content-fingerprinted routes; `/favicon.ico` provides the conventional
fallback. These GET/HEAD routes serve embedded assets, need no credentials, and
remain available when admissions or payment dependencies are unavailable. They
do not read or change quote journals, check model capacity, or dispatch work.
The root's canonical and image URLs use the configured payment-resource origin,
never an incoming Host header. No current prices or availability are cached in
the root page. HTML, CSS and the favicon cache for five minutes; fingerprinted
PNGs cache immutably. The content security policy permits only same-origin
images and styles, with no scripts, forms, framing, or external asset requests.

The assets are unchanged copies of Covenant's official branding:

| Embedded asset | Canonical source | SHA-256 |
| --- | --- | --- |
| `branding/favicon.ico` | Main Covenant site's `landing/app/favicon.ico` | `2b5136747b64db4eba4bd7f43c41af6c970bdd4680f4d9c0af4854174c9e4d1a` |
| `branding/covenant-icon-02026997.png` | Main Covenant site's `landing/app/icon.png`, 512 × 512 | `02026997226a417d29b16bd90b19893965217e0ed3d71b80daf7ab1cba10264a` |
| `branding/covenant-banner-4b58cf6c.png` | Covenant's `assets/brand/covenant-banner.png`, 3000 × 1000 | `4b58cf6c68c0179b911953aef8dc1aac872860272de2ac0bd1db3104d7d11d4c` |

The icon and favicon were also checked byte-for-byte against the public main
Covenant site. The compute branch's older `landing` icons are not the current
Covenant mark and are not used here.

The pinned scanner's [origin scraper](https://github.com/Merit-Systems/x402scan/blob/5732bb83c8c4290f413f24318a838e3fee4ed2bf/apps/scan/src/services/scraper/index.ts)
reads HTML metadata and favicon links from the inference origin. Its base-domain
fallback applies only to `api.*` subdomains. Its
[server header](https://github.com/Merit-Systems/x402scan/blob/5732bb83c8c4290f413f24318a838e3fee4ed2bf/apps/scan/src/app/%28app%29/server/%5Bid%5D/%28overview%29/_components/header/index.tsx)
displays the icon; desktop origin cards display the first OG image, and the
server's social previews use that image. The server header has no full-width
merchant banner slot. Publishing OG metadata makes the official banner available
where those surfaces support it; it does not change x402scan's page layout.

After deployment, the existing server page's **Refresh** button re-runs origin
registration and scrapes the metadata again. This can create unpaid quote
probes, so use the same reconciliation procedure below. Reload the page after
success: the pinned refresh hook invalidates resource lists but does not refresh
the server header's `origins.get` query. Verify the stored favicon and OG image
URLs plus the rendered public result, rather than treating a refresh notification
as proof that branding is visible.

## Release and listing acceptance

1. Run gateway tests, strict Clippy, public acceptance fixtures, and repository
   guards. Validate the generated OpenAPI and unchanged 402 serializer with the
   pinned indexer parser using only isolated synthetic responses.
2. Qualify an immutable image. Discovery smoke checks the empty OpenAPI paths,
   manifest and model catalog before and after persistent-volume restart, with
   admissions and paid dispatch disabled. Enable `SMOKE_BRANDING=true` for this
   full image: it also verifies bounded root/asset GETs and HEADs, exact official
   binary hashes, canonical HTML metadata and unchanged retained state after
   both rounds. The result must report `branding_reads_verified: true`.
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
