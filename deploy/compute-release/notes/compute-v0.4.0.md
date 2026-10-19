Covenant Compute 0.4.0 adds explicit model-specific inference tariffs. Buyers can
see a fixed USDC quote based on the selected model and requested output limit
before signing. Requests without a tariff retain their existing pricing behavior.

## Upfront inference tariffs

`covenant-compute infer --tariff-id ID --model MODEL --max-tokens N` selects a
currently advertised tariff. Add `--dry-run --json` to inspect its exact price,
revision, validity window, model, and output cap without buying work. The MCP
`compute.infer` tool accepts the same `tariff_id`, `model`, and `max_tokens` fields.

The quote is a base price plus an output-cap component, rounded upward to whole
USDC microunits. The full price settles on successful execution; a shorter answer
does not reduce the quote. Existing per-purchase and total spending controls
still apply. A missing or expired tariff fails the tariff request instead of
falling back to a higher price.

The buyer-signed job binds the exact tariff snapshot. Routing and node admission
check the operator's advertised offer. Retrying an existing purchase keeps its
original signed quote and job identity; it does not reprice or create a new
purchase. Legacy profiles and envelopes preserve their existing signing format.

Operators explicitly configure tariffs with
`COVENANT_COMPUTE_NODE_TARIFFS_FILE`. There are no built-in native model prices.
Installing these commands does not configure or activate a production tariff.

## Gateway and client compatibility

The accompanying gateway source adds `pricing: "model_tariff_v1"` as an explicit
x402 request opt-in and exposes configured tariff metadata through
`GET /v1/models`. Requests that omit `pricing` keep the existing flat path.
The JavaScript x402 client 0.3.0 and Python x402 client 0.2.0 are separately
packaged releases; they are not installed by this command archive.

The initial commercial proposal applies only to the already qualified Haiku 4.5
text model. Upstream provider budgets, model pinning, and conservative input-token
reservations remain unchanged. Deployment, provider acceptance, chain settlement,
and publication are recorded separately from local checks. These notes do not
establish that new prices are live.

## Included commands and verification

The release archive format contains `covenant-compute`, `covenant-compute-mcp`,
`covenant-compute-node`, and `covenant-compute-stake`, all versioned `0.4.0`, plus
the license, notice, command guide, and these notes. The release workflow targets
Apple silicon and Intel macOS, and x86-64 and ARM64 Linux. Linux executables use
static musl linking.

For a published archive, verify its checksum, signature, and workflow provenance.
The workflow supplies cosign signatures and certificates for each archive and
`checksums.txt`. For the Linux x86-64 archive:

```sh
cosign verify-blob \
  --signature covenant-compute-linux-x86_64.tar.gz.sig \
  --certificate covenant-compute-linux-x86_64.tar.gz.pem \
  --certificate-identity https://github.com/open-covenant/covenant/.github/workflows/compute-release.yml@refs/tags/compute-v0.4.0 \
  --certificate-oidc-issuer https://token.actions.githubusercontent.com \
  covenant-compute-linux-x86_64.tar.gz
grep '  covenant-compute-linux-x86_64.tar.gz$' checksums.txt | sha256sum -c
gh attestation verify covenant-compute-linux-x86_64.tar.gz --repo open-covenant/covenant
```

Use the matching filename for another platform and `shasum -a 256 -c` on macOS.
Signature and provenance checks identify the published build; they do not prove
production tariff activation or inference correctness. Homebrew and the website
installer use separately pinned releases, so check `covenant-compute --version`.

The [tariff guide](https://github.com/open-covenant/covenant/blob/compute-v0.4.0/docs/compute-inference-tariffs.md)
describes request behavior. The
[release evidence](https://github.com/open-covenant/covenant/blob/compute-v0.4.0/docs/compute-model-pricing-release-evidence.md)
separates commercial policy, measured samples, local validation, and production
release gates.
