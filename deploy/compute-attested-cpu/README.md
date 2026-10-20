# Production-guest collector candidate

This directory supplies the guest image for a bounded CPU lease on production
dstack 0.6.1. The memory-hardening candidate requires a new immutable image and
runtime acceptance; previous image evidence does not qualify these new bytes.
It does not activate hardware trust or resolve managed-KMS policy. The served
ceiling remains `Open`. The external lifecycle is described in
[the lease integration guide](../../docs/compute-phala-lease-lifecycle.md).

## Image and ownership

Build one Linux amd64 image from the repository root:

```sh
docker build --platform linux/amd64 \
  -f deploy/compute-attested-cpu/Dockerfile -t covenant-attested-cpu:local .
python3 deploy/compute-attested-cpu/runtime_smoke.py --image covenant-attested-cpu:local
```

The base is the official Debian trixie-slim amd64 manifest
`sha256:918311b7b6c4c6f68b232ba516584925f6c78ad82b6fd534b98979df6438e483`.
Debian and Debian Security packages come from the immutable
`20261020T000000Z` snapshots. HTTP is used to bootstrap the minimal base without
CA certificates; apt still requires Debian archive signatures and verifies
package hashes. Only the expired-metadata time check is disabled for these
fixed historical snapshots. `/opt/covenant/packages.tsv` records every installed
package/version. Retain that file, the final OCI digest, source commit, build
provenance and security review before qualifying the image.

The same image runs two roles:

- `ssh`: an ordinary, unprivileged-mode container whose OpenSSH master starts
  as container root for privilege separation. Only `CHOWN`, `SETUID`, `SETGID`
  and `SYS_CHROOT` remain. Buyer sessions run as uid10001. The host private key
  is freshly generated in tmpfs inside this container, is root-readable only,
  and is never passed into the collector or provider configuration.
- `collector`: uid10002, no capabilities, read-only root filesystem. It has the
  dstack quote socket, a read-only memory volume containing the SSH public key
  and short-lived runtime status, and its own persistent consumption journal.
  The buyer SSH container has
  neither this socket nor this journal nor the collector authorization secret.

The sole SSH authorization key comes from the public lease policy. Password,
root, agent forwarding, port forwarding, user RC, PTY and tunnel access are
disabled. The buyer can execute commands and transfer synthetic workspace files;
the root filesystem and root-owned key authorization remain immutable to it.
No host root, Docker socket, privileged mode or guest native administrative
SSH is used. A container restart generates a new host key; evidence for the old
key must fail. Container lifetime checks stop access at lease expiry, but do
not delete a billed CVM or replace external cleanup.

## Memory and swap policy

The new measured image enforces this policy using actual Linux interfaces.
Unsupported or unreadable controls stop startup or collection; there is no
compatibility fallback or request flag that disables checks.

| Storage or control | Required runtime state |
| --- | --- |
| SSH `/run` and `/tmp` | Whole tmpfs mounts, `noswap,nosuid,nodev,noexec`, 8 MiB and 16 MiB |
| Buyer `/workspace` | Whole tmpfs mount, `noswap,nosuid,nodev`, 128 MiB, uid/gid10001 |
| Shared `/host-public` | Whole tmpfs mount, `noswap,nosuid,nodev,noexec`, 1 MiB; collector reads only |
| Both container cgroups | Private cgroup v2; exact 768/256 MiB memory limits; `memory.swap.max=0` and `memory.swap.current=0` |
| Guest swap | `/proc/swaps` has a valid header and no configured swap devices/files |
| Core dumps | Both soft/hard `RLIMIT_CORE` limits zero and a nonempty, non-piped guest `core_pattern` |

Each memory filesystem uses a separate local-driver tmpfs volume whose name
includes the job UUID. `volume.nocopy` prevents Docker copying image directory
contents into it. This passes `noswap` to the Linux mount operation; Docker's
service `tmpfs`/`--tmpfs` option parser rejects that flag before reaching Linux.
The public volume prefix is `host-public-memory-v1`, so updating an older
deployment cannot silently reuse its disk-backed `host-public` volume. Actual
mount verification is still required. Runtime and workspace volumes are mounted
only by the SSH role, and lose contents when that role stops and their last
mount is released. A restart generates a new private host key. The 128 MiB
workspace accommodates the buyer's maximum 64 MiB snapshot transfer; it does
not promise space for multiple maximum snapshots.

The SSH role checks storage before generating its private key, then every
250 ms while serving. A failure removes its public status and terminates the
role and its sessions. Status is an atomically replaced, root-owned regular
file with mode0644, bound to the entire lease policy and current public key.
The collector requires this status to be at most two seconds old using
`CLOCK_BOOTTIME`, checks its own kernel controls, and repeats these checks
before and after guest RPCs. The signed challenge and quote wire schemas are
unchanged: this policy is an invariant of the independently pinned image and
measured workload, not an extra unsigned attestation claim. Retain raw runtime
observations separately during acceptance.

