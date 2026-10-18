# PR #239 public-route proof appendix: reduced domains

Base: `81fabc86869d0a4db7da3fabc7eab5430b1f02c5`. Appendix lives at the end of `tests/proofs_v16.rs`.

## Provenance

The PR #239 engine fix and the two original reference harnesses
(`proof_v16_public_recovery_forfeit_of_last_leg_resets_legless_asset`,
`proof_v16_public_auto_crank_release_of_last_obligation_resets_legless_asset`)
predate this job and were authored locally. This appendix only authors the
new reduced harnesses below. No production code was edited.

## Status

At the authorship checkpoint, Kani results were not yet established; later native receipt evidence is recorded separately.
No timeout or compile-only success is a proof. Prior 32GB runs: auto-crank timed out at 30 minutes; the forfeit run lost its connection without a confirmed outcome. Concrete sweeps are not proofs of the symbolic harnesses.

## Narrowed slice

These are narrowed slices, not the original full size/weight domain. Short-side and live-position forfeit cases, and short-side/variable-weight auto-crank cases, are deferred coverage and are not claimed.

### `proof_v16_public_forfeit_long_flat_residue`
- Fixed: Long side, flat basis=0 (pending obligation), loss_weight=POS_SCALE, Recovery lifecycle.
- Symbolic: `long_residue: u8` and `short_residue: u8`, each 0..=3, independent.
- Public call: `forfeit_recovery_leg_not_atomic`.
- Covers (after the call, require `result.is_ok()`): "pr239 forfeit zero residue", "pr239 forfeit long-only residue", "pr239 forfeit short-only residue", "pr239 forfeit two-sided residue".
- Single final assertion message: `PR239_FORFEIT_LONG_FLAT_RELEASE_INVARIANTS`.

### `proof_v16_public_auto_crank_long_residue`
- Fixed: Long side, weight=POS_SCALE, barrier-free flat obligation (`install_flat_pending_obligation`).
- Symbolic: `residue: u8` in 0..=3, shared on both Live sides.
- Public call: `permissionless_auto_crank_not_atomic` with the reference workload.
- Covers (after the call, require `result.is_ok()`): "pr239 auto-crank zero residue", "pr239 auto-crank two-sided residue".
- Single final assertion message: `PR239_AUTO_CRANK_LONG_RELEASE_INVARIANTS`.

Both assertions conjoin the expected public result, both-side OI zero, zero weights, zero stored/pending counts, ResetPending modes, risk_epoch+2, detached account bitmap, zero payout blocker, and no movement of vault/c_tot/insurance/account capital/pnl. Both use `#[kani::unwind(24)]`.

## Intended acceptance

- Ordinary: `cargo test --offline --locked --features fuzz --test v16_spec_tests` passes; structural buyer check of prefix, names, labels.
- Later native Kani (each harness separately): `cargo kani --tests --features fuzz --harness <name>`. `fuzz` is required by declared integration-test targets, adds no dependency, and leaves the relevant production cfg gates unchanged under cfg(kani), matching the original host wrapper.
- Regression control: the identical harness source is transplanted onto buggy base `4db11a8cb0053815e23a35d3a7d3edc265d8d866` without changing assertions; the named assertion must be the only failure, and covers must remain reachable.
- Green native runs and buggy-base refutation are mandatory before publishing.
