# Paid Knowledge pilot acceptance

Two authorized public-source jobs completed on 20 October 2026 against the
production Compute coordinator. Both returned the same deterministic knowledge
artifact, passed independent signature and content verification, imported
locally, and settled on Solana. The [machine-readable record](acceptance.json)
contains the original job IDs, operator-signed receipts, payout proofs, source
revision and exact compiler and client hashes.

This is acceptance of a bounded pilot. It does not establish ongoing public
Knowledge operator capacity or a standing price. The existing operator was
temporarily assigned to the exact compiler; its original service is restored
after acceptance. Local Knowledge availability is a separate release gate.
The subsequent [dedicated operator activation](../remote-activation-2026-10-20/report.md)
records limited live capacity, cohosting, idle restart and an installed-client
preview separately. It does not turn these earlier receipts into jobs run by
the new identity.

## Observed behavior

| Check | Result |
| --- | --- |
| Anonymous public-source verification and quote | All 50 selected files matched the pinned public commit; zero-spend preview quoted 50,000 micro-USDC |
| Normal execution | Job `181df61f-4544-4f5d-a95d-cbff19a2ad57` completed, imported, and replayed with the identical receipt and no additional debit |
| Actual client restart | Job `408e1676-9cdd-4cdf-9afd-1da0fa8091dc` was acknowledged as offered; its client was stopped and killed, then a new process recovered the original signed envelope and key |
| Restarted job replay | The recovered job replayed with the identical signed receipt and unchanged buyer accounting |
| Artifact integrity | Ed25519 receipt signatures, original envelope hashes, output hashes, compiler digest, source hash and snapshot hash verified independently |
| Local import | Deterministic local recompilation accepted the artifact; `knowledge verify` confirmed 50 files and 1,378 objects |
| Source evidence | Every object span resolves to captured UTF-8 bytes with the recorded source hash |
| Settlement | Both payouts verified against finalized Solana transactions, including the receipt-bound memo, USDC mint, recipient and 45,000 micro-USDC operator amount |
| Charges | Two job charges of 50,000 micro-USDC; 100,000 micro-USDC total |
| Network fees | Funding 5,000 lamports plus two payouts of 5,000 lamports; 15,000 lamports total |

The source is
[`pallets/itsdangerous` at `672971d66a2ef9f85151e53283113f33d642dabd`](https://github.com/pallets/itsdangerous/tree/672971d66a2ef9f85151e53283113f33d642dabd).
Its captured snapshot contains 1,378 objects and 2,628 observed relationships:
1,374 resolved and 1,254 unresolved. It covers 15 Python files, four Markdown
files and 31 text-only files. These are corpus-specific counts, not coverage or
performance guarantees for other repositories.

The accepted compiler is
`knowledge-compile-v1:d011ce03baa42e825e345bd028c4b9af82e210535958c4ce5b49eb8a0503745c`.
The coordinator and pilot operator used deployment revision
`5146e46e6bae31bf6813ca85f099674e878a1ae9`. The Linux ARM64 compiler image ran
under host emulation with no network, a read-only root, nonroot user, dropped
capabilities and bounded resources. Host observations and execution counts are
retained separately alongside this report.

The paid buyer is the clean native 0.5.0 candidate identified in the JSON.
Public archive signing and fresh public installation have their own evidence.
This run does not imply that all four platform archives completed paid jobs.
After publication, the freshly installed and authenticated Apple Silicon
0.5.0 binary also [recovered both settled original receipts](public-installed-replay.json)
and deterministically re-imported the same snapshots. Both original envelopes,
budget entries and buyer accounting remained unchanged; no new job or charge
occurred. This read-only recovery succeeded after Knowledge capacity was
withdrawn. Its public binary SHA-256 is
`6cbe4f6ec099a8d15be82ca51b5687732fbef790c1f93f019f51b91962fc85b6`.

No repository scripts were executed. Private remote processing, shared daemon
project authorization, continuous remote supply and speedup claims remain
outside this acceptance.

Source citations identify captured source. Signed receipts identify execution
and its output. Neither establishes semantic correctness or confidential
execution.