The reviewed dstack source pins Linux 6.18.40 and supports configurable swap;
the reviewed app-compose wrapper must explicitly set `swap_size` to `"0"`.
That configuration alone is insufficient, which is why the runtime must also
pass the checks above. Kernel documentation specifies that tmpfs can otherwise
swap pages and that `noswap` disables it. Docker requires memory and
memory-swap to be equal to prohibit container swap.
Sources: [pinned kernel version](https://github.com/Dstack-TEE/dstack/blob/7f143148cc49f4a09799958835d310e050137cb3/os/mkosi/versions.env),
[tmpfs](https://docs.kernel.org/filesystems/tmpfs.html),
[Docker memory limits](https://docs.docker.com/engine/containers/resource_constraints/#--memory-swap-details).

These controls reduce accidental disk persistence. They do not prove safety
against a compromised guest kernel/root, malicious KMS or mutable container
runtime state. The collector journal remains disk-backed to preserve replay
consumption across restarts; Docker state and application configuration also
remain subject to the separate KMS review. A buyer command can deliberately
send plaintext to output or another destination. Local workspace encryption
does not make an untrusted execution environment safe to receive plaintext.

The Docker smoke checks actual mounts/cgroups, transfers a synthetic 64 MiB file
over SSH, verifies the capacity limit, confirms restart erasure/key rotation,
and changes a real container swap allowance to require watchdog termination.
It also tests rejected unsafe startup configurations, authentication, replay,
SFTP and expiry. Its fake dstack responses do not constitute TEE acceptance.
The disposable CI host explicitly disables swap and configures a non-piped
core handler; no test helper changes these settings on a live Phala guest.

## Candidate inputs

Create a private local policy file with exactly these fields. Replace every
placeholder with the original persisted plan's actual public identities; do
not create replacement jobs or reset the lease's accounting state.

```json
{
  "schema_version": 1,
  "job_id": "<existing full nonzero UUID>",
  "operator_pubkey_b58": "<32-byte public identity>",
  "buyer_pubkey_b58": "<32-byte public identity>",
  "lease_generation": "<original provider generation>",
  "lease_expires_at_ms": 0,
  "gateway_domain": "dstack-pha-prod5.phala.network",
  "buyer_ssh_public_key": "ssh-ed25519 <public key only>",
  "max_challenges": 32
}
```

The renderer rejects missing/extra fields, mutable image tags, expired plans
and acceptance windows exceeding one hour. It never overwrites an existing
candidate. This is deliberately a bounded acceptance profile, not an
indefinitely running public rental.

```sh
python3 deploy/compute-attested-cpu/render.py \
  --image-digest sha256:<accepted-OCI-digest> \
  --policy /private/path/lease-policy.json \
  --output /private/path/compose.candidate.yaml
```

Inject a **new independent 32-byte secret encoded as 64 lowercase hexadecimal
characters**, named `COVENANT_COLLECTOR_TOKEN`, through Phala's encrypted
environment facility. It must not be a Phala API key, buyer private key or
operator signing key. Do not put its value in Compose, logs, retained public
evidence or the SSH container. The template references the encrypted variable
only in the collector service. The token authorizes collection; it does not
grant trust or make returned evidence authentic.

Retain the provider's exact serialized `app-compose` bytes and hash. The
renderer prints the hash of Docker Compose YAML only; **that is not the
dstack measured app-compose hash**. Independently verify the measured wrapper
including all settings, images, environment policy and any startup script.
Secret rotation and application update behavior need review with Phala KMS.

## Collection and channel binding

The collector publishes guest host port 18090 to container port 8090
(`18090:8090`). Its instance-pinned HTTPS endpoint is
`https://<Info.instance_id>-18090.<gateway_domain>`.
dstack 0.6.1 already binds guest host `0.0.0.0:8090` for its own
[guest agent](https://github.com/Dstack-TEE/dstack/blob/7f143148cc49f4a09799958835d310e050137cb3/dstack/guest-agent/dstack.toml#L28-L30).
Publishing or requesting collector traffic on host 8090 conflicts with that
service. The collector image continues to listen on internal port 8090.

`GET /identity` and `POST /quote` require `Authorization: Bearer <token>`.
The POST body is the complete coordinator `HardwareChallenge` object.
The collector checks:

1. The original job, buyer, operator, generation and lease expiration.
2. The public key generated by the measured SSH service.
3. An instance-pinned endpoint constructed as
   `<Info.instance_id>-1022.<gateway_domain>`, external port443, user `covenant`.
4. `scope.workload_sha256 == Info.compose_hash`, and the expected production
   OS image hash. These Info comparisons are consistency checks; independent
   attestation verification must authenticate every identity and measurement.
5. Strict canonical ASCII/safe-integer encoding, two-minute challenge window,
   current lease window, and durable one-use challenge ID and nonce.
6. Current kernel storage/swap controls and a fresh, key/lease-bound status
   from the SSH watchdog, repeated after potentially blocking guest calls.

It consumes a valid challenge before requesting a quote; failure leaves it
consumed. The persistent journal rejects a different job, clock rollback,
replay after process restart and exceeding the fixed challenge budget.
It submits the exact 64-byte SHA-512 Covenant guest-domain binding as lowercase
hex to frozen `/GetQuote`; no extra SDK domain hash is applied. It rechecks
identity, key and expiry after collection. Responses are always labelled
`unverified`, preserve `quote`, `event_log` and `vm_config`, and never grant
workspace access.

Production app-level SSH uses Phala's standard TCP-over-TLS gateway. First
confirm the actual instance-pinned URL and gateway domain. The client must
validate the outer TLS hostname/chain and strictly pin the independently
attested SSH host key inside that connection. A controlled local tunnel can
provide this transport; arbitrary provider-supplied `ProxyCommand` strings
are not a supported Covenant interface. Existing typed SSH metadata does not
accept a caller-selected TLS hostname: the buyer's native transport validates
the exact advertised endpoint and then the independently verified SSH key.

## dstack 0.6.1 qualification remains separate

The known production release is
[`mkosi-os-v0.6.1`](https://github.com/Dstack-TEE/dstack/releases/tag/mkosi-os-v0.6.1),
source `7f143148cc49f4a09799958835d310e050137cb3`, with
`os_image_hash=ec43cb8c5e04b6bb6c53b33b16076a01d83d01f7f4b703f2f78a0d947d6247e6`.
The OS archive SHA-256 is
`7583f51c717011b8086c2dd4d1e73fbbe7bf2d6d2420b90858d538c26efcca41`.

Production0.6.1 uses V2 event preimages and measured VM configuration. Keeping
`/GetQuote` compatibility does **not** qualify its event or measurement
policies. The Covenant V2 integration and exact production profile require
independent review and live evidence before acceptance:

- Independently derive the complete expected boot measurement tuple for the
  exact production image and VM configuration; authenticate the image source,
  artifact and expected production metadata.
- Qualify V2 event replay and authenticated application, instance, OS,
  key-provider and system-ready boundaries. Preserve numeric versions and hex
  preimages; do not translate V2 logs into invented V1 events.
- Verify quote signatures and current Intel collateral. Enforce `UpToDate`,
  empty advisories and debug disabled; a generic upstream `is_valid` result is
  insufficient. Unknown/null development-image metadata is not production.
- Review Phala KMS, device authorization, replication, disk encryption,
  management/admin paths and possible host-key export paths. Prove the actual
  SSH handshake, then run endpoint/key/workload/replay substitution negatives.
- Connect the accepted evidence to a real Covenant lease/workspace and retain
  original resource identity, billing, deletion and settlement evidence.

The collector uses no GPU API and cannot establish GPU confidentiality.
Do not loosen measurement/TCB policy or use the development OS to make it pass.

## Local validation

```sh
python3 -m unittest discover -s deploy/compute-attested-cpu -p 'test_*.py' -v
```

Unit tests exercise substitution, exact raw report-data submission, V2 byte
preservation, malformed canonical input, expiry, durable replay/rollback and
Unix-socket RPC. `runtime_smoke.py` separately requires Docker and performs
real SSH public-key login, SFTP, wrong buyer/host-key rejection, key/socket/
secret isolation, journal restart and a separate short lease's shutdown with
a **fake** guest socket. Neither
suite proves TDX, a Phala deployment, current collateral or billable acceptance.
Container and live runtime results must be retained separately.

Primary API/network references:

- [Pinned dstack0.6.1 guest RPC](https://github.com/Dstack-TEE/dstack/blob/7f143148cc49f4a09799958835d310e050137cb3/sdk/curl/api.md)
- [Phala TCP gateway](https://docs.phala.com/phala-cloud/networking/expose-tcp-service)
- [Native guest SSH restrictions](https://docs.phala.com/phala-cloud/networking/enable-ssh-access)
- [Encrypted environment variables](https://docs.phala.com/phala-cloud/cvm/set-secure-environment-variables)
- [Pinned instance-specific gateway selection](https://github.com/Dstack-TEE/dstack/blob/7f143148cc49f4a09799958835d310e050137cb3/dstack/gateway/src/main_service.rs#L2541-L2556)
- [Pinned guest socket permissions](https://github.com/Dstack-TEE/dstack/blob/7f143148cc49f4a09799958835d310e050137cb3/os/common/rootfs/dstack-guest-agent.socket#L11-L15)
