# Dedicated Knowledge paid acceptance

One authorized public-source compilation completed on the dedicated Knowledge
operator, using the signed public **Compute 0.5.1 Apple Silicon client**. Repeating
the identical key returned the same original job, signed receipt and snapshot,
with no additional job or charge. The observation completed at
**2026-10-20T22:39:42.469Z** (21 October in Copenhagen).

This run is separate from the [earlier two-job temporary-operator pilot](../paid-pilot-2026-10-20/report.md)
and the [dedicated operator activation](../remote-activation-2026-10-20/report.md).
It exercises normal completion and a settled replay, not client interruption,
service restart, sustained load or an uptime guarantee.

| Check | Observed result |
| --- | --- |
| Operator | `CfaV6nB59aDnTJccD5WXUiQejg9prMvtQrkFqQZRhWmf` |
| Original and replay job | `db3133b0-0f70-434d-a031-34b1e71ffff4` |
| Signed public client | `compute-v0.5.1`, Darwin ARM64, source `809aa264c2c1cb965c11001052096eece5caf0cf` |
| Source | 50 selected public files at the pinned `itsdangerous` commit; normalized source 294,697 bytes |
| Artifact | Independent Ed25519, original envelope, output, source, compiler and snapshot checks passed |
| Local import | Deterministic recompile passed; separate `knowledge verify` confirmed 50 files and 1,378 objects |
| Citations | All 1,378 object byte spans and 30 returned search citations matched captured UTF-8 source bytes and hashes |
| Replay | Same full signed receipt and snapshot; zero new job or debit |
| Operator counter | Released 0→1 for original, remained 1 across replay; faults 0 |
| Existing state | Existing commitments and historical purchase records preserved |
| Kani | Exact public capability still advertised by one operator at 19,000 micro-USDC per job |

The gross job charge was **50,000 micro-USDC (0.05 USDC)**: **45,000** to the operator
and **5,000** marketplace fee. The funding transfer and payout each cost
**5,000 lamports**, totaling **10,000 lamports (0.00001 SOL)** against a 20,000-lamport
ceiling. Both finalized, existing token accounts incurred no new rent, and the
installed public client returned `verified_onchain`. The separately authorized
stake extension is recorded independently and is excluded from these job fees.

Funding: [2kdoMUqoPpWpPLR7kydBsWT11kKangFzQXLHP9L5vX98Ypj4sXAmZknoVmg3pnsj3PHRbLnn34HMrBnAY66NtAME](https://explorer.solana.com/tx/2kdoMUqoPpWpPLR7kydBsWT11kKangFzQXLHP9L5vX98Ypj4sXAmZknoVmg3pnsj3PHRbLnn34HMrBnAY66NtAME).
Payout: [57zGGeEc7r4TiQU6Q4SqhesCbtB8Jd3gH17yWJm8fq39VzBouxVZdS4Vf9fXFMtwFNi6uSJWYyFGg1vb6MUrw8Ya](https://explorer.solana.com/tx/57zGGeEc7r4TiQU6Q4SqhesCbtB8Jd3gH17yWJm8fq39VzBouxVZdS4Vf9fXFMtwFNi6uSJWYyFGg1vb6MUrw8Ya).

The [machine-readable acceptance](acceptance.json), [operator-signed receipt](signed-receipt.json),
[artifact checks](artifact-integrity.json) and [fresh public capacity observation](capacity-after.json)
record exact hashes and observations. The source is
[`pallets/itsdangerous` at 672971d66a2ef9f85151e53283113f33d642dabd](https://github.com/pallets/itsdangerous/tree/672971d66a2ef9f85151e53283113f33d642dabd).
The compiler is `knowledge-compile-v1:d011ce03baa42e825e345bd028c4b9af82e210535958c4ce5b49eb8a0503745c`.

Normal purchase-book opening compacted duplicate journal rows and the budget
clock watermark advanced. Logical records and all existing commitments
were checked; no claim of whole-file byte equality is made. No service was
restarted or reconfigured. Operator release counts and buyer/coordinator
accounting were checked; this run did not collect a new host-level execution
trace. The initial read-only on-chain verification omitted an RPC URL and was
refused; specifying the public RPC passed without another transaction or job.

These corpus-specific counts do not establish general coverage or speedup.
Signed receipts bind execution and output, while citations identify captured
source; neither proves semantic correctness or confidential execution.
