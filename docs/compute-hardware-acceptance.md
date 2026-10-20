# Measured CPU lease acceptance plan

Prepared 2026-10-20 for Prism adoption package 11. This is implementation
preparation, not a production activation or an acceptance result. No resources
were purchased, no provider account was created or logged into, and no paid
hardware test was performed for this investigation. Keep the served hardware
trust ceiling at `Open`.

The first target is one lease-bound, measured CPU guest. A successful CPU
acceptance would establish only the reviewed guest/session properties. It would
not attest a GPU, establish confidential GPU execution, prove arbitrary program
outputs correct, or complete package 14's confidential inference integration.

## Existing route and access

The deployed rental integration uses `covenant-compute-vast` through the node's
`broker.rs`. It launches a digest-pinned OCI image with the `ssh_direct` runtime,
attaches the buyer's SSH public key, and reconciles provider teardown. It does
not provision an SNP/TDX virtual machine or collect hardware reports.
[Vast's instance documentation](https://docs.vast.ai/guides/instances/overview)
describes its instances as Docker containers. A GPU model check, provider log,
container digest, or authenticated SSH connection cannot substitute for a
measured guest.

Historical broker configuration contains `COVENANT_VAST_API_KEY`,
`COVENANT_COMPUTE_BROKER_IMAGE`, `COVENANT_VAST_GPU_MODELS`, and
`COVENANT_VAST_MAX_HOURLY_MICROS`. Their presence is not a new authorization or a
fresh account/balance check.

The initial read-only investigation established no usable Phala/GCP/Azure/AWS API credential.
The standard local Phala, GCP, Azure and AWS configuration directories were
absent, and the inspected Compute release environment files supplied no new
provider connection. This was a scoped check, not an exhaustive credential
inventory. Phala documents `PHALA_CLOUD_API_KEY` for API access; no key value
belongs in this document or the repository.
[Phala authentication](https://docs.phala.com/phala-cloud/references/cloud-go-sdk/authentication)

## Candidate providers

| Candidate | Documented capability | Remaining incompatibility |
| --- | --- | --- |
| Phala `tdx.small`, 1 vCPU / 2 GB RAM | Intel TDX and dstack workload measurements; programmatic custom-data quotes | Signed-in account and production OS availability subsequently confirmed below; accepted guest profile, lifecycle adapter and SSH transport remain pending |
| GCP `c3-standard-4` in `us-central1-a` | Supported Intel TDX configuration; raw hardware challenge quotes available | Stock GCP boot measurements and event logs do not satisfy the current dstack-specific policy; no connected project, billing/quota confirmation, or adapter |
| GCP N2D with Milan and SEV-SNP in a supported zone | Raw AMD hardware reports available | Current imported SNP policy is Genoa-specific and pins a particular Kata guest plus workload `host_data`; a separate reviewed platform/image design is required |

The initial comparison used documented offerings, not reserved capacity. The
documented unauthenticated Phala instance-types request returned HTTP 403 during
research, so actual inventory remains unconfirmed. Do not infer availability
from the pricing page or substitute a non-confidential VM when capacity is
missing.

Primary sources:

- [Phala CPU instance pricing](https://phala.com/pricing)
- [Phala public instance-type API](https://docs.phala.com/api-reference/instance-types/list-family-instance-types)
- [GCP supported machines, technologies and zones](https://docs.cloud.google.com/confidential-computing/confidential-vm/docs/supported-configurations)
- [GCP raw SNP/TDX report collection](https://docs.cloud.google.com/confidential-computing/confidential-vm/docs/attestation)

Phala is the first candidate to qualify because its quote and measured
application model are closest to the existing verifier. This choice does not
establish compatibility or authorize a purchase.

## Production guest and channel requirements

The imported TDX reference describes `dstack-0.5.9-dev` with 1 vCPU and 2 GB RAM.
It records an OS archive hash and a tuple of MRTD plus RTMR0, RTMR1 and RTMR2.
That upstream reference is not an accepted Covenant guest. Never promote it
merely because a rented machine returns matching bytes.

A development OS image and the hardware TDX debug attribute are different
properties. The former includes software such as administrative SSH access;
the latter can allow the host to inspect TD state. Both need explicit policy
review. A hardware quote with debug disabled does not make a development
software image production-approved. Do not enable hardware debug or relax
measurement, collateral, TCB or advisory checks to obtain a passing result.

Phala's built-in guest SSH is available only on development OS images.
Production OS images disable it. Its documented SSH transport uses an OpenSSL
TLS `ProxyCommand`, and account-level SSH keys are copied into new guests.
Those facts prevent treating native `phala ssh` as an already compatible,
buyer-exclusive Covenant rental.
[Phala SSH access](https://docs.phala.com/phala-cloud/networking/enable-ssh-access)

A possible production route is a reviewed application container that owns the
lease's SSH service and collector inside a production dstack guest. This is a
design to qualify, not an established supported deployment recipe. Before
provisioning, demonstrate that the selected provider image and networking can
serve it without enabling the development guest shell. If they cannot, stop
and select a different measured guest route.

The acceptance profile must establish all of the following:

1. **Independent image provenance.** Pin the OS, collector, SSH service and
   workload images by digest. Retain source/build provenance and calculate
   expected boot measurements independently from the guest being tested.
   Record the CPU/memory/QEMU configuration used in the calculation. Approve
   complete measurement tuples, not independently interchangeable registers.
2. **Measured workload binding.** Review the exact dstack `app-compose` bytes
   and their hash. Replay the authenticated runtime event log and require the
   expected workload, instance, OS and key-provider bindings. Merely placing a
   claimed workload hash in `reportData` does not prove that workload ran.
3. **Guest-owned channel key.** Generate the SSH host private key within the
   measured guest/service for this lease. Do not inject a host key supplied by
   the broker, export it, or reuse it across leases. Review persistence,
   backup, logging, administrative access and any KMS dependency for paths
   through which another party could obtain it.
4. **Collector ownership checks.** The measured collector must obtain the
   public key from the actual SSH service and bind the current lease context.
   It must not act as a quote oracle for arbitrary caller-supplied channel
   keys, workload labels or unrelated endpoints. Restrict the dstack quote
   socket to the reviewed collector; measure and review other components
   that can request quotes or change the SSH service.
5. **Channel possession.** The buyer independently verifies the report, then
   performs a strict SSH handshake using only the attested host key. Retain
   proof that the endpoint answering that handshake owns the corresponding
   private key. This complements the measured key-generation/custody design;
   a handshake alone does not prove the key was never copied.
6. **Buyer access isolation.** Review all injected account, administrator and
   buyer authorization keys. Prove which parties can enter or alter the
   guest/service. Do not describe a session as buyer-exclusive while an
   unreviewed provider-account key still grants access.

Phala's current `DstackClient.getQuote()` accepts at most 64 bytes of
`reportData` without automatically hashing them. Supply the exact 64-byte
Covenant binding digest, not a hex string's UTF-8 bytes or a second digest.
Collect raw quote bytes, the event log, VM configuration, image provenance and
fresh Intel collateral. Verification happens outside the guest.
[Quote collection](https://docs.phala.com/phala-cloud/attestation/get-attestation),
[application verification](https://docs.phala.com/phala-cloud/attestation/verify-your-application),
[platform and OS verification](https://docs.phala.com/phala-cloud/attestation/verify-the-platform)

## Smallest viable Covenant integration

Implement a separately configured CPU lease provider path; do not repurpose a
Vast GPU offer or advertise this CPU guest as GPU capacity. Reuse Covenant's
lease ownership, duration, journal and settlement contracts where applicable,
while retaining the provider's real instance identity and billing terms.

The minimum adapter needs authenticated discovery/qualification, a fixed
create identity, create-outcome reconciliation, readiness, supported channel
transport, evidence collection, and confirmed deletion. It must reject
unexpected instance type, OS/image, workload, region, endpoint, account-key
policy or price changes before handing over access. A generic signed-report
upload endpoint by itself is not a functioning provider integration.

Keep TLS transport identity separate from the SSH host key terminating inside
the guest. If TLS tunneling is required, define and validate the transport
explicitly instead of accepting arbitrary shell commands or weakening strict
SSH checking. Bind the final externally reachable endpoint and the actual
guest key into the Covenant challenge.

Before enabling protected workspace grants, integrate the adapter with a live
Covenant lease and exercise independent buyer verification. An out-of-band VM
probe can qualify a collector or verifier, but cannot by itself prove the
lease/workspace integration.

## Bounded acceptance sequence

1. Resolve a connected provider account, current inventory, exact production
   image availability, permissions, billing terms and the reviewed guest/
   channel design. Prepare an immutable plan with separate provider and native
   Compute cost limits. There is no approved provider spending record here.
2. Before creating a resource, persist the acceptance run ID, Covenant job ID,
   buyer/operator identities, lease generation, provider create label or
   idempotency identity, workload/image pins, maximum lifetime and teardown
   owner. Reserve the authorized budget without resetting existing ledgers.
3. Create at most the planned guest. On an ambiguous response, reconcile the
   original identity; do not generate a replacement label, new job or second
   purchase. Preserve unknown costs and cleanup obligations.
4. Obtain a fresh coordinator challenge after the real lease endpoint and
   guest key are known. Collect a report bound to the full lease scope and
   exact nonce. Verify vendor signatures, current collateral/TCB policy,
   measured image/workload, key/endpoint binding, and the paid lease window.
   Retain both raw evidence and independent verifier results.
5. Demonstrate the attested SSH handshake and a synthetic workspace grant/
   transfer under the reviewed trust policy. No customer secrets are needed.
   Do not lift the compiled production trust ceiling to run a diagnostic.
   A positive stronger-floor test needs an explicitly reviewed isolated
   acceptance build/profile, followed by a separate release decision.
6. Exercise the negative cases below and restart/recovery behavior. Re-read
   current lease ownership, endpoint, generation and expiry before a grant;
   a stored earlier verdict must not override changed live state.
7. Close the original lease, reconcile its original settlement identities,
   delete the original provider instance and disks, and confirm absence
   through authenticated inventory and billing evidence. A stop request,
   timeout or missing SSH response is not deletion proof. Retain unresolved
   obligations until reconciled.

Required rejection cases include evidence from another lease/buyer/operator or
generation; consumed or expired nonce; changed endpoint or host key; a valid
quote paired with a different SSH server; changed workload/compose hash;
unknown image measurements; altered or missing runtime events; expired or
unacceptable vendor collateral/TCB; closed/expired leases; and a restart with
an already-consumed challenge. A failed verification consumes its challenge.
A fresh challenge may retry evidence collection within the original plan,
without buying a replacement lease. Replaying an accepted response must not
create another resource or payment.

## Cost envelope and unresolved blockers

Phala advertises `tdx.small` compute at **$0.06/hour** and disk at
**$0.000139/GB-hour**. A one-hour planning window with a 20 GB disk therefore
estimates **$0.06278**, approximately **$0.063**, for those two components.
This is not an all-in quote or an approved spend. Ancillary charges, taxes,
payment minimums, account-specific terms and available inventory remain
unconfirmed. Do not count advertised trial credits as an available balance.
[Pricing and storage lifecycle](https://phala.com/pricing)

The planned maximum runtime must include boot and teardown, not only the
successful probe. Confirm billing granularity and arrange independent expiry
cleanup before creation. Phala storage continues billing while a stopped CVM
exists, so successful deletion is required. A timer or client-side budget
cannot guarantee the provider's bill is capped if deletion fails.

The current blockers are provider credentials/access and inventory, a reviewed
production guest with independently derived measurements, proven collector/
SSH key custody and transport, a durable CPU provider lifecycle adapter, an
approved all-in cost envelope, and real lease/workspace/teardown acceptance.
Local verifier tests and the diagnostic port resolve none of these external
acceptance gates by themselves.

Only after evidence review may a separate code/release change raise the served
ceiling for the specifically accepted profile. Keep other operators, GPU
claims and unaccepted guest profiles at their existing trust level.

## Signed-in account qualification (2026-10-20)

The user confirmed an existing Phala account and requested a new Covenant
Compute node. Its authenticated deployment form offers production
`dstack-0.6.1` on `prod5` in US West. The selected preview is a small TDX
instance with 1 vCPU, 2 GB RAM, 20 GB storage, Phala KMS and ZFS. It quotes
**$0.06074/hour**, or **$44.34/month** if kept running. This account preview
supersedes the earlier public estimate for this configuration; it is not a
resource reservation or final billing evidence.

The user created `covenant-compute-attestation`; its boot completed and the
instance gateway served the default Phala Next.js starter. The existing CVM is
`32ed28e4-0a6f-4dff-a9bf-ba84c38d6e9f`, app
`d5f0ef708c68c52916a7b0da313ae416eb0b980d`, instance
`fc551bcb5f96283763f42da3578805ec7e2ecfe3` on
`dstack-pha-prod5.phala.network`. Reuse this identity; do not create a duplicate.
The starter is unsuitable for lease/key binding and still needs the reviewed
Covenant service image. Running the starter is not hardware acceptance or
registered Covenant Compute capacity.

The supplied clipboard credential was saved privately as requested,
but the management API rejected it with HTTP 401; a Cloud management API token
is still required for the API lifecycle path. The signed-in browser provides
configuration visibility without that token.

The available production OS also exposes an implementation gap: dstack 0.6.1
uses V2 event/VM-configuration bindings. The imported verifier currently pins a
historical V1 profile. Production image measurements and V2 log verification
must be independently qualified, with strict vendor TCB/advisory policy retained,
before any stronger trust can be served.

The starter's public `Info` response reports production OS hash
`ec43cb8c5e04b6bb6c53b33b16076a01d83d01f7f4b703f2f78a0d947d6247e6`
and a lite TDX measurement document. Its embedded checksum manifest and
measurement CBOR were compared byte-for-byte with the official
`mkosi-os-v0.6.1` archive, whose SHA-256 is
`7583f51c717011b8086c2dd4d1e73fbbe7bf2d6d2420b90858d538c26efcca41`.
The manifest hash matches the pinned OS hash. This qualifies the source
material, not the running quote: independent launch replay and signature/TCB
verification are still required. The actual boot reports ext4 storage,
superseding the earlier deployment form's ZFS preview.
