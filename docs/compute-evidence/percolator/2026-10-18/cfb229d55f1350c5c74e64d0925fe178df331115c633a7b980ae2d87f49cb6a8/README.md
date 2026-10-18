# Percolator #239: auto-crank attempt with unwind 17 ran out of memory

Native Covenant check `6040695e-4349-4fa9-98c9-dbdbd8b61e51` returned a signed **out-of-memory** result for `proof_v16_public_auto_crank_long_residue`. Acceptance is false. This execution establishes neither a proof nor a counterexample, and it does not establish the required covers.

The attempt used a 27 GiB proof limit (`27648 MiB`), 6 CPUs and `8192 MiB` scratch. Its signed plan selected `--tests --features fuzz`, cadical from the harness attribute, explicit unwind 17, and a 1200-second harness budget. The signed report duration is 980,970 ms; the enclosing command duration is 981,625 ms; `oom_killed=true`, `timed_out=false`, `evidence_overflow=false`, and `evidence_complete=false`. The Kani JSON export also reports `out_of_memory`; its property checks are empty. An outer-executor report records the failed attempt. The archive preserves all **2,606,221 available raw artifact bytes**, not a claim that a complete solver verdict was produced.

The original frozen source is `ea72a05d351207ce2eded5b517b049e186fed3aa`; the signed history-free snapshot is `895f9a8f0164d199584a1035297d7f5e0d00f8d4`. Both have tree `5703a742d9b3fca8038ae4f93ec74e0be2b7fc46`. The exact signed source bundle is included (SHA-256 `d05075704f9fc84b176cf5fa97ce577df9fd5ea7e0840ab8eecd08c123cb3c0d`). The input includes the unchanged accepted patch from authoring job `f79b455b-09e8-4a82-a971-fa00a3248100` (patch SHA-256 `8a1f74112d4097729afc18b2f83e9e7984e79fc55ead5dfcd4922ded261ac95b`). The authorship/checker chain is separately available in the immutable archive with inventory `9542384342c68fcbfa6ff3b7ac5d2d69e961bcc3232018ed337d787b79983beb`. It establishes paid authorship and ordinary acceptance, not Kani proof. The auto-crank input domain is the unchanged Long-side flat obligation, fixed `POS_SCALE` weight and shared residue `0..=3` on both sides.

Requested and resolved Kani image: `ghcr.io/open-covenant/covenant/compute-node-public@sha256:5648c867341b054aebf3570266200cd49766caea7f5119d2735dc437fcfe3f7d`. The signed tool pin is Kani 0.68.0, CBMC 6.11.0 and `nightly-2026-08-21-x86_64-unknown-linux-gnu`. Node source was `4dd036a4d59612326095768ff4b4f272aa48562d`; the coordinator remained at `983172ddd04ac2de70136125e2088f22c6edf505`. Actual buyer/verifier build identities and scoped source equivalence are preserved separately.

The receipt execution status is `ok`, meaning the check executor returned its recorded result; the proof was not accepted. The signed price and coordinator-reported charge are **50,000 micro-USDC**. The buyer budget journal keeps that ceiling counted. No payout was present in the collected coordinator response, and settlement has not been independently verified. Full original private budget/journal records remain private; `failure-summary.json` contains the relevant nonsensitive accounting fields.

## Verify offline without spending

Obtain Covenant source at `4dd036a4d59612326095768ff4b4f272aa48562d`. The existing verifier was actually built at `d997bb6436569d8dc96f963aaf72259b92ccb5ba`; see its scoped input-equivalence record.

```sh
python3 build-verifier.py --checkout /path/to/covenant --project /tmp/covenant-receipt-verifier
cp verifier.Cargo.lock /tmp/covenant-receipt-verifier/Cargo.lock
cargo build --locked --manifest-path /tmp/covenant-receipt-verifier/Cargo.toml
python3 verify-archive.py --archive . --verifier /tmp/covenant-receipt-verifier/target/debug/verify-public-receipt
```

Building may fetch Rust dependencies. Verification uses no buyer identity, coordinator request, RPC or payment. It recomputes source-tree equality, signatures, exact output/artifact bindings and the unsuccessful typed Kani result. Operator signatures authenticate the recorded claims; they do not independently establish physical isolation, execution integrity or settlement. Preserve this attempt unchanged when recording any later larger-resource run.
