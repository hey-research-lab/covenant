# Knowledge operator stake extension

Result: **PASS**. Independently verified 2026-10-20T22:39:06.133Z (21 October 2026 in Europe/Copenhagen).

The existing Knowledge operator's 1,000,000 CVNT stake was extended to the user's approved fixed timestamp, **19 November 2026 at 23:10:13 CET** (`2026-11-19T22:10:13Z`, Unix `1795126213`). Under the accepted 48-hour remaining-lock admission requirement, its new cutoff is **17 November 2026 at 23:10:13 CET** (`2026-11-17T22:10:13Z`). Operator health and available capacity remain separate conditions.

The [finalized Solana transaction](https://explorer.solana.com/tx/5j9KJ1TCvvfz2hZj3cVp8HJqJ18R1KMgVKsBxZKjkFRFur49PY2pdRo8bfdN9EvytcbZZ83Q4nJFerSB4oaFhqxS) landed at slot `459398415`, block time `2026-10-20T22:37:16.000Z`. Its exact wire bytes match the immutable intent persisted before submission; its owner signature and reconstructed three-instruction message verify independently. The instructions are the released compute limit and priority settings followed by `ExtendStake` with zero additional tokens and the exact approved expiry.

| Verified effect | Result |
| --- | --- |
| Retained stake | 1,000,000 CVNT |
| Additional CVNT | 0 |
| Source and vault token balances | Unchanged |
| Transaction fee | 25,000 lamports / 0.000025 SOL |
| Other lamport changes | 0 |
| New accounts or rent | None |
| Owner, operator and vault | Unchanged |
| Position | Active; exact approved expiry |

Operator: `CfaV6nB59aDnTJccD5WXUiQejg9prMvtQrkFqQZRhWmf`. The [machine-readable record](extension.json) includes the public program, mint, owner and derived position/vault bindings, exact transaction/message hashes, finalized status and checked account effects. It excludes private wallet balances, keys, signed wire data and local journal paths.

The preceding lock ended on 27 October at 19:17:25 UTC, with admission eligibility ending two days earlier. The new fixed date follows the approved 30-day-from-preparation proposal, including 600 seconds of clock slack; it does not add 30 days to the old expiry. The same program-controlled vault and protocol slash authority remain in place. The lock cannot be shortened; expiry does not automatically withdraw tokens. No automatic renewal was created.

## Public capacity after the extension

Unauthenticated public reads at 2026-10-20T22:39:06.134Z found:

- The [dedicated Knowledge operator](https://compute-api.opencovenant.org/federation/operators/CfaV6nB59aDnTJccD5WXUiQejg9prMvtQrkFqQZRhWmf/reputation) registered, staked, online and matchable.
- [Public capacity](https://compute-api.opencovenant.org/federation/capacity) advertised one Knowledge operator for exact compiler `d011ce03baa42e825e345bd028c4b9af82e210535958c4ce5b49eb8a0503745c`, with an observed 50,000-micro-USDC per-job ask.
- The same capacity response retained one Kani operator, with an observed 19,000-micro-USDC per-job ask.

These are dated observations, not standing prices or uptime guarantees. The verification used ordinary coordinator refresh, with no service restart, new transaction or job. The [initial dedicated activation](../remote-activation-2026-10-20/report.md) remains historical evidence of its earlier lock and cutoff. Paid jobs and replay evidence are recorded separately; this stake-extension record does not establish or authorize a new purchase.
