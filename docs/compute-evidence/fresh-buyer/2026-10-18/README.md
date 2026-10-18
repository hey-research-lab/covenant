# Fresh-buyer Kani rehearsal — 2026-10-18

One new buyer bought the public `receipt_smoke` Kani fixture through Covenant
Compute. Job `e996644b-b3a6-4762-a2ab-0abdc6bc0e29` proved the assertion and
satisfied the `smoke reachable` cover. The complete signed record passed the
unchanged public offline verifier. A separate `output` fetch matched the original
receipt and output exactly.

This is an internal fresh-buyer rehearsal, not an external customer or a claim
that Covenant authored the fixture. The CLI budget fix and documentation were
authored locally; the proof execution was purchased through Covenant. There was
no operator SSH, access-policy change, worker resize or deployment for this test.

## Reproduce and verify

Follow the [buyer walkthrough](../../../compute-kani-smoke.md) for source
installation, funding, a new bounded purchase, retrieval and offline verification.
The tested buyer was built from public commit
`832210f8239cd5bd52f0451246d80e3355e9e916`, using an isolated copy of an existing
Cargo dependency cache. The public documentation adapter was fetched at
`6488fc6e7cd6b5143b86e8367a2ff929dabaf7b9`. Its source-bundle/tree checks and the
verifier ran locally without any network or buyer credentials during verification.

To verify this saved execution without buying another job, build the public
verifier as described in the walkthrough, then run:

```sh
cd docs/compute-evidence/fresh-buyer/2026-10-18
shasum -a 256 -c SHA256SUMS
"$KANI_VERIFIER" native-verifier-record.json
```

Require zero exit status, `acceptance_passed: true`, `kani.accepted: true` and
`kani.outcome: "proven"`. The record contains the unchanged signed buyer envelope,
signed receipt, complete output and explicit expected metadata. Its inline source
bundle has tree `8c2ba88ee1f50b53462940d3f9f5e5457b319c6b`. The exact image and
12,288 MiB / 6 CPU / 4,096 MiB profile appear in the preview and signed record.
Hashes detect file changes; they do not establish completeness of a wider campaign.

## Actual path and cost

- A fresh identity and separate persistent budget were created through the CLI.
  The initial unfunded order returned HTTP 402. Funding used the public CLI's
  deposit instructions, an official SPL-token transfer with the buyer memo, and
  the public `deposit` claim command.
- Replaying the first order after funding returned `operator_rejected`, charged
  zero. Its timestamp was older than the 120-second admission window, consistent
  with expiry; the public response did not expose a detailed signed reason. Its
  50,000 micro-USDC reservation remains held because unsigned rejection does not
  authorize budget release.
- An attempt to change the budget through the environment was refused locally.
  That second journaled job returned public HTTP 404 and was never submitted.
  Its aged envelope was preserved.
- The new `budget set-limit 100000 --expect-limit 50000` command changed only the
  persisted limit. Every prior entry and the clock watermark were preserved.
  A separately recorded fresh order then completed successfully for 50,000
  micro-USDC (0.05 USDC). No second successful job was purchased.
- Of 100,000 micro-USDC funded, 50,000 was charged and the unused 50,000 returned
  to the original payer. The return transaction was independently checked as
  finalized with the exact USDC balance changes. The buyer's final balance is zero.
  See [cleanup verification](cleanup-verification.json) and
  [final balance](balance-final.json).

The prior campaign's conservative 4,650,000 micro-USDC total plus the preserved
50,000 hold and this 50,000 execution is 4,750,000 against its original 5,000,000
ceiling. This accounting retains the rejected hold despite the zero actual charge.

## What this establishes

The [offline result](independent-verification.json) authenticates the buyer and
operator signatures, exact envelope/output commitments, source and plan binding,
typed Kani outcome, raw proof artifacts and required cover. It does not establish
hardware isolation, external demand, complete campaign inventory or on-chain job
payout settlement. Coordinator charge observations and the finalized unused-funds
return are recorded separately from proof verification.

The path is repeatable but still requires a source build, an external funding
wallet, a public archived capability descriptor and a documented offline adapter.
The walkthrough now puts funding before signing and explains conservative holds.
This rehearsal stopped after its one accepted smoke; it did not rerun Percolator.
