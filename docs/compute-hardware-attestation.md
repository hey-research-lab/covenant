# Hardware evidence and release status

The `covenant-compute-attestation` verifier checks raw AMD SEV-SNP, Intel TDX/DCAP and NVIDIA H100 CC reports cryptographically. Certificate chains, report signatures, reference measurements, TCB floors, debug/secure-boot/CC policy, fresh nonce and report-data bindings all participate. Decoding a report or observing `tee_capable` never earns trust.

The Covenant wrapper binds the full UUID, buyer, assigned operator, lease generation, endpoint, typed SSH host key, workload SHA-256 and lease expiry into guest report-data (SHA-512, distinct Covenant domain) and GPU nonce (SHA-256, separate domain). Combined evidence additionally commits the exact GPU report and chain into the guest report-data via `guest_report_data_for_gpu`, so an independent guest report cannot be combined with a different GPU report. The coordinator persists and consumes each challenge before verification, rejects clock rollback/corrupt journals, limits concurrent verification without an unbounded wait queue, rechecks the current lease before persisting a result and before every protected workspace grant, and prevents one verified GPU being assigned to different operators during a live verdict.

`COVENANT_COMPUTE_HARDWARE_WORKLOAD_SHA256` enables signed diagnostic routes alongside the coordinator journal. Absent configuration registers no hardware routes. POST `/federation/hardware/{job}/challenge` accepts `{"host_key_index":0}`; the assigned operator POSTs the returned challenge and raw guest/GPU evidence to `/evidence`; buyer/operator GET `/verdict`. Ed25519 headers bind method, path, exact body and a 30-second timestamp window. The buyer crate's `hardware::HardwareClient` implements these requests and independently re-verifies raw vendor evidence against its expected lease/workload, rather than trusting a coordinator label. `verified_known_hosts` only formats the verified key and callers must use strict host-key checking.

Challenge life is at most two minutes, verdict life five minutes, both bounded by the paid lease window. Raw evidence and verdict metadata are saved privately, atomically and with fsync; no private SSH keys or prompts belong in this journal. A changed workload pin requires a new journal namespace/review. Replaying evidence from another lease, nonce, generation, endpoint or key fails. A failed verification consumes the challenge; request a fresh challenge to retry. An overload rejection before verification does not consume it. Saved verdicts recheck vendor certificates/collateral at the current read time as well as recomputing the original verdict; the challenge collection window is not extended. HTTP evidence responses use `Cache-Control: no-store`.

The served trust ceiling is deliberately `Open`. Imported SNP and TDX genuine vectors demonstrate verifier behavior, not Covenant's deployment. A genuine NVIDIA CC chain reaches the measurement allowlist and is refused because its captured firmware differs from the reviewed reference. The legacy GPU fixture still lacks a production capture. Reference images and roots retain pinned upstream provenance in `crates/covenant-compute-attestation/UPSTREAM.md`. No Covenant hardware was purchased or accepted, and no confidential capacity is advertised. Stronger workspace floors remain unavailable, even when a diagnostic report has valid cryptography.

CPU-attested release requires a fresh Covenant-controlled guest image measurement, reproducible image/source provenance, per-lease hardware report collection with the returned challenge and an in-guest channel key, vendor collateral/TCB acceptance, and live negative replay/endpoint/key/workload-substitution checks. A later confidential GPU release additionally requires current GPU CC firmware allowlisting, measured in-guest local-device collection and guest-to-GPU secure-channel custody. Raising the compiled served ceiling is an explicit reviewed change after that acceptance; it is not an environment flag. This package supplies verification and signed transport; it does not claim to install a kernel attestation driver on rented machines.

The current Vast broker still emits legacy lease access (`ssh: None`), so these
routes cannot attest existing rentals. The additive typed metadata preserves
legacy serialized bytes when absent. Provider integration, a measured in-guest
collector and current image review remain explicit work in the
[acceptance plan](compute-hardware-acceptance.md). This implementation does not
activate the separate SSH attestor feature or confidential inference package.

## Local validation

Validated on 2026-10-20 against deployment baseline
`72f66142f407690024c6a0ca4a3bdaf148ef8766`, with Rust 1.98.1:

- 1,419 library tests passed across attestation, protocol, coordinator, buyer,
  control and node; 20 existing opt-in tests remained ignored.
- Another 76 attestation integration tests passed against vendor/synthetic
  vectors; four fixture regeneration or opt-in capture cases remained ignored.
- Cases include exact lease/key/workload substitution, replay, close/expiry,
  clock rollback, certificate expiry during a saved verdict, tampered vendor
  evidence, strict signed HTTP requests, bounded request/response bodies and
  overloaded blocking workers that do not touch the journal.
- Legacy lease access retains identical serialized bytes without the optional
  SSH metadata. Existing workspace controls and rental paths remain covered by
  the affected-crate suites.
- Workspace formatting and the crate inventory, Compute environment inventory
  and Compute network documentation guards passed. All-target Clippy passed for
  all six affected crates with warnings denied.

These are local software results. No genuine Covenant-bound CPU/GPU report,
provider lifecycle, stronger-floor workspace grant, paid settlement or production
deployment was accepted in this change.
