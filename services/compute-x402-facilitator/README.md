# Covenant x402 payment verifier

Private Solana payment component for `covenant-compute-gateway`. This service uses
the pinned official `@x402/svm` 2.28.0 exact-payment verifier. It listens only on
loopback, requires a shared bearer secret, and does not expose a public
facilitator or signing endpoint.

## Deployment contract

Run Node 24 (`npm ci --omit=dev --ignore-scripts`, then `npm start`) alongside the
Rust `covenant-compute-gateway` binary. Keep one gateway replica with its durable
volume. Start the verifier before opening public gateway traffic.

Shared environment:

- `COVENANT_GATEWAY_BRIDGE_SECRET`: at least 32 random characters; persist across
  deployments because it authenticates the stored payment evidence.
- `COVENANT_GATEWAY_NETWORK`: Solana mainnet CAIP-2 identifier
  `solana:5eykt4UsFv8P8NJdTREpY1vzqKqZKvdp` (devnet available for rehearsal).
- `COVENANT_GATEWAY_ASSET`: exact official USDC mint for the selected network.
- `COVENANT_GATEWAY_PAY_TO`: merchant wallet address, with its USDC ATA created.
- `COVENANT_GATEWAY_FEE_PAYER`: dedicated funded facilitator public key.
- `COVENANT_GATEWAY_RESOURCE_URL`: canonical public HTTPS URL ending in
  `/v1/chat/completions`.
- `COVENANT_GATEWAY_PRICE_MICRO_USDC`: fixed customer price, positive integer.

Verifier only:

- `COVENANT_GATEWAY_SOLANA_RPC`: HTTPS RPC for the configured network; genesis is
  checked at startup and before each verification/settlement.
- `COVENANT_GATEWAY_FEE_PAYER_KEY_FILE`: private absolute JSON file containing the
  64-byte Solana keypair; no group/world permissions. Never put this in a repo.
- `COVENANT_GATEWAY_BRIDGE_PORT`: optional, default `4043`.

Rust gateway:

- `COVENANT_GATEWAY_ENABLED=true` and `COVENANT_GATEWAY_HOME`: absolute persistent
  directory; private identity, replay journal and exclusive process lock live here.
- `COVENANT_GATEWAY_BRIDGE_URL=http://127.0.0.1:4043`.
- `COVENANT_COMPUTE_COORDINATOR_URL`: actual funded coordinator endpoint.
- `COVENANT_COMPUTE_DAILY_BUDGET_MICRO_USDC` and `COVENANT_COMPUTE_BUDGET_HOME`: mandatory persistent rolling spend cap.
- `COVENANT_GATEWAY_RESERVE_MICRO_USDC`: lifetime operator-cost ceiling. Completed
  costs, uncertain work and known seller losses all consume it. Customer payments
  never automatically replenish it or imply stablecoin conversion.
- `COVENANT_GATEWAY_OFFER_MICRO_USDC`, `COVENANT_GATEWAY_MODEL`,
  `COVENANT_GATEWAY_MAX_TOKENS`: exact native inference offer and input limits.
- `COVENANT_GATEWAY_LISTEN`: public listener address, default `127.0.0.1:4042`.

To increase a funded lifetime reserve, verify the buyer's available coordinator
balance first, raise the configured ceiling and explicitly set
`COVENANT_GATEWAY_ALLOW_RESERVE_INCREASE=true` for that startup. The journal is
updated atomically with all prior entries retained. Reductions and unapproved
increases fail closed. Never delete, rotate or reset the journal to add capacity.
Changes to merchant, network, buyer, model, prices, fee payer or canonical URL need
an explicit migration/recovery review; they cannot silently reinterpret old work.

## Public protocol and recovery

POST `/v1/chat/completions` with `model`, text-only `messages`, `max_tokens`,
optional `temperature` and optional `pricing: "model_tariff_v1"`. A UUID
`Idempotency-Key` reuses the initial quote. A 402
responds with the standard Base64 `PAYMENT-REQUIRED` V2 header. Clients sign the
returned exact-SVM requirements and repeat the same request with
`PAYMENT-SIGNATURE`; no Covenant-specific payment header is required.

Each quote includes a memo in `extra.memo`, and the official x402 client
(`@x402/svm`) copies it into the payment transaction it signs. Once the verifier
accepts a payment, the gateway assigns Covenant Compute job IDs to the request
and records the payment and its jobs before any job starts; a retry of the same
paid request reuses them and is not charged again. The verifier adds its
fee-payer signature to a local copy of the transaction and never submits from
`/verify`. Before simulating or signing, it rejects any transaction with more than
two signatures, a compute-unit limit above 60,000 or a priority fee above 1,000
microlamports per unit, so Covenant pays at most 10,060 lamports in fees per
payment. Payments from smart wallets or through cross-program invocation (CPI)
are not supported.

The memo is the first 16 bytes, as 32 lowercase hexadecimal characters, of the
SHA-256 hash of `covenant.x402.v3:<quote ID>:<request hash>`, using `extra.quoteId`
and `extra.requestSha256` from the quote. The [deployment guide](../../deploy/compute-x402/README.md)
gives an example and covers quotes from earlier gateway versions, whose 118-byte
memo standard wallets cannot pay. In a read-only mainnet simulation, a payment
built by the official client at its fixed 20,000 compute units used 13,180. The
verifier accepts either memo when it matches the quote, and for both it runs the
official x402 checks on the signatures, memo and USDC transfer and simulates the
transaction against the live network.

Only output whose native operator receipt verifies can trigger customer
settlement. `/settle` always queries historical signature state before sending
exactly the persisted transaction bytes. Lost RPC responses, expiry and unknown
execution remain pending; they never mint a replacement payment or release a
result. Finalized success additionally requires an exact transaction readback.
Native operator payments are never reversed to make a failed customer charge
whole. The gateway bears that bounded reserve loss.

The Rust recovery worker resumes pending work after restart even without client
traffic. HTTP 202 means retry the identical body and original signed header.
Successful responses carry the standard Base64 `PAYMENT-RESPONSE` plus a native
signed envelope, signed receipt and exact output JSON for independent client
verification. Cached output expires after five minutes; permanent replay records
remain and return 410 rather than repurchasing.

`GET /health` is process liveness, not proof of funded inference capacity or a
successful paid transaction. Production acceptance must demonstrate those
separately. Set `COVENANT_GATEWAY_ADMISSIONS_ENABLED=false` on rollback; recovery and
settlement continue against the original durable state while new quotes and
payment admissions are refused. Keep `COVENANT_GATEWAY_ENABLED=true` so the
process and recovery worker stay alive.

## Focused verification

```sh
npm test --prefix services/compute-x402-facilitator
cargo test --manifest-path agent-os/Cargo.toml -p covenant-compute-gateway
```

Tests use real Solana signatures with the official verifier, mocked chain reads,
restart/concurrency fault cases and a real local HTTP server. They do not spend
funds or establish production acceptance.
