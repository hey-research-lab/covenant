# Compute inference edge transport

A dependency-free Cloudflare module Worker for `compute-inference.opencovenant.org`.
It forwards only `GET`/`HEAD /health` and `POST /v1/chat/completions` to the fixed
origin `https://covenant-compute-inference.onrender.com`. Query strings, other hosts,
methods and paths are refused before any outbound request. No storage, credentials,
bindings, logging or automatic retries are used.

The request body is bounded to 64 KiB before forwarding, including chunked input.
The combined `Payment-Signature` and legacy `X-Payment` values have a separate
32 KiB limit. Body bytes, payment headers, idempotency keys, response status and
response bytes pass through without JSON parsing or rewriting. Cookies, arbitrary
authorization, Host and forwarding headers are not copied to the origin.

Origin redirects are returned without following them. Transport failure returns
`502 origin_outcome_unknown`; the client must preserve the same request ID and
payment authorization because the origin may already have admitted the purchase.
An interrupted origin response remains interrupted; it is never fetched again.
All responses and origin fetches disable caching. The Wrangler configuration also
disables preview/Workers.dev routes and observability. Deployment uses the existing
Cloudflare account; the source contains no account credentials or secret bindings.

```sh
node --test deploy/compute-x402/edge/worker.test.mjs
```

The injected transport tests make no network calls. Actual edge verification should
first read `/health` and confirm a disabled inference request remains disabled,
then compare the public x402 resource URL and payment headers under the separately
authorized acceptance allowance. This source and its tests do not deploy a Worker
or activate paid admissions.

Cloudflare documents [fetch cache control and response passthrough](https://developers.cloudflare.com/workers/runtime-apis/fetch/)
and [manual response encoding](https://developers.cloudflare.com/workers/runtime-apis/response/).
