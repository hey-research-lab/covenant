# Paid Kani smoke on the retained 12 GiB profile

Native Covenant check `56f8bab8-d1e6-4231-9e76-7137574a3fa4` returned **proven**, accepted, after the provider stepdown. The exact signed order requests **12,288 MiB memory, 6 CPUs and 4,096 MiB scratch**. This locally authored tiny canary verifies the paid Kani receipt path on that profile. It does not establish a Percolator proof, Covenant authorship of the canary, or that the larger Percolator harnesses fit retained capacity.

Harness `receipt_smoke` selects an arbitrary `u8`, assumes it is below 10, requires the reachable `value == 4` cover and asserts `value < 10`. The raw export contains one successful assertion and the satisfied `smoke reachable` cover. The source also includes `receipt_refute`; this job did not execute it. No patch overlay or setup command was used.

The signed result reports exit code 0, no timeout, OOM or evidence overflow, and a 2,152 ms runner duration. The Kani export reports 135 ms. All **7,354 raw bytes** from the signed artifact package are retained: export, runner result, stdout and stderr. The plan permits 30 seconds for the harness, 300 seconds for the enclosing check and a 600,000 ms order deadline.

Original source commit `64b353b47a9549259c14ddb00e2e81bb91fc1936` and signed history-free snapshot `c441cb62c506ef4e2921ab4944a87149320977f5` have identical tree `8c2ba88ee1f50b53462940d3f9f5e5457b319c6b`. The exact source bundle is included, SHA-256 `11b3300ac6df43fb529608c502f0b1afe153976c9718719d24d40c8baeda27d2`.

Requested and resolved image is `ghcr.io/open-covenant/covenant/compute-node-public@sha256:5648c867341b054aebf3570266200cd49766caea7f5119d2735dc437fcfe3f7d`, with signed Kani 0.68.0, CBMC 6.11.0 and `nightly-2026-08-21-x86_64-unknown-linux-gnu` tool pins. Node source is `4dd036a4d59612326095768ff4b4f272aa48562d`; the coordinator remained at `983172ddd04ac2de70136125e2088f22c6edf505`. Actual buyer and verifier build identities are preserved separately with scoped input equivalence. The original larger-profile image/provenance records are historical; the signed order and `smoke-summary.json` bind this exact smaller profile.

Signed price and coordinator-reported charge are **50,000 micro-USDC**. Payout fields in `smoke-summary.json` remain coordinator reports; settlement has not been independently verified. Operator signatures authenticate recorded claims and exact artifacts, not physical hardware isolation or a complete activity inventory. Provider and service-state observations are separately identified unsigned operational evidence; they are not authenticated by the job receipt. `final-provider-unsigned-summary.json` records the observed 16 GiB cx43 host, the exact retained profile and healthy idle service. `provider-observation-provenance.json` records hashes of the private original audits and the later observation that the old 27 GiB capability was absent from live aggregate capacity. These are dated observations, not guarantees of future availability.

## Verify offline without spending

Obtain Covenant source at `4dd036a4d59612326095768ff4b4f272aa48562d`. The preserved verifier was actually built at `d997bb6436569d8dc96f963aaf72259b92ccb5ba`; see the scoped input-equivalence record.

```sh
python3 build-verifier.py --checkout /path/to/covenant --project /tmp/covenant-receipt-verifier
cp verifier.Cargo.lock /tmp/covenant-receipt-verifier/Cargo.lock
cargo build --locked --manifest-path /tmp/covenant-receipt-verifier/Cargo.toml
python3 verify-archive.py --archive . --verifier /tmp/covenant-receipt-verifier/target/debug/verify-public-receipt
```

Building may fetch Rust dependencies. Verification itself needs no buyer credentials, network request, provider capacity or payment. It checks signatures, source equality, exact plan/image/resource bindings and the full classified Kani export.

## Optional new paid smoke

The old released `compute-v0.2.0` binaries lack this native Kani interface despite sharing the version number. Build the buyer CLI from canonical source `4dd036a4d59612326095768ff4b4f272aa48562d` with its lockfile and record its binary hash; `pilot-cli-provenance.json` records the actual pilot build.

After offline verification, `python3 native-rerun-command.py --job jobs/native-kani-post-stepdown-canary --cli /path/to/covenant-compute --source-dir /path/to/new/source` prints source reconstruction and an exact no-spend `check --dry-run` command. It executes nothing. A separately opted-in paid rerun removes `--dry-run`, uses one fresh idempotency key and requires a funded buyer plus currently matching capacity. The printed paid command is commented out. Historical receipts do not guarantee future capacity.
