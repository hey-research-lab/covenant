# KMS policy review

`kms_policy.py` is an optional, read-only operator tool. It is not installed in
the guest image and does not change attestation admission or the Open ceiling.
It requires Python with `cryptography>=42`.

Supply an owner-only JSON plan with exactly these fields:

```json
{
  "schema_version": 1,
  "app_id": "<40 lowercase hex characters>",
  "workload_sha256": "<64 lowercase hex characters>",
  "expected_ca_spki_hex": "<182 lowercase hex characters>"
}
```

The CA pin must come from separately reviewed app-compose bytes and the
authenticated `key-provider` runtime event. Do not derive an expected pin from
the endpoint being checked. The app/workload fields identify the review;
`GetMeta` does not prove authorization of either value.

```sh
python3 deploy/compute-attested-cpu/kms_policy.py \
  --plan /private/path/kms-review-plan.json \
  --output /private/path/kms-meta.json
```

The output directory must be owner-only; both output files must be new. The
tool authenticates the fixed prod5 KMS endpoint against the supplied CA public
key, exact DNS name, certificate validity, signing usage, and `kms:rpc` role
before sending `GET /prpc/KMS.GetMeta`. No client credential, key-release RPC,
redirect, retry, signing, account mutation, or quote request is used. TLS uses
explicit pinned-key verification rather than public-Web-PKI trust. The embedded
certificate attestation is retained but is **not** marked hardware-verified.

The raw metadata and a separate review record include body hashes, the peer
certificate, times, authentication result, and policy gaps. A successful
collection exits **2** because it does not qualify production policy. Failure
also preserves captured evidence. Positive `is_dev=false` or
`allow_any_upgrade=false` fields do not establish an immutable authorization
policy. Missing OS-image verification is never interpreted as enabled.

## Required decision before production qualification

For the reviewed Phala managed KMS, the direct authenticated response reported
`os_image_verification=false`, `chain_id=0`, and no contract address. The official
public KMS registry's CA key matched the application's measured provider key.
This identifies the managed KMS; it does not publish its complete workload
allowlist, authorized account administrators, or future policy changes.

Resolve that gate through one of these explicitly reviewed paths:

1. **Customer-controlled policy:** obtain authenticated KMS deployment/config
   provenance, current attestation and enabled OS-image verification; bind its
   authorization backend to the actual chain and contracts. Review app and KMS
   owners, implementation/code, approved Compose hashes, device/TCB policy and
   all mutation powers at one finalized block. Decide acceptable governance
   and refresh requirements. `disableUpgrades` blocks implementation upgrades
   only; it does not disable owner-operated Compose/device/TCB setters.
2. **Bounded managed-policy acceptance:** an authorized Covenant operator must
   explicitly accept Phala's centralized, mutable policy and disabled KMS
   image check for the named CA pin, app, exact workload, device/lease scope
   and finite expiry. Retain authenticated account evidence of current
   deployment/revision and device policy, responsible administrators, and the
   decision. Covenant must independently verify the full production launch
   tuple, Intel TCB/advisories, fresh challenge and signed per-lease workload
   pins. Limit exposure to the approved lease; do not claim immutable policy
   or provider-independent persistent-secret exclusivity.

This helper cannot make that acceptance decision and has no override that
turns collected metadata into a passing policy verdict.

Read-only account routes include `/api/v1/apps/{app_id}/kms-info`,
`/device-allowlist`, `/revisions`, and `/pending-updates`. The signed-in Phala
app overview/instance settings can support that review. Account UI evidence
does not turn centralized governance into independently immutable policy.

The authorized account read on 2026-10-20 returned HTTP 200 for all four
routes for the diagnostic app
`d5f0ef708c68c52916a7b0da313ae416eb0b980d`. It confirmed
`is_onchain_kms=false`, with no chain, KMS/app contract, deployer, or Safe
addresses. Device policy returned `allow_any_device=null` and `devices=[]`;
these represent unavailable on-chain policy information, not a verified
deny-all or allow-all rule. Revision history contained the starter Compose
hash `e611067a2f2dc3b5c0c2bedfccc0b3f4567f0044c22a30213868b936db1c8d61`
and the reviewed collector hash
`c4428b75f358f9f3295dea7fa73c59ab590344ee42717e7080c6c6167ea5ac97`.
The pending-update list was empty at collection time. Raw account responses
and hashes are retained privately; no account credential is part of the
source or evidence output. These reads resolve the account-access gap, but
do not publish the managed KMS's full authorization rules or administrator
powers, and do not alter the disabled-OS-verification finding above.

## Source basis

- [Phala Cloud versus on-chain KMS](https://docs.phala.com/phala-cloud/key-management/cloud-vs-onchain-kms)
  documents the managed policy authority.
- Pinned dstack `7f143148cc49f4a09799958835d310e050137cb3`:
  `dstack/cert-client/src/lib.rs:34–46` requires `kms:rpc`, because the root also
  anchors application certificates; `dstack/dstack-util/src/system_setup.rs:2412–2424`
  compares the KMS key against the app-compose pin.
- `dstack/kms/src/main_service.rs:387–475` gates release through authorization
  and derives keys from app/instance identity rather than Compose hash;
  `:534–562` emits metadata. `DstackApp.sol:136–223` distinguishes implementation
  upgrades from mutable policy setters.

Run `python3 -m unittest discover -s deploy/compute-attested-cpu -p
'test_kms_policy.py' -v` with `cryptography>=42` to exercise certificate and
transport rejection tests. The normal dependency-free suite skips these
optional crypto tests when that package is absent.
