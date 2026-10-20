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

Resolve the policy choice through an explicitly reviewed path. The second
path permits only a bounded synthetic pilot, not stronger production trust:

1. **Customer-controlled policy:** obtain authenticated KMS deployment/config
   provenance, current attestation and enabled OS-image verification; bind its
   authorization backend to the actual chain and contracts. Review app and KMS
   owners, implementation/code, approved Compose hashes, device/TCB policy and
   all mutation powers at one finalized block. Decide acceptable governance
   and refresh requirements. `disableUpgrades` blocks implementation upgrades
   only; it does not disable owner-operated Compose/device/TCB setters.
2. **Bounded managed-policy acceptance for a synthetic pilot at `Open`:**
   an authorized Covenant operator must
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

## OS verification and the limits of on-chain app ownership

The pinned dstack source explicitly warns that when `image.verify=false`,
`os_image_hash` is caller-supplied and the `allowedOsImages` check does not
enforce the booted OS identity. The KMS returns without verifying the image
when this flag is false, even after a successful authorization decision.
Selecting an on-chain app contract does not enable that independent server
check. See the [configuration warning](https://github.com/Dstack-TEE/dstack/blob/7f143148cc49f4a09799958835d310e050137cb3/dstack/kms/kms.toml#L80-L86)
and [release-path check](https://github.com/Dstack-TEE/dstack/blob/7f143148cc49f4a09799958835d310e050137cb3/dstack/kms/src/main_service.rs#L369-L410).

Customer ownership of `DstackApp` controls its Compose/device allowlists and
optional `UpToDate` requirement. The standard app policy does not check an
exact OS measurement or advisory IDs. The separate `DstackKms` owner controls
the global OS allowlist, permitted KMS measurements/devices, factory
implementation and shared-contract upgrades. App ownership does not transfer
control of the KMS server or root keys. A custom app policy can inspect
additional measurement/advisory fields, but its implementation, authenticated
inputs and enforcement require their own review.
Sources: [app policy](https://github.com/Dstack-TEE/dstack/blob/7f143148cc49f4a09799958835d310e050137cb3/dstack/kms/auth-eth/contracts/DstackApp.sol#L190-L216),
[KMS authority](https://github.com/Dstack-TEE/dstack/blob/7f143148cc49f4a09799958835d310e050137cb3/dstack/kms/auth-eth/contracts/DstackKms.sol#L102-L305)
and [policy inputs](https://github.com/Dstack-TEE/dstack/blob/7f143148cc49f4a09799958835d310e050137cb3/dstack/kms/auth-eth/contracts/IAppAuth.sol#L36-L45).

These are properties of the reviewed source, not proof that the observed
managed KMS runs those exact bytes. Matching version metadata and the expected
CA authenticate the reported identity; they do not establish measured code or
configuration provenance. A different KMS authority requires fresh CA,
configuration, governance and boot qualification; previously reviewed OS and
container bytes may be reusable, but existing prod5 acceptance cannot simply
be transferred to the new authority.

The KMS choice remains pending and new creation remains disabled. Any later
managed-policy acceptance must stay a finite synthetic pilot at `Open`, with
independent Covenant evidence checks. It is insufficient for stronger
production trust or provider-independent persistent-secret exclusivity.

## Prism comparison at a pinned revision

Prism `main` was checked at `4e32eb5eb1398fd9d11dcd29aa5a96e3f2f646ec`.
Its inference verifier explicitly skips `key-custody`: no implemented verifier
appraises the dstack KMS custody chain. That documented skip is allowed in an
overall `verified` verdict; the label therefore does not establish exclusive
private-key custody. See the [skip and verdict rule](https://github.com/prismnetwork-tech/prism/blob/4e32eb5eb1398fd9d11dcd29aa5a96e3f2f646ec/sdk/attest.mjs#L578-L601).

Prism's [confidential inference path](https://github.com/prismnetwork-tech/prism/blob/4e32eb5eb1398fd9d11dcd29aa5a96e3f2f646ec/inference/README.md#L205-L212)
relays to Phala-hosted inference; this is distinct from provisioning and
closing a renter's attested node lease. Its [node lease code](https://github.com/prismnetwork-tech/prism/blob/4e32eb5eb1398fd9d11dcd29aa5a96e3f2f646ec/node/prismd/src/attestation.rs#L184-L245)
binds a quote to a challenge, lease, node and guest SSH key. Code presence alone
does not demonstrate an accepted deployed lifecycle. The actual
[verification ceiling is `Confidential`](https://github.com/prismnetwork-tech/prism/blob/4e32eb5eb1398fd9d11dcd29aa5a96e3f2f646ec/crates/protocol/src/lib.rs#L145-L167);
older prose describing a lower ceiling is not an activation record. An empty
public rental-offer list likewise says nothing about the separate inference
service's availability.

A repository-wide source search at that revision found no guest KMS contract
or server-policy implementation under `DstackApp`, `KmsAuth`, `kms_contract`,
`os_image_verification` or `image.verify`. This scoped finding does not identify
the upstream service's actual KMS policy. Prism's [AWS KMS signer](https://github.com/prismnetwork-tech/prism/blob/4e32eb5eb1398fd9d11dcd29aa5a96e3f2f646ec/crates/chain/src/lib.rs#L731-L773)
signs chain transactions; it does not establish dstack guest key-release policy.

Keep Covenant's KMS qualification separate from launch, channel and receipt
verification. End-to-end encryption reduces relay plaintext exposure but does
not prove exclusive private-key custody. Prism supplies no evidence that
resolves the observed prod5 OS-verification gap or justifies raising `Open`.

Prism's [SNP guest bootstrap](https://github.com/prismnetwork-tech/prism/blob/4e32eb5eb1398fd9d11dcd29aa5a96e3f2f646ec/node/prismd/assets/workspace-bootstrap.sh#L45-L121)
generates the SSH host key in guest runtime storage and binds its public key
to the lease challenge. Its [workspace client](https://github.com/prismnetwork-tech/prism/blob/4e32eb5eb1398fd9d11dcd29aa5a96e3f2f646ec/sdk/workspace.mjs#L1-L40)
keeps snapshot encryption keys with the buyer. These are useful limits on
provider-held persistent secrets, not independent proof of guest key custody.
Its SNP [runtime configuration](https://github.com/prismnetwork-tech/prism/blob/4e32eb5eb1398fd9d11dcd29aa5a96e3f2f646ec/node/prismd/src/runtime.rs#L464-L489)
also puts `/workspace` in tmpfs, reducing restored plaintext written to disk.

Covenant likewise generates the SSH private key in `/run` tmpfs and keeps
workspace encryption keys local to the buyer. Its current Compose still uses
disk-backed named volumes for restored workspace plaintext, the public host-key
sidecar and collector state. KMS compromise combined with disk access could
therefore affect their confidentiality or integrity. The collector's read-only
mount does not establish integrity against an actor controlling the underlying
storage. A valid fresh quote does not independently authenticate every mutable
file it reads.

Tmpfs alone also does not establish that key pages never reach persistent
storage: the reviewed dstack source supports encrypted-disk swap, while this
Compose does not establish a no-swap policy. Actual VM configuration and runtime
swap behavior require qualification before claiming memory-only custody.
These are residual trust boundaries, not evidence of an observed compromise.
See dstack's [persistent container storage](https://github.com/Dstack-TEE/dstack/blob/7f143148cc49f4a09799958835d310e050137cb3/os/common/rootfs/dstack-prepare.sh#L286-L303)
and [swap setup](https://github.com/Dstack-TEE/dstack/blob/7f143148cc49f4a09799958835d310e050137cb3/dstack/dstack-util/src/system_setup.rs#L2499-L2573).
