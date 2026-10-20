# Native Phala no-KMS acceptance — 2026-10-20 UTC

**The coordinator integration is deployed, but paid production qualification
failed.** The original buyer passed independent TDX admission, uploaded a real
client file through the signed relay, and closed the lease. Phala deletion
outlasted the node's 60-second successful-receipt deadline. Covenant returned a
verified execution failure, restored all buyer credit, and continued durable
cleanup until the original VM was confirmed absent. No replacement lease was
submitted.

This is separate from the earlier [isolated no-KMS acceptance](compute-ephemeral-phala-acceptance.md).
The served trust ceiling remains `Open`; neither managed-KMS qualification nor
persistent-secret custody is claimed.

## Candidate and deployment

| Item | Verified value |
| --- | --- |
| Coordinator revision | `da00ccf7541d94cfb97c2a43dd9dd96f6719f499` |
| Coordinator image | `ghcr.io/open-covenant/covenant/compute-coordinator-public@sha256:795fdd1dce0536b5a6526b1ad6fde732f14541751cb54486a4dcfb4054679140` |
| Node image qualified | `ghcr.io/open-covenant/covenant/compute-node-public@sha256:1f7398191a6d0457ec42a489ea048f7f65a54c50fdffb38a78d070b94b373d25` |
| Local buyer/node implementation | `ee37d807aa4b2327a7b813ff44e35f7e8acec548`, immutable locally built binaries |
| Guest image | `sha256:925122b9c68fe8f385784b3d926685f6e08f59467c5a76fbfe71faa701d81932` |
| Model/profile | `phala-cpu-ephemeral-dstack061` / `phala_ephemeral_dstack061` |
| Render deployment | `dep-dbbusarl9sls739bdq4g`, live at 22:42:18 UTC |

The difference from `ee37d807` to `da00ccf7` fixes a test-clock race and adds a
development-only Tokio feature; production implementation is unchanged. The
exact deployed revision passed push/PR CI, release builds and image builds.
Every coordinator and node image layer was downloaded anonymously and checked
against its content digest. Local and CI validation covers the independent
attestation, native buyer, relay, Phala lifecycle and settlement paths.

Only hardware lease pins, the ephemeral relay, and its public origin were added
to production configuration. All 82 existing environment values, the existing
disk and service configuration were preserved. The native journal retained all
119 pre-test jobs and the same idle financial counters. Startup compacts that
journal, so this does not claim byte equality or a captured comparison of every
historical job field. The older unresolved canary obligation and its reservation
were preserved; this acceptance did not resolve that separate obligation.

Relay and hardware state lock files were confirmed under the persistent `/data`
mount in the running service. Unsigned relay requests returned 401. These
checks establish initial persistence configuration, not a completed relay
restart acceptance.

## Original identities and bounded spend

| Item | Value |
| --- | --- |
| Job | `23b95fa0-d813-4b6d-805b-970ddf6d05d7` |
| Buyer | `7dd9Lzn4K1MokMqKyn3C4cKjVzQ2YZffVSFthasZ7xFZ` |
| Operator | `6iB1M8xKhMWHxRrZjy2NYqop9iCQ74ARP3H1zdmZKY21` |
| Fresh provider VM | `7fe4c48b-7757-4102-9d90-62e0de8639d1` |
| Lease maximum | 900 seconds at 34 micro-USDC/second; 30,600 micro-USDC total |
| Provider allowance | One new VM; $0.05 planning reservation, not an invoice cap |

