# Packaged Knowledge CLI: public-source dry run

At 2026-10-20T16:08:12.910122+00:00, the packaged **covenant-compute 0.5.0** candidate reached the expected exact-operator refusal after anonymous public-source verification:

```text
Error: no operator serves this exact pinned knowledge compiler; nothing was submitted
```

This is a successful **no-spend refusal check**, not a successful quote or paid compilation. The dry-run path checks public source before anonymous capacity and returns before signing or purchase-journal code. The evidence for completion of public verification is that code ordering plus the observed refusal; HTTP traffic was not captured.

- Binary SHA-256: `16d5a702cbe298580a130f2707e84ff8e2bb9fc6a423ee31aa07306b423a74bf`.
- Archive SHA-256: `ecf0bdaa46b13c25e0603a314511850d90bcae0aea0ca77b87b2f87f84621858`.
- Repository: [https://github.com/pallets/itsdangerous](https://github.com/pallets/itsdangerous); pinned [commit](https://github.com/pallets/itsdangerous/commit/672971d66a2ef9f85151e53283113f33d642dabd) `672971d66a2ef9f85151e53283113f33d642dabd`.
- Public, non-archived, BSD-3-Clause metadata observed; last push reported 2025-06-14T20:35:46Z. No claim of a recent upstream release.
- Entire root: **50 selected files**, **282,547 source-content bytes**, **294,697 normalized source JSON bytes**, **1,424,760 local snapshot JSON bytes**. Input fits the 256-file/512-KiB remote bounds; local snapshot is below 3 MiB before the small result wrapper.
- Image SHA-256 requested: `d011ce03baa42e825e345bd028c4b9af82e210535958c4ce5b49eb8a0503745c`.
- Coordinator: `https://compute-api.opencovenant.org`.
- Exit code 1; duration 13.013 seconds.

The source size/digest was reconstructed from the same installed CLI's actual local snapshot with capture diagnostics cleared, matching request preparation. The refused remote invocation prints no source measurements. Selected public file paths, lengths and SHA-256 hashes are retained in `public-fixture-evidence.json`; source content is not vendored into repository documentation.

No source scripts, hooks or dependencies executed. A disposable isolated context was necessary because the CLI loads an identity even for dry runs. Its generated identity was deleted after the check. The context created no purchase or budget journal, and the existing buyer was never accessed. **Zero signatures, funding transfers or purchase submissions.**

## Reproduction

Use the exact installed binary to be accepted. Clone the public repository into a separate temporary fixture directory with user/system Git configuration and hooks disabled, and ensure the pinned commit exists. Do not use a subdirectory or modify exclusions to evade root capture. Use a disposable context only for this unpaid preview; paid acceptance must use its reviewed original identity and journals.

```sh
COVENANT_COMPUTE_COORDINATOR_URL=https://compute-api.opencovenant.org \
COVENANT_COMPUTE_MCP_HOME="$disposable_context/buyer" \
COVENANT_KNOWLEDGE_HOME="$disposable_context/knowledge" \
COVENANT_COMPUTE_MAX_PRICE_MICRO_USDC=50000 \
"$installed_cli" knowledge compile "$fixture_root" \
  --commit 672971d66a2ef9f85151e53283113f33d642dabd \
  --remote --repository https://github.com/pallets/itsdangerous \
  --compiler-image-sha256 d011ce03baa42e825e345bd028c4b9af82e210535958c4ce5b49eb8a0503745c \
  --dry-run --json
```

The named shell variables must be absolute paths to the prepared temporary context, unpacked installed binary and cloned repository root. No idempotency key is supplied. Preserve the resulting stdout, stderr, exit status and binary hash. Revalidate capacity at the time of the run; if an exact operator appears, this dry run can return a preview quote but still cannot buy.

This binary was packaged before the final source-anchor rebuild; the hashes above identify the bytes actually tested. The [final native archive](../release-candidate-0.5.0/report.md) contains the byte-identical buyer executable, verified by SHA-256. Its archive hash differs because documentation and build metadata changed. This correspondence is recorded in `public-fixture-evidence.json`; a different buyer executable would require new preflight evidence.
