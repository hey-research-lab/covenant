# Percolator #239: interrupted forfeit attempt with unwind 17

Native Covenant check `0bd8b774-78e7-44bd-a52d-40286b63efba` returned signed **tool_error**, acceptance false, for `proof_v16_public_forfeit_long_flat_residue`. Its reason is `property counter passed missing or not an integer`. The signed report says `oom_killed=false` and `timed_out=false`. This establishes neither a proof nor a counterexample and does not establish the required covers.

Separately collected operator records report one SIGTERM to the exact CBMC process at 2026-10-18 01:41:40.439 UTC after sustained memory pressure and negligible observed progress. That event is operator audit evidence, not a claim authenticated by the job receipt. The event summary and hashes of the privately preserved originals are in `operator-interruption-summary.json`. The raw signed result was not changed or relabelled as timeout or OOM.

The attempt used a 27 GiB proof limit (`27648 MiB`), 6 CPUs and `8192 MiB` scratch. The signed plan selected `--tests --features fuzz`, explicit `--unwind 17` and a 3600-second harness budget; the enclosing check budget was 4200 seconds. Reported duration was 1,851,274 ms, exit code 1, with no evidence overflow. Kani exported empty property checks, unavailable property counters and `verification_failure` / `exit_code_15`. All **2,519,217 available raw artifact bytes** are preserved, including stdout, stderr, the export and runner result. This is complete preservation of the available artifacts, not a completed solver verdict. The report does not contain an `evidence_complete` field; none is inferred.

Original frozen source: `ea72a05d351207ce2eded5b517b049e186fed3aa`. Signed history-free snapshot: `895f9a8f0164d199584a1035297d7f5e0d00f8d4`. Both have tree `5703a742d9b3fca8038ae4f93ec74e0be2b7fc46`. The exact signed source bundle is included. It carries the unchanged accepted patch from authoring job `f79b455b-09e8-4a82-a971-fa00a3248100`, SHA-256 `8a1f74112d4097729afc18b2f83e9e7984e79fc55ead5dfcd4922ded261ac95b`. The author/checker chain is separate evidence. This attempt changed the proof-execution bound from the prior attempt; it did not change the proof source or domain.

Requested and resolved Kani image: `ghcr.io/open-covenant/covenant/compute-node-public@sha256:5648c867341b054aebf3570266200cd49766caea7f5119d2735dc437fcfe3f7d`. The signed tool pin is Kani 0.68.0, CBMC 6.11.0 and `nightly-2026-08-21-x86_64-unknown-linux-gnu`. Node source was `4dd036a4d59612326095768ff4b4f272aa48562d`; coordinator source remained `983172ddd04ac2de70136125e2088f22c6edf505`. Actual buyer/verifier builds and scoped input equivalence are preserved separately.

Receipt execution status is `ok`: the checker executor returned its recorded result, while proof acceptance was false. Signed price and coordinator-reported charge are **50,000 micro-USDC**; that ceiling remains counted in the buyer budget. No payout was present in the collected coordinator response. Settlement has not been independently verified.

## Verify offline without spending

Obtain Covenant source at `4dd036a4d59612326095768ff4b4f272aa48562d`. The preserved verifier was actually built at `d997bb6436569d8dc96f963aaf72259b92ccb5ba`; see the scoped input-equivalence record.

```sh
python3 build-verifier.py --checkout /path/to/covenant --project /tmp/covenant-receipt-verifier
cp verifier.Cargo.lock /tmp/covenant-receipt-verifier/Cargo.lock
cargo build --locked --manifest-path /tmp/covenant-receipt-verifier/Cargo.toml
python3 verify-archive.py --archive . --verifier /tmp/covenant-receipt-verifier/target/debug/verify-public-receipt
```

Building may fetch Rust dependencies. Verification uses no buyer identity, coordinator request, RPC or payment. It recomputes source-tree equality, signatures and exact output/artifact bindings. It authenticates the unsuccessful typed result, not a proof. Operator signatures do not independently establish execution integrity, physical isolation, operator audit truth or settlement. Preserve this attempt unchanged when recording later work.
