# Covenant Compute 0.5.0 public release verification

Observed 2026-10-20T17:19:18.406109+00:00. Result: **pass** for the four signed public release archives and the native post-download installation.

- [Published release](https://github.com/open-covenant/covenant/releases/tag/compute-v0.5.0)
- [Accepted source](https://github.com/open-covenant/covenant/commit/8cbaeb7e423eead8bf12389820857437ad693717)
- [Four-platform candidate and fresh installs](https://github.com/open-covenant/covenant/actions/runs/39460383574)
- [Tagged builds, fresh installs, signing and publication](https://github.com/open-covenant/covenant/actions/runs/39462626252)
- [Machine-readable verification](verification.json), [authenticated checksums](checksums.txt)

Each archive passed Cosign verification for the exact `compute-v0.5.0` release-workflow identity and GitHub OIDC issuer; signed SHA-256 verification; GitHub attestation verification restricted to the accepted source commit, tag ref, workflow and hosted runners; clean source/build metadata; and all four packaged binary hashes. The downloaded public archive bytes equal the corresponding tag-run artifact that passed a fresh installation on its native runner.

| Platform | Public archive SHA-256 | Verification |
| --- | --- | --- |
| darwin-arm64 | `7bb4a1ebfbc9aa20c725fb637f5ede9585da412b56f014eed314a9d01995e586` | pass |
| darwin-x86_64 | `0525372e349c5ad01f16cef22d5c488d5022bd435236c7642b9f1e0069fee74d` | pass |
| linux-x86_64 | `ba1185a4650d953fa71fabb0a0e4b8beee619b17bd3b9d53b7c10396ccf60cf2` | pass |
| linux-arm64 | `26ccb6c8305f4828f56906b662c2d4d136802128672efa884379675e43470495` | pass |

The downloaded Apple Silicon archive also passed an actual fresh offline installation after publication: four command versions and binary hashes, retained notices/examples, repeat install, conflicting-data rejection, source citations, and six read-only MCP tools. The Knowledge smoke recorded 19 checks and 18 citations on the original packaged example. No wallet, budget, purchase or user configuration was created.

The website-transport portion of `check-install.py` uses a temporary local transport stub. It is not evidence of a deployed website pin or a Homebrew installation. The separate [installer promotion record](installer-promotion.md) now records a successful fresh installation through the actual public website URL and the blocked Homebrew update. The public release is local Knowledge; remote supply and the bounded paid pilot are separate acceptance records.

The certificate, signature and attestation JSON files retained here are public release provenance. No binary archives, buyer financial state, local private paths or keys are included.
