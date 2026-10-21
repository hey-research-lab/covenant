# Covenant Compute 0.5.2

This client release makes it easier to connect a repository to a local agent and
see when saved source is out of date.

- `covenant-compute knowledge setup <path> --query SYMBOL` explicitly compiles a
  local repository, checks a source citation, and emits project-bound stdio MCP
  configuration with absolute executable and store paths. It leaves client
  settings unchanged and reports empty/no-match captures truthfully.
- CLI status, MCP status and the loopback viewer report current, stale or
  unavailable source. Status reads never recompile or replace saved evidence.
- Packaged installation acceptance executes the emitted MCP configuration,
  checks exact citations, and exercises source edit/restoration transitions.

Local Knowledge needs no wallet, daemon, model key or Compute payment. A model
connected through an external MCP client has its own usage and data handling.
Extraction remains syntactic and bounded; unresolved relationships are not
proof that a dependency does not exist. Freshness covers admitted source and
the supported capture policy, not ignored content or runtime behavior.

The snapshot format and exact remote compiler capability are unchanged. Existing
remote acceptance was performed with earlier installed clients and remains
separate evidence. This release adds no paid acceptance job, operator deployment,
token purchase or stake change. Remote capacity and prices must be checked at use.

Four native archives cover macOS ARM64/x86-64 and Linux ARM64/x86-64 musl. Each
includes all four Compute executables, examples, guides, licenses and offline
acceptance tools. Use the signed checksum file and GitHub attestations to verify
the downloaded archive; website and Homebrew promotion are separate gates.

The Sigstore certificate identity for this release is:

```
https://github.com/open-covenant/covenant/.github/workflows/compute-release.yml@refs/tags/compute-v0.5.2
```

The OIDC issuer is `https://token.actions.githubusercontent.com`.
