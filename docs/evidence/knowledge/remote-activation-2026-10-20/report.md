# Dedicated Knowledge operator activation

**Limited public capacity accepted on 20 October 2026 at 19:12 UTC.** One
dedicated operator advertised the accepted compiler at **50,000 micro-USDC /
0.05 USDC per job**, with one job in flight. The existing Kani service remained
active. The [activation manifest](activation.json) separates deployment,
[finalized stake](stake.json), [cohosting tests](coexistence.json),
[live capacity](capacity.json), [controlled restart](lifecycle.json) and the
[installed-client preview](installed-preview.json). Capacity and price are
observations, not a guarantee of future availability.

## Finalized stake

Operator `CfaV6nB59aDnTJccD5WXUiQejg9prMvtQrkFqQZRhWmf` received a finalized
1,000,000 CVNT stake in the settlement program. The original signed transaction
was persisted before broadcast and reconciled by that same signature. Its
account deltas, operator binding, amount, vault and lock were checked.
[Inspect the finalized transaction](https://explorer.solana.com/tx/3xUhtz2Cg1LoF4YWZmwqiET5ENLmXPqLAgLVyAUuwfGLWztV4S3HrkVrc72FBuB9YtbQTBFizCFATATpEWj36ukD).

Opening the three accounts reserved 0.0042164 SOL; the staking transaction fee
was 0.000025 SOL. The stake transaction's total debit was 0.0042414 SOL.
The CVNT remains locked until **27 October 2026 at 19:17:25 UTC**. Admission
requires at least 48 hours remaining, so the initial eligibility cutoff is
**25 October 2026 at 19:17:25 UTC**, unless the owner extends it.
Neither timestamp guarantees uptime or reserves execution capacity.

An unsigned live `ExtendStake` simulation with zero added tokens passed.
No extension has been executed or scheduled. The initial lock was seven days
plus the released CLI's ten-minute clock slack.

## Cohosting acceptance

The Knowledge service uses a dedicated identity, home, unit and resource group.
The accepted host configuration permits one Knowledge job in flight, with a
256 MiB aggregate memory limit and one CPU of aggregate quota. The node service
itself is capped at 128 MiB. The compiler remains the accepted Linux ARM64
digest, executed through the host's established AMD64 emulation.

The [sanitized cohosting record](coexistence.json) retains the following:

- Memory pressure and an intentional out-of-memory probe stayed within the
  Knowledge resource group; the existing Kani service remained active.
- A real near-limit fixture containing 256 files and 524,000 bytes of source
  JSON produced a 769,919-byte snapshot.
- The final concurrent run completed that compiler fixture and a small real
  Kani proof; the Kani outcome was `proven`.
- Existing Kani configuration, identity and financial journals remained
  unchanged, its capacity remained available, and test containers were removed.
- Three earlier concurrent fixture attempts were incomplete because the Kani
  fixture did not pass. Their exit codes remain in the evidence; they are not
  counted as acceptance.

These tests establish the observed bounded case. They do not establish sustained
peak-load behavior, a full 12 GiB Kani peak, host reboot recovery or a paid job
under the new operator identity.

## Live service and installed buyer

Fifteen observations between **19:10:24 and 19:11:42 UTC** show the dedicated
operator registered, staked, online and matchable, with the exact Knowledge
model at 50,000 micro-USDC. Kani remained available at its original 19,000
micro-USDC ask. The Knowledge service was restarted only after authenticated
job inspection and local accepted/outbox records showed no outstanding work.
It returned with the same identity and journals. The original Kani process and
protected files stayed unchanged; no compiler containers or new memory events
remained.

This is an idle service restart, not recovery of an in-flight customer job.
The services share one host and failure domain. No host reboot or redundant
operator was tested.

At **19:12:11 UTC**, the freshly installed signed Apple Silicon Compute 0.5.0
buyer verified the pinned public `pallets/itsdangerous` fixture and received the
50,000-micro-USDC quote for the exact compiler. Its output recorded
`dry_run: true`, `submitted: false`, 294,697 source JSON bytes and the accepted
source digest. Capacity contained both Knowledge and Kani before and after.
Existing buyer accounting stayed byte-identical. No idempotency key, new job,
purchase reservation or additional USDC charge was introduced.

The [two earlier paid jobs](../paid-pilot-2026-10-20/report.md) used the same
compiler through a temporary operator and passed signed receipt, deterministic
import, interruption/recovery, replay and finalized payout checks. Those paid
results remain distinct from this dedicated operator's activation.
No new USDC purchase is part of this activation, and no paid execution under
the new identity is claimed.

The [public-source buyer guide](../../../compute-knowledge-remote.md) explains
the preview, budgets, limits and recovery. The public
[capacity directory](https://compute-api.opencovenant.org/federation/capacity)
contains aggregate counts and asks; it does not publish operator identities.
Private host addresses, credentials, buyer balances, pending holds, financial
journals and local identity paths are excluded from this public record.
