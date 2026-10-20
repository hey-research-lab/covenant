# Covenant Compute 0.5.1 public release verification

Observed 2026-10-20T19:53:16.023238+00:00. Result: **pass** for all four signed public archives and the native post-download installation.

This patch restores the Covenant visual identity in the embedded Knowledge viewer: black surfaces, official fonts and mark, violet panels and blue focus states. It updates the packaged installation guides. The Knowledge compiler image, coordinator routing and deployed operators remain unchanged.

- [Published release](https://github.com/open-covenant/covenant/releases/tag/compute-v0.5.1)
- [Accepted release source](https://github.com/open-covenant/covenant/commit/809aa264c2c1cb965c11001052096eece5caf0cf)
- [Four-platform code candidate and fresh installs](https://github.com/open-covenant/covenant/actions/runs/39482506021)
- [Tagged builds, fresh installs, signing and publication](https://github.com/open-covenant/covenant/actions/runs/39484253976)
- [Machine-readable verification](verification.json), [release and installer acceptance](release-acceptance.json), [authenticated checksums](checksums.txt)

Each public archive passed Cosign verification for the exact `compute-v0.5.1` release-workflow identity and GitHub OIDC issuer, signed SHA-256 verification, and GitHub provenance verification restricted to the accepted source commit, tag ref, workflow and hosted runners. All archives identify clean source `809aa264c2c1cb965c11001052096eece5caf0cf`, and their four binary hashes match their build manifests. The public archive bytes equal the corresponding tag-run artifacts that passed fresh installation on their native runners.

| Platform | Public archive SHA-256 | Verification |
| --- | --- | --- |
| darwin-arm64 | `8cf3b09bcd4ea950d59a753a98b026e010f6078ed26727fad4062620391b28b1` | pass |
| darwin-x86_64 | `ecd80b4bbb8bfb53093cdb9f0096f3f39bab0ffc7622b366b170ccc8f0d1c74e` | pass |
| linux-x86_64 | `4873355a9db5dbc18cb8a3e6a71fcddb8e3d77e779b3485cc3785a6e5b8ec484` | pass |
| linux-arm64 | `f77c04008d6352df3b9c8daef9638715e6ea70fdd8540ac6f95434ba8fef2e96` | pass |

The downloaded Apple Silicon archive also passed an actual fresh offline installation after publication: all four command versions and binary hashes, retained notices/examples, repeat installation and conflicting-data rejection. The packaged Knowledge smoke passed 19 checks, including 18 exact citations and all six read-only MCP tools. No wallet, budget, purchase or user configuration was created.

The website-transport portion of `check-install.py` uses a temporary local transport stub. It does not establish live website pins or a Homebrew update. The separately reviewed installer changes only its version and four archive pins; its body stays byte-identical outside that block. That prepared body also passed a fresh installation using the actual public HTTPS release archive, with all four binary hashes and the same Knowledge checks verified. Its SHA-256 is `74fdf1d23c38597d0ada3015f47a71a86e87cc07c43ff1e1e89a72627dffc9dc`. Homebrew remains at 0.4.1.

The subsequent [website and served-installer promotion](installer-promotion.md) passed at 20:40 UTC. Required branch and application checks passed before the normal protected main fast-forward to `d693a8dee46a09d7a363650371ed6ced91b1d576`. The existing Render service deployed that exact source without configuration changes. The live HTTPS installer matched the reviewed hash and passed a fresh public-archive installation with all four binary hashes, 19 Knowledge checks, 18 citations and six MCP tools. The Knowledge page's six standard Covenant CTAs passed desktop, tablet and mobile review; all six release, guide and evidence links returned 200. [Machine-readable promotion evidence](installer-promotion.json) records the scoped dependency and font-build repairs, CI and deployment identities, and live checks.

The earlier `release-acceptance.json` is preserved as the original 19:55 UTC observation, when website promotion was still pending. The later promotion record establishes that this gate passed. [Public capacity at 20:37 UTC](capacity-final.json) showed both Knowledge and Kani available; this dated observation is not an uptime or standing-price guarantee.

The release was tagged once after the four-platform code candidate passed. The final source differs from that candidate only in two packaged installation guides; locally built executable hashes were identical. Public CI executables have their own recorded hashes and are not claimed to match the local build environment.

The [dedicated remote operator activation](../remote-activation-2026-10-20/report.md) and [two earlier paid jobs](../paid-pilot-2026-10-20/report.md) retain separate evidence. This viewer patch did not redeploy the operator, change its pinned compiler or add a paid acceptance job. The campaign film uses genuine candidate-source recordings and clearly labels the earlier paid receipt; it is not a new paid execution.

Certificates, signatures and attestation JSON retained here are public release provenance. No archives, keys, private buyer journals or private source repositories are included.
