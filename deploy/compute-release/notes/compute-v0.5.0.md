Covenant Compute 0.5.0 adds local Knowledge: compile supported source into saved
snapshots, search exact source evidence, trace bounded dependencies, compare
snapshots, and connect an agent through six read-only MCP tools.

This is a release candidate until the four platform archives pass CI and the
tagged release is signed and published. The remote compiler is an opt-in
implementation; this client release does not announce operator availability,
deployment or funded acceptance.

## Use local Knowledge

From the unpacked archive:

```sh
./covenant-compute knowledge compile ./examples/knowledge --json
# Use the returned project and snapshot IDs:
./covenant-compute knowledge query calculate_total --project PROJECT_ID
./covenant-compute knowledge verify SNAPSHOT_ID --project PROJECT_ID
./covenant-compute knowledge mcp --project PROJECT_ID
./covenant-compute knowledge serve --project PROJECT_ID
```

Local commands need no wallet, coordinator, model API key or source execution.
The compiler extracts Rust, Python, JavaScript, TypeScript/TSX and Markdown.
Snapshots retain captured source and citations with byte ranges, line ranges and
SHA-256 hashes. Dependencies are static observations; dynamic dispatch, macros,
external packages and ambiguous calls can remain unresolved. Verification checks
captured bytes and extraction, not semantic correctness or authorship.

The local viewer binds to loopback and uses a private per-session URL. MCP exposes
only search, get, evidence, dependencies, changes and status for one project.
There is no shared hosted knowledge API in this release.

See `docs/compute-knowledge.md` for limits, historical reads, snapshot comparison,
remote prerequisites and measured retrieval results. The small local benchmark
does not establish production speed, accuracy or token savings.

## Install and verify

The archive includes four versioned commands, original examples, guides,
Cloudflare audit attribution, and pinned Tree-sitter and font licenses.
After verifying the archive, install offline:

```sh
sh ./install.sh
covenant-compute --version
```

The default binary directory is `~/.local/bin`; documentation and examples are
retained under `~/.local/share/covenant-compute/0.5.0`. Override them using
`COVENANT_COMPUTE_INSTALL_DIR` and `COVENANT_COMPUTE_DATA_DIR`.
The installer does not configure a wallet, MCP client or network.

The release workflow builds Apple Silicon and Intel macOS, and static-musl x86-64
and ARM64 Linux. Each extracted archive runs the offline source/citation/MCP
acceptance check before it can be published. For a published Linux x86-64 archive:

```sh
cosign verify-blob \
  --signature covenant-compute-linux-x86_64.tar.gz.sig \
  --certificate covenant-compute-linux-x86_64.tar.gz.pem \
  --certificate-identity https://github.com/open-covenant/covenant/.github/workflows/compute-release.yml@refs/tags/compute-v0.5.0 \
  --certificate-oidc-issuer https://token.actions.githubusercontent.com \
  covenant-compute-linux-x86_64.tar.gz
grep '  covenant-compute-linux-x86_64.tar.gz$' checksums.txt | sha256sum -c
gh attestation verify covenant-compute-linux-x86_64.tar.gz --repo open-covenant/covenant
```

Use the matching asset name for other platforms and `shasum -a 256 -c` on macOS.
Checksums alone do not authenticate a release: verify the signature and workflow
provenance. Homebrew and the website installer are pinned separately and must
only be advanced after that verification; older installer pins do not include
Knowledge.
