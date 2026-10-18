# Percolator #239: Covenant-authored reduced harnesses

Covenant authoring job `f79b455b-09e8-4a82-a971-fa00a3248100` returned the exact 12,172-byte patch in `jobs/pr239-authorship-v2/authored.patch` (SHA-256 `8a1f74112d4097729afc18b2f83e9e7984e79fc55ead5dfcd4922ded261ac95b`). Linked checker `785892f2-264c-436f-b5fe-d639ef1ff811` accepted it after 159 ordinary specification tests and the exact buyer acceptance contract passed. The full signed parent/checker records are included.

This is an **authorship and ordinary-check receipt**, not a Kani result. The author reported that Kani was not run, and the linked checker did not run Kani. Later source freezing, proof runs and buggy-base controls are separate evidence. The engine fix and original broad reference harnesses were authored locally before this job. The buyer prepared the task, domain constraints and hidden acceptance contract locally. This archive attests only the returned patch and recorded checker output; it does not attribute those earlier inputs or later integration changes to the paid author.

## Source and returned change

The original input is `81fabc86869d0a4db7da3fabc7eab5430b1f02c5`, and the signed history-free snapshot is `b3a59d131e8669d4b8a03e4cca6ce55be433b823`. Both have tree `6b2882d92ffad6e8df932acae4a0a3d356a1d372`. The original commit bytes, exact signed source bundle and readable source files are preserved. Offline verification checks their equality without fetching Percolator.

The returned patch appends two harnesses to `tests/proofs_v16.rs` and adds `docs/proofs/pr239-route-domains.md`. The forfeit slice fixes the Long side, a flat pending obligation and weight `POS_SCALE`, with independent symbolic long/short residues in `0..=3`. The auto-crank slice fixes a Long-side flat obligation and weight `POS_SCALE`, with a shared symbolic residue in `0..=3`. The harnesses specify unwind 24 and cadical. Their named release/value assertions and six post-call cover labels are source content, not solver verdicts. Short-side, live-position and variable-weight cases remain outside these narrowed slices.

The raw buyer task has SHA-256 `fe8385440a84e2e13b5eaed19f484fa26e11dfb45e96e2fe17f289b5065a9a57`; the signed task has SHA-256 `9bf24e644247931ae693b501a7a4d915fb0c9f3dbfcb389c697f4262ecadb855`. The native CLI removed exactly one final newline; all other task bytes match.

## Acceptance and trust

The signed author operator is `HuTP2kfzhns9p3ZvmuZxGzcgYan7z4LaZVytgcG9VVuh`; the linked checker operator is `8N238CskW3qA8uAyJNYpMMovoedsSKycXhkZLmFLsi7m`. These are different signing keys. That alone does not establish independently owned infrastructure. The requested generic checker image was `covenant-compute-check:rust`, a tag; these receipts do not attest an immutable image digest or complete toolchain pin.

The buyer's locally prepared hidden contract is disclosed at `jobs/pr239-authorship-v2/disclosed-hidden/.covenant-pr239/acceptance.py`. The verifier requires the exact original base, single file, content hash, bundle hash and command in `hidden-disclosure.json`, and binds it to every linked signed checker order. Disclosure was reviewed for this archive only. No general hidden-input bypass is enabled.

The coordinator reported a parent charge of **331,280 micro-USDC**, below the signed **2,000,000 micro-USDC ceiling**. The linked checker receipt carries 5,000 micro-USDC; the parent charge already includes its check fee. Worker-reported model spend is a separate field. Transaction references and payout metadata are preserved as coordinator reports; settlement has not been independently verified.

Signatures authenticate the recorded operators' claims and bind the exact orders, outputs, source and returned patch. They do not independently establish execution integrity, physical isolation, complete job inventory or settlement. Generic command evidence contains the protocol's bounded `output_tail` fields, not complete raw process logs or an author transcript.

## Verify offline, without a purchase

Obtain Covenant source at `4dd036a4d59612326095768ff4b4f272aa48562d`. The existing verifier was actually built at `d997bb6436569d8dc96f963aaf72259b92ccb5ba`; `verifier-merge-equivalence-4dd036a.json` records the exact scoped input equivalence. Do not infer whole-tree equivalence or identify this feature by the shared `0.2.0` version string.

```sh
python3 build-verifier.py --checkout /path/to/covenant --project /tmp/covenant-receipt-verifier
cp verifier.Cargo.lock /tmp/covenant-receipt-verifier/Cargo.lock
cargo build --locked --manifest-path /tmp/covenant-receipt-verifier/Cargo.toml
python3 verify-archive.py --archive . --verifier /tmp/covenant-receipt-verifier/target/debug/verify-public-receipt
```

Building may fetch Rust dependencies. Verification itself uses no buyer identity, coordinator request, RPC or payment. The archive must be reviewed and all parent/child execution windows must expire before public export. Publication is separate from local preparation.
