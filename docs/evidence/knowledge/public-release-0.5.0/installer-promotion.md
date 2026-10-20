# Installer promotion

The signed release and native CI checks are complete; see [release verification](report.md).

## Website installer: passed

Observed 2026-10-20T17:36:47.190600+00:00. The actual public [website installer](https://opencovenant.org/install/compute.sh) returned exactly the reviewed SHA-256 `18883da2515ea243ef7c95a92c1fac3a60f8c30f55d25df17c17c688efa0b0ef`. It was then executed in a fresh isolated installation prefix. It downloaded the actual public 0.5.0 archive over HTTPS and installed all four commands and supporting files. All installed binary hashes and versions match the independently authenticated release. The installed Knowledge workflow passed 19 checks, 18 exact-source citations, and all six read-only MCP tools; no wallet, budget, or HOME state was created.

The website deployment is `5b86392ab80be15611cba4bec6cc24ee48b7de43`; the separately built and signed binary release source is `8cbaeb7e423eead8bf12389820857437ad693717`. See [live installation evidence](live-website-install.json).

The installer body equals the accepted binary release commit's data-aware installer outside its five pinned values. [Pin proof](pin-proof.json) records the script and archive digests. A [predeployment HTTPS acceptance](predeployment-script-acceptance.json) also passed using the reviewed local script copy. The live installation above used the deployed public script URL, with no transport stub.

## Homebrew: promotion blocked

The Homebrew formula was prepared from the same authenticated checksums and passed Ruby syntax and diff checks. Normal Git publication with the existing credential was denied; a fresh read confirms the public tap remains 0.4.1. No alternate credential or write route was attempted, and no 0.5.0 Homebrew CI or installed acceptance is claimed. See [promotion result](homebrew-promotion.json) and the [reviewable proposed formula diff](homebrew-proposed-0.5.0.diff).

Signed archive and website-script installation are verified and may be advertised independently. Do not advertise Homebrew as a 0.5.0 installation method until that repository's normal publication and installed acceptance checks pass.
