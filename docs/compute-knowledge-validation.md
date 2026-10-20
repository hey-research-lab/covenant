# Knowledge validation and release evidence

Recorded 20 October 2026 on Apple Silicon macOS with Rust 1.98.1. This is an
independent implementation based on the refreshed deployment source
`72f66142f407690024c6a0ca4a3bdaf148ef8766`. It does not replace existing production
acceptance records or authorize spending, deployment, release publication, or posts.

## Local results

The following command completed with **1,236 passed, zero failed, 40 ignored**
across 26 test binaries. Ignored environment-dependent tests were not accepted
as evidence; no real OCI runtime or funded job was exercised.
The [machine-readable result](evidence/knowledge/validation-2026-10-20.json)
records tested source `fa8c47153c859fac092ab7397b32d61937e0efcf`.

```sh
cd agent-os
CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 CARGO_INCREMENTAL=0 \
  cargo test -p covenant-knowledge -p covenant-compute-protocol \
  -p covenant-compute-node -p covenant-compute-buyer --lib --tests --quiet
```

The core suite includes 22 original extraction/lifecycle tests and three concurrency tests for all supported language families,
exact captured source spans, dirty and committed revisions, exclusions,
symlinks and invalid paths, malformed syntax, ambiguous calls, multiple imports,
Markdown sections, incremental/full equivalence, deletion/rename history,
immutable rows, cache corruption, index reconstruction, project isolation,
schema upgrade, and rollback after an injected transaction failure. The rollback
test simulates commit failure; it is not a power-loss test.

The four standalone CLI integration tests cover offline local operation, bounded
MCP frames and project selection, mutation refusal, snapshot selection, and
loopback viewer session, origin, host, proxy-header and body-size restrictions.
They were rerun successfully after the final CLI deadline validation change.

Remote tests cover exact capability matching (including rejection of `any` and
`:latest` aliases), typed contracts, source/result tampering, zero-spend previews,
durable request identity, verified receipt replay, node admission, and resource
arguments and cleanup using a fake container runtime. The HTTP test uses a real
local coordinator and signatures, with mock payment, source verification and
container execution explicitly identified in its exported evidence.

The signed HTTP fixture was rerun successfully after adding an opt-in evidence
export. The export contains no private signing keys. It is **local test evidence**,
not a production payment, a GitHub visibility attestation or a real OCI run.

## Subsequent shipping and Docker checks

The candidate received a distinct **0.5.0** version while the public installer
remained pinned to the existing 0.4.1 release. Its native Apple Silicon archive and
fresh installed Knowledge workflow passed the 19 offline checks, including 18
exact citations and all six MCP reads. The release path now retains examples,
guides, licenses and build metadata after installation. The
[final native acceptance record](evidence/knowledge/release-candidate-0.5.0/report.md)
identifies clean source commit `3d4499211fabec97da0ee918dc281bd3a4226758`,
the exact archive and binary hashes, and 11 installer checks plus destination
conflict refusal.

