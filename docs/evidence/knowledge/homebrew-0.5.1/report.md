# Homebrew 0.5.1 candidate acceptance

Observed 2026-10-21T00:38:50.109851+00:00. The candidate is verified locally; the public tap remains **0.4.1**.

The existing authenticated Git credential and GitHub connector both report read access but no push, maintain or admin permission for `open-covenant/homebrew-tap`. No alternate credential, remote branch, pull request or main update was used. Publication needs repository write access for the existing identity, or a maintainer to apply [the prepared patch](candidate.patch.gz) through the tap's normal checks.

## Candidate

- Baseline: `1e9ad904647bbae930a347be1414e00b4d0daccc` (freshly fetched public main).
- Local branch: `codex/compute-0.5.1`; commit `2cec26210097920b6447a4d6f666d4a2b45fd8d0`.
- Updates exactly four archive pins to the independently signature/attestation verified [Compute 0.5.1 release](https://github.com/open-covenant/covenant/releases/tag/compute-v0.5.1), source `809aa264c2c1cb965c11001052096eece5caf0cf`.
- Retains the four executables; requires installation of bundled examples, guides, licenses, acceptance tools and build metadata under `share/covenant-compute`.
- Formula test compiles the installed multi-language example, checks exact source excerpts, verifies the snapshot, initializes MCP and checks the six read-only tools and a real search.
- README uses the actual Homebrew data path. Existing workflow and branch protections are unchanged.

## Verification

Actual fresh Homebrew installation in an isolated custom prefix on macOS ARM64 passed. `brew style`, `brew audit --strict --online`, and `brew test` passed. All four installed binary versions and byte hashes match the signed public archive. All 271 installed data files match that archive.

The installed packaged acceptance tool passed **19 checks**, **18 source citations**, and all **six read-only MCP tools**, including mutation/project-boundary rejection, unchanged source and absence of a buyer identity. It used no wallet, daemon, model key or purchase.

Homebrew consumed the retained verified public archive from its cache; no release was rebuilt. All four platform pins were rechecked against the signed public archives. Native Homebrew installation was exercised only on macOS ARM64; the tap's existing macOS/Linux CI remains pending authorized publication. The custom test prefix has Homebrew support-tier caveats and does not claim a fresh default-prefix or other-platform installation.

Machine-readable details are in [acceptance.json](acceptance.json). The existing permission check is current; the previous HTTP403 push was a separate historical 0.5.0 attempt. No new push was attempted after the current read-only permission checks.

The patch is gzip-compressed to preserve its exact Git mail-patch whitespace. Decompress before review/application: `gzip -dc candidate.patch.gz > candidate.patch`. Uncompressed SHA256: `0277114727a8ed22d8380a50a209f3074a457d3b6c480569876ad29d31559dae`.