The authorized 1,000,000-CVNT stake finalized before admission. Its transaction
is [kQ1QpfVL…](https://explorer.solana.com/tx/kQ1QpfVLbsnHTLmUDCKacpcmzJNmgQC5XAqNa51gD9LS1vprvNve5PKMgKaocYHBtuhFqWcGTqym3NRDvewtezw),
with lock expiry 2026-10-27 22:48:30 UTC. The one-use provider allowance pins
this exact buyer and job. Deletion does not replenish that allowance. The
pre-existing diagnostic VM was not adopted or deleted.
After the failed lease, finalized reads still showed the full 1,000,000 CVNT in
the original position and vault, with unchanged lock expiry and no deduction.

## Live result

1. The native node allocated one fresh VM/disk with the no-KMS profile. Operator
   and coordinator evidence agreed on the independently verified TDX app/boot
   identity and original buyer, job, workload, host key and lease-expiry scope.
   Each quote bound its own separate fresh challenge and nonce.
2. The buyer independently verified admission and opened the original signed
   relay session within its quote deadline. It uploaded **81,920 bytes** from
   the client to `/workspace/native-acceptance.bin` as unprivileged UID 10001.
   The returned file hash matched the client source:
   `d980aa7bb22cd97d3d80d5c68cdb9fc4f09bb63c18819cbbb8abb40597025ca6`.
   This is a native client upload, not a file generated inside the guest.
3. Explicit lease close succeeded at 22:50:16.056 UTC. The node requested
   deletion of the original owned VM and retained the cleanup intent while
   Phala continued returning that VM.
4. The 60-second cleanup deadline expired. The signed, verified terminal error
   was `ephemeral Phala deletion is unconfirmed; original resource and cleanup
   intent remain durable; successful receipt refused`.
5. The recovery pass stamped 22:51:38.480 UTC reached durable `deleted`, about
   82 seconds after close; that stamp is not an exact fsync-completion time.
   A separate authenticated read of that exact VM returned 404
   at 22:52:50.614 UTC. Late deletion did not change the failed job into a
   successful lease.

The node's one-use allowance is consumed and the operator is not offered as
production capacity. The deployed coordinator capability remains available for
explicitly authorized qualification; standing production creation is not
enabled. The operator process exited cleanly after confirmed provider deletion
and finalized escrow return. A public status read confirmed `offline`,
`live=false`, `matchable=false`, and no Phala entry in advertised capacity.
The failed attempt is retained in operator reputation as one fault; no stake
deduction was observed.

## Final financial reconciliation

| Item | Final result |
| --- | --- |
| Native job | `failed`, verified receipt, `execution_failed` |
| Buyer charge / operator payout / marketplace fee | 0 / 0 / 0 micro-USDC |
| Buyer available Compute credit | 30,600 micro-USDC |
| Local budget | Pending 0; settled 0; remaining 30,600 micro-USDC |
| Original on-chain renter escrow | Full 30,600 micro-USDC returned, finalized |
| Original escrow vault | Absent |
| Provider VM | Durable `deleted` plus independent exact-UUID 404 |

The original lease terms were voided by finalized
[4iDeKeDit…](https://explorer.solana.com/tx/4iDeKeDitRsXttVURqgiANBu6xUpFBUdyTQJaFFsbs2NvHLuh7SePNuNGeCnVb3tCByx7TC6UTNgcdb3eePKU6RL).
Finalized [29wpMntra…](https://explorer.solana.com/tx/29wpMntraAoDD7HM7J4MhoD5FeiLnHCbWSjBSoSG6YjKwCdG6HYThzmEvJ49vwFY7vVh21CYR6zqN4zC8AJNErnw)
returned exactly 30,600 USDC base units to the coordinator renter wallet.
Both original terms payment flags are set and the vault is absent. The failed
path retains an unconcluded meter owned by the delegation program. Its retained
L1 state does not establish the final active rollup meter state, and this is not
a claim that every on-chain account was closed. Restored buyer credit remains
Compute credit, not a withdrawal to an external buyer wallet. Provider billing,
transaction fees and the CVNT lock are separate from the zero Compute charge.

## Remaining qualification

The repair must provide a bounded provider-deletion grace period and compatible
signed result-delivery slack without extending guest access, signed lease
expiry or metered runtime. Raising only the cleanup timeout is insufficient
for a lease that runs to expiry, because the generic signed result slack is
also 60 seconds. Successful receipts must still require confirmed deletion;
unknown cleanup must remain a failure with durable recovery.

The original failed envelope, resource, receipt and reservations remain in the
record. A new paid acceptance requires fresh authorization and a distinct
one-use allowance. Successful operator payout and paid settlement qualification
remain unproven by this attempt. General production activation remains blocked
until the repaired complete path passes and a standing bounded operator policy
is explicitly selected.

## Private evidence hashes

Raw quotes, collateral, management responses, signed requests and credentials
remain private. Selected retained SHA-256 hashes:

| Evidence | SHA-256 |
| --- | --- |
| Exact-head CI summary | `fd43c7183ee0a2f424edbb4bfa6f76b28f77d75000e465a906f0e76537438a2a` |
| Deployment preservation validation | `85a47438bd7f4bc942f181d71de9d6dc12431b19ca62957f4e48b7a1275f7e4d` |
| Initial persistent feature paths | `5111fb7afaf641725d861364e7cda35c9e3a35777d600773e6f73b0230060cba` |
| Verified native client upload | `5118c61bcddb540dda2bd05483aabbd7ebcf09ea75ad6becc9ef33fdfc1e7b3f` |
| Independently confirmed provider deletion | `14d5c7ee3b0ddd793a15b8740e1f3fbcfc646de3079c648faf20bb89ce5a81b8` |
| Failed receipt and finalized escrow reconciliation | `a99c8712d584fcd3589cac0629238d4d569269664927e105cab0d629b54d3790` |
| Public operator/capacity status | `6a9be1b22397169715adb86a1c4c429d3cdd5d0d3a7c9cd3df5ffbfd616ae2ca` |