The subsequent [signed public 0.5.0 release](https://github.com/open-covenant/covenant/releases/tag/compute-v0.5.0)
was built from `8cbaeb7e423eead8bf12389820857437ad693717` through the normal
[release workflow](https://github.com/open-covenant/covenant/actions/runs/39462626252).
All four native macOS/Linux targets passed package and fresh-install checks.
Downloaded public archives independently passed exact tag/workflow Cosign
identity, signed checksum, source/ref attestation, clean build metadata and
binary-hash verification. The downloaded Apple Silicon package also passed a
fresh local installation. The deployed
[website installer](https://opencovenant.org/install/compute.sh) then passed a
fresh installation with real HTTPS downloads, all four binary hashes and
versions, 19 Knowledge checks, 18 citations and all six MCP tools.
See the [installer promotion record](evidence/knowledge/public-release-0.5.0/installer-promotion.md)
and [shipping checklist](../deploy/compute-release/KNOWLEDGE_RELEASE.md).
Homebrew remains on 0.4.1 because the tap rejected the normal update with HTTP
403; it is not advertised as a Knowledge installation method.

The production Docker executor subsequently passed the
[real-container HTTP acceptance](evidence/knowledge/docker-http-acceptance-2026-10-20.json):
signed fixture receipt, deterministic import, one compiler execution, same-job
replay and tamper/conflict refusal. The
[container lifecycle probes](evidence/knowledge/docker-lifecycle-acceptance-2026-10-20.json)
passed all 11 isolation/cleanup checks. See
[build provenance](evidence/knowledge/docker-build-acceptance-2026-10-20.json)
for the installed Linux/ARM64 image digest and source input hashes. Funding,
payout and public-source verification in the HTTP fixture are substituted;
no paid acceptance is claimed.

The byte-identical 0.5.0 buyer also completed a
[public-source preflight](evidence/knowledge/public-source-preflight-2026-10-20/report.md)
against a pinned public Python repository: 50 selected files and 294,697 bytes
of normalized source JSON. It then refused before signing because no operator
advertised the exact accepted image digest. This establishes the observed
zero-spend refusal and, from the reviewed call order, completion of anonymous
source verification; it is not a successful quote or remote job.

The subsequent [paid pilot acceptance](evidence/knowledge/paid-pilot-2026-10-20/report.md)
records a fresh source-verified quote and two successful production jobs on the
exact compiler digest. The normal job replayed with the same receipt and no
additional debit. The second client was killed after authoritative submission;
a new process recovered its original signed envelope, then settled replay also
passed. Independent signature, artifact and source hashes, deterministic local
import, all 1,378 citation spans, and both finalized on-chain payouts verified.
Host evidence records exactly two executions and one paid earnings entry per
job. Total charges were 0.10 USDC with 0.000015 SOL in network fees. The existing
hold was preserved. The original operator service and price were restored.

The subsequent [dedicated operator activation](evidence/knowledge/remote-activation-2026-10-20/report.md)
passed finalized 1,000,000 CVNT staking, bounded cohosting with Kani, fifteen
live capacity observations around an idle service restart, and an installed
public 0.5.0 source-and-quote preview. At 19:12 UTC, one operator advertised the
exact compiler at 50,000 micro-USDC per job, with at most one job in flight. Kani's
original process, configuration and journals stayed unchanged. The preview
submitted no job and left buyer accounting byte-identical.

This establishes limited capacity, not a new-identity paid execution, sustained
peak-load performance, reboot recovery or guaranteed uptime. Initial stake
admission eligibility ends 25 October 2026 at 19:17:25 UTC unless extended;
locked tokens unlock two days later. No automatic renewal is scheduled.

The central website graph was updated to square glass nodes. Its production
build and desktop/mobile layout checks passed; campaign exports retain exact
brand fonts and the official mark at 1080p and 4K composition sizes.

## 0.5.1 patch release verification

The subsequent [signed public 0.5.1 patch release](https://github.com/open-covenant/covenant/releases/tag/compute-v0.5.1)
was built from `809aa264c2c1cb965c11001052096eece5caf0cf`. Its
[four-platform code candidate](https://github.com/open-covenant/covenant/actions/runs/39482506021)
passed before the tag was created. The final source adds only two installation
guide corrections to that candidate; all four locally built executables remained
byte-identical. The [tagged release workflow](https://github.com/open-covenant/covenant/actions/runs/39484253976)
then rebuilt and freshly installed all four native macOS/Linux targets before
signing and publishing the 15 expected assets.

Independent downloads of all four public archives passed exact tag/workflow
Cosign identity, signed checksum, source/ref attestation restricted to hosted
runners, clean build metadata and all four binary-hash checks. Each archive was
byte-identical to the corresponding tag-run artifact that passed its native
fresh-install checks. The public Apple Silicon archive also passed a fresh
offline installation, reinstall and conflicting-data rejection, plus 19
Knowledge checks, 18 exact citations and all six read-only MCP tools. The
[0.5.1 verification record](evidence/knowledge/public-release-0.5.1/report.md)
retains the exact source, archive and binary hashes. Public CI executable hashes
are recorded separately from the local build environment.

The separately prepared 0.5.1 website installer changes only its version and four
archive pins; bytes outside that block match the live 0.5.0 baseline. Its SHA-256
is `74fdf1d23c38597d0ada3015f47a71a86e87cc07c43ff1e1e89a72627dffc9dc`.
That reviewed body downloaded the actual public release over HTTPS, installed
all four matching binaries into an isolated prefix, and passed the same 19
Knowledge checks without HOME or wallet state. The website-transport subtest
inside `check-install.py` is separately identified as a local stub.

The separate [live 0.5.1 website promotion](evidence/knowledge/public-release-0.5.1/installer-promotion.md)
passed at 20:40 UTC. All required branch and application checks passed before
normal protected main promotion of `d693a8dee46a09d7a363650371ed6ced91b1d576`.
Render automatically deployed that exact source with unchanged service
configuration. The actual served HTTPS installer matched the reviewed hash and
passed a fresh installation with all four public binary hashes, 19 Knowledge
checks, 18 citations and six MCP tools. Live CTA, focus, layout and link checks
passed at desktop, tablet and mobile widths. Homebrew has not been promoted.
This viewer and guide patch did not replace the compiler image, coordinator or
deployed operators, or add a paid job.

## Interface and measurement evidence

- [Viewer acceptance](evidence/knowledge/viewer-acceptance.md): actual saved
  snapshot search, evidence, dependency, diff and verify flows at desktop,
  tablet and mobile widths.
- The public console proxy regression checks reject knowledge access before
  reading its shared operator token or making an upstream request. Run
  `node agent-os/covenant-web/scripts/check-knowledge-proxy.mjs`.
- [Reproducible retrieval measurements](evidence/knowledge/benchmark-2026-10-20/report.md)
  retain raw commands, stdout, timings, corpus and executable hashes. All 80
  retrieval runs completed as expected. On this nine-file, 1,813-byte fixture,
  direct `rg` search was faster and smaller in the initial unoptimized run.
  The [optimized preview run](evidence/knowledge/benchmark-release-2026-10-20/report.md)
  had lower Knowledge command latency while direct search still returned fewer
  bytes in every matching case; background test load was not isolated. No speed, token savings or LLM answer-quality claim is supported.
- [Behavioral research](compute-knowledge-research.md) distinguishes observed
  behavior of the pinned reference application from Covenant's original tests.

## Separate release statuses

| Gate | Status |
|---|---|
| Local compiler and interfaces | Implemented; relevant suites and responsive viewer checks passed |
| Local package candidate | Final 0.5.1 Apple Silicon package from clean commit 809aa264c passed fresh installation and its literal packaged-guide command; executables matched the accepted code candidate. [Historical 0.5.0 candidate evidence](evidence/knowledge/release-candidate-0.5.0/report.md) remains separate |
| Signed public macOS/Linux release | [compute-v0.5.1 published](https://github.com/open-covenant/covenant/releases/tag/compute-v0.5.1); all four native builds/installs, public archive signatures, source attestations and CI-artifact equality verified; fresh public Apple Silicon archive installation passed; [evidence](evidence/knowledge/public-release-0.5.1/report.md) |
| Website installer promotion | [Live 0.5.1 promotion passed](evidence/knowledge/public-release-0.5.1/installer-promotion.md): actual served HTTPS body matched the reviewed hash and passed fresh public-archive installation, all four binary hashes and 19 Knowledge checks. Only five installer pin assignments changed. Historical 0.5.0 evidence is preserved; Homebrew remains 0.4.1 |
| Actual OCI compiler image | Exact Linux/ARM64 image passed Docker lifecycle checks and two funded production jobs on an isolated host; image has no public registry release |
| Public operator and routing rollout | Coordinator/node revision 5146e46e6bae31bf6813ca85f099674e878a1ae9; [dedicated limited capacity accepted](evidence/knowledge/remote-activation-2026-10-20/report.md), at most one in-flight job and observed 50,000-micro-USDC ask; Kani preserved |
| Dedicated operator lifecycle | Finalized stake, bounded cohosting, live registration/heartbeat and guarded idle restart passed; actual installed public-client preview passed with no new job or accounting changes; initial admission cutoff 25 October 2026 at 19:17:25 UTC |
| Installed client paid acceptance | Two funded jobs passed with the clean native 0.5.0 candidate; the freshly installed signed public Apple Silicon binary then recovered both original receipts and imported identical snapshots without new jobs or charges; [evidence](evidence/knowledge/paid-pilot-2026-10-20/report.md) |
| Real process restart / financial reconciliation | Actual client interruption after acknowledged submission, same-envelope recovery, identical settled receipt replay, two executions and two finalized payouts verified; no coordinator or host power-loss claim |
| Shared authenticated daemon access | Not exposed; public proxy refuses knowledge access; project authorization adapter remains a separate gate |
| Private remote repositories | Refused by the public-source pilot; operator-access policy remains undefined |
| Feature page and campaign | [Public feature page](https://opencovenant.org/compute-knowledge) and 0.5.1 installer deployed from d693a8dee46a09d7a363650371ed6ced91b1d576; standard CTA styles, three-viewport live review and six release/guide/evidence links passed. Revised artwork and demo prepared; no X posting or scheduling |

Follow the [remote acceptance guide](compute-knowledge-remote-acceptance.md) for
the coordinator → compiler operator → client order and the evidence that must
exist before enabling purchases. Source citations identify captured content.
Signed receipts identify execution and its output. Neither proves correctness
or confidential execution.
