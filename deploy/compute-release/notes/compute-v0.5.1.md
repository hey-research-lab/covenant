Covenant Compute 0.5.1 updates the local Knowledge viewer to match Covenant's
website typography and black, violet, blue and mint palette. It adds the bundled
Archivo body font while retaining the source inspection, dependency and snapshot
comparison workflows introduced in 0.5.0.

The package also includes the current Knowledge guide and a remote compilation
guide with the dedicated operator's acceptance evidence. Remote capacity is a
live network condition: check the exact model and quote before buying work. This
release does not replace the deployed operator or change the accepted compiler
image, source limits, signed job protocol, receipts or payment rules.

## Use Knowledge

From the unpacked archive:

```sh
./covenant-compute knowledge compile ./examples/knowledge --json
# Use the project ID returned by compile:
./covenant-compute knowledge query calculate_total --project PROJECT_ID
./covenant-compute knowledge serve --project PROJECT_ID
```

The viewer prints a private URL bound to loopback and the selected project.
Local compilation, search, verification and the six read-only MCP tools need no
wallet, coordinator or model API key. Source is parsed rather than executed.
See the bundled `docs/compute-knowledge.md` for supported languages, static
analysis limits, citations, snapshot comparison and MCP configuration.

After installation, the bundled example is at
`~/.local/share/covenant-compute/0.5.1/examples/knowledge` by default. The local
index remains at `~/.covenant/knowledge`; installing this release does not reset
saved projects or snapshots.

## Install

The archive contains four commands, original examples, guides, licenses and
recorded evidence. After verifying the download:

```sh
sh ./install.sh
covenant-compute --version
```

The expected version is `0.5.1`. The default binary directory is `~/.local/bin`;
documentation and examples are retained under
`~/.local/share/covenant-compute/0.5.1`. Override those locations with
`COVENANT_COMPUTE_INSTALL_DIR` and `COVENANT_COMPUTE_DATA_DIR`.
Installation does not configure a wallet, MCP client or network.

The website installer and Homebrew formula are pinned separately. Do not infer
that either has advanced from this release's publication; inspect the installed
version. The reviewed archive installer works offline.

## Verify a download

The release workflow builds Apple Silicon and Intel macOS, and static-musl x86-64
and ARM64 Linux. Each extracted archive checks all four commands and runs the
offline source, citation and MCP acceptance before publication. For the published
Linux x86-64 archive:

```sh
cosign verify-blob \
  --signature covenant-compute-linux-x86_64.tar.gz.sig \
  --certificate covenant-compute-linux-x86_64.tar.gz.pem \
  --certificate-identity https://github.com/open-covenant/covenant/.github/workflows/compute-release.yml@refs/tags/compute-v0.5.1 \
  --certificate-oidc-issuer https://token.actions.githubusercontent.com \
  covenant-compute-linux-x86_64.tar.gz
grep '  covenant-compute-linux-x86_64.tar.gz$' checksums.txt | sha256sum -c
gh attestation verify covenant-compute-linux-x86_64.tar.gz --repo open-covenant/covenant
```

Use the matching archive name for another platform and `shasum -a 256 -c` on
macOS. Checksums alone do not authenticate the release: verify its signature and
workflow provenance. No new paid acceptance job is implied by a client update;
the included remote records distinguish bounded paid acceptance, dedicated
operator activation, and the current quote-only preview.
