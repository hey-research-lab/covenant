# Covenant Compute Knowledge

**Compute 0.5.2 guide.** This client adds guided local setup and a freshness check for saved source. Extraction, snapshot format, the six read-only MCP tools and the pinned remote compiler remain compatible with 0.5.1. One dedicated operator offers limited remote capacity. Its [dedicated-operator paid acceptance](evidence/knowledge/dedicated-acceptance-2026-10-21/report.md) verified one job and an identical-key settled replay with the installed 0.5.1 client; its [finalized stake extension](evidence/knowledge/stake-extension-2026-10-21/report.md) supports admission through 17 November 2026 at 22:10:13 UTC, subject to current capacity and operator health. Two earlier paid pilot jobs separately passed artifact, receipt and interrupted-client recovery checks. These are separate remote results, not a new paid acceptance of 0.5.2. No speed, accuracy or token-savings claim follows from these checks.

See the [original 0.5.0 release evidence](evidence/knowledge/public-release-0.5.0/report.md), [original installer acceptance](evidence/knowledge/public-release-0.5.0/installer-promotion.md), [0.5.2 release](https://github.com/open-covenant/covenant/releases/tag/compute-v0.5.2), [paid pilot report](evidence/knowledge/paid-pilot-2026-10-20/report.md) and [dedicated operator activation](evidence/knowledge/remote-activation-2026-10-20/report.md). Homebrew promotion is separate. Confirm the installed version: guided setup requires 0.5.2, while manual compile and MCP setup work in 0.5.1.

Compute Knowledge turns supported repository files into saved, queryable snapshots. It is part of `covenant-compute`, with no model API key required for local compilation or queries. It does not run the source repository's build scripts, tests or programs.

## Install and compile locally

Install the released binary and its original example repository:

```sh
curl -fsSL https://opencovenant.org/install/compute.sh | sh
export PATH="$HOME/.local/bin:$PATH"

covenant-compute --version
covenant-compute knowledge setup \
  "$HOME/.local/share/covenant-compute/0.5.2/examples/knowledge" \
  --query calculate_total --json
```

The installer verifies the archive against pinned SHA256 checksums. The [0.5.2 release](https://github.com/open-covenant/covenant/releases/tag/compute-v0.5.2) provides signatures and attestations for independent verification. Confirm the reported version before using its versioned example path. If an installation channel still reports 0.5.1, use its manual `knowledge compile` workflow or the signed 0.5.2 archive; do not treat a version check as an automatic update.

`setup` explicitly compiles the directory, searches for the requested symbol, and checks a citation against the captured file's hash, displayed source fragment and line range. Its JSON contains `compile`, `citation_verification`, `freshness`, and `mcp_config`. Copy the entry under `mcp_config.mcpServers` into your MCP client's stdio configuration. It includes the absolute executable and store paths and a fixed project ID; no shell-variable expansion is needed. The command does not modify client settings, create a buyer wallet, start a watcher or make a remote purchase.

Without `--query`, setup chooses a searchable object. An empty capture or a query with no usable citation is reported as `compiled_without_citation`, not a successful first cited query. Inspect diagnostics or select another symbol. Truncated evidence verifies only the displayed fragment against its captured source. `show` and the viewer use the same excerpt limit; inspect the corresponding source at the captured revision for additional context, and check its hash before equating it with saved evidence.

For your own checkout, use `covenant-compute knowledge setup /path/to/repo --query SYMBOL`. You can also keep using `knowledge compile /path/to/repo` when you only want a new snapshot. The emitted `commands` contain the exact arguments for status, the viewer and explicit recompilation.

### Optional source build

Use the `compute-v0.5.2` tag and a Rust toolchain compatible with the workspace:

```sh
cargo build --manifest-path agent-os/Cargo.toml \
  -p covenant-compute-buyer --bin covenant-compute

# Store the index outside the source tree.
export COVENANT_KNOWLEDGE_HOME="$HOME/.covenant/knowledge"
./agent-os/target/debug/covenant-compute knowledge compile ./examples/knowledge
```

When using a source build, add the development binary directory to this shell:

```sh
export PATH="$PWD/agent-os/target/debug:$PATH"
```

The command prints JSON including the project and snapshot IDs. Local source roots become separate projects. The optional `--store /absolute/path` overrides the store for one command. Keep the store outside the compiled directory; a store inside the source is rejected.

In setup output, those IDs are under `compile.project.id` and `compile.snapshot_id`.

Supported extraction covers Rust, Python, JavaScript, TypeScript and Markdown headings. Tree-sitter parsers recover syntax and a bounded set of imports, references and calls. This is not whole-program type checking. Dynamic calls, external targets and ambiguous names can remain unresolved.

Source capture respects Git ignore rules, `.covenantignore`, built-in sensitive-path and generated-directory exclusions, and file/count/byte limits. Symlinks are not followed. Default limits are 20,000 files, 1 MiB per file and 64 MiB total source. Treat the built-in exclusions as a precaution, not a comprehensive secret scanner. Review capture diagnostics before using a repository outside your own machine.

## Search, citations and dependencies

```sh
covenant-compute knowledge status
covenant-compute knowledge query "calculate_total" --project PROJECT_ID --limit 10
covenant-compute knowledge show OBJECT_ID --project PROJECT_ID
covenant-compute knowledge impact OBJECT_ID --project PROJECT_ID \
  --depth 2 --direction both
```

Replace uppercase placeholders with IDs from the preceding JSON. Search is lexical retrieval over admitted names, paths and source text; it does not call a language model. Source evidence includes the saved file, exact excerpt, line range and source hash. Dependencies distinguish resolved targets from unresolved references. The impact command explores bounded graph relationships; it does not prove all runtime callers have been found.

Use `--snapshot SNAPSHOT_ID` on read commands to select historical evidence. Without it, queries use the selected project's latest snapshot. If more than one project exists, pass `--project PROJECT_ID` explicitly.

### Check whether saved source is current

`knowledge status`, MCP `knowledge.status` and the viewer compare the selected saved snapshot with a bounded read of its registered source directory. `freshness.state` is `current`, `stale` or `unavailable`; unavailable never means current. Changes include admitted additions, edits, deletions and the supported capture configuration and repository ignore files. Git metadata is excluded from this freshness check. Ignored content is outside the comparison; the check does not expand macros, execute source or infer runtime changes.

This is an observation at the time of the request, not a continuous watcher. Status does not update the snapshot or parser cache. If source is unavailable, saved citations still work. If it is stale, explicitly recompile and use `knowledge diff` to inspect the update. The viewer labels freshness for the selected snapshot, including historical snapshots.

## Recompile and compare

```sh
covenant-compute knowledge compile ./repo
covenant-compute knowledge compile ./repo --full
covenant-compute knowledge diff OLDER_SNAPSHOT NEWER_SNAPSHOT --project PROJECT_ID
covenant-compute knowledge verify SNAPSHOT_ID --project PROJECT_ID
```

Ordinary compilation can reuse content-addressed parser results. `--full` bypasses the parser cache. Every snapshot captures the admitted source set; removed objects disappear from the current snapshot and remain inspectable through older snapshots. Diff reports additions, removals and changes between saved snapshots.

Verification checks the artifact schema, supported compiler/parser versions, content hashes and extraction against the embedded source. It detects corruption or mismatched extraction; it does not prove semantic correctness, absence of vulnerabilities, authorship or trusted hardware execution.

Parser failures and incomplete extraction remain diagnostics alongside valid records. Inspect diagnostics and language coverage before relying on the graph. A successful command is not a claim of complete language understanding.

## Local viewer

```sh
covenant-compute knowledge serve --project PROJECT_ID
# Optional: --port 8788
```

Open the URL printed by the process. The server binds to `127.0.0.1`, chooses an ephemeral port by default, and requires a random session token in the URL. The viewer provides project coverage, search, source excerpts, dependencies, snapshot comparison and verification. Keep the URL private and stop the process when finished. It is a local, read-only viewer, not a hosted team workspace.

## Read-only MCP

```sh
covenant-compute knowledge mcp --project PROJECT_ID
```

Configure your MCP client's stdio transport to launch the absolute binary path with those arguments. This command does not edit client or agent configuration automatically. The selected project is fixed for the server process. Tools expose:

| Tool | Purpose |
|---|---|
| `knowledge.search` | Find records in a selected snapshot |
| `knowledge.get` | Read a record |
| `knowledge.evidence` | Read its saved source evidence |
| `knowledge.dependencies` | Inspect bounded graph relationships |
| `knowledge.changes` | Compare saved snapshots |
| `knowledge.status` | Inspect project state and coverage |

The MCP interface has no compile, purchase, source-edit or arbitrary shell tool. It is not a project authorization adapter for a shared daemon. Hosted/team knowledge query access and private remote repositories are follow-up work.

## Remote compilation

See the [remote acceptance and deployment guide](compute-knowledge-remote-acceptance.md) for the exact coordinator, image, operator and funded acceptance order.

The [public-source walkthrough](compute-knowledge-remote.md) includes the accepted
compiler digest, a pinned example and the separate dedicated-operator activation
record.

One dedicated operator admits at most one job in flight. One separately authorized job completed at 50,000 micro-USDC; its signed receipt, artifact, local import and finalized settlement verified, and an identical-key settled replay created no additional job or charge. See the [dedicated-operator paid acceptance](evidence/knowledge/dedicated-acceptance-2026-10-21/report.md). A new purchase requires current exact-model capacity, a current quote and an authorized budget. The [finalized stake extension](evidence/knowledge/stake-extension-2026-10-21/report.md) retains 1,000,000 CVNT with no added tokens, fixes stake unlock at 19 November 2026 at 22:10:13 UTC, and moves the 48-hour admission cutoff to 17 November 2026 at 22:10:13 UTC. There is no automatic renewal or uptime guarantee. The two earlier paid pilot jobs used a temporary operator and remain separate evidence; the dedicated job did not test interruption or restart recovery.

The remote path submits the supported source subset from a **public GitHub repository at a full pinned commit**. Before submission, anonymous GitHub checks establish public visibility and compare every selected source file byte-for-byte with that commit. Dirty or unpublished selected source is rejected. Requests are bounded to 1–256 files and 512 KiB of source JSON; results are capped at 3 MiB.

The client strips local capture diagnostics before upload so excluded filenames do not enter the request. The compiler regenerates parser diagnostics for the submitted public subset. Remote coverage therefore describes that subset, not every file on the developer's machine.

Start with a preview:

```sh
covenant-compute knowledge compile ./repo \
  --remote --dry-run \
  --repository https://github.com/OWNER/REPOSITORY \
  --commit FULL_40_CHARACTER_COMMIT \
  --compiler-image-sha256 d011ce03baa42e825e345bd028c4b9af82e210535958c4ce5b49eb8a0503745c
```

The dry run does not purchase execution. Read the resulting request and constraints. It still performs public-repository validation.

Actual purchase additionally requires an authorized coordinator, configured buyer credentials, the existing per-call cap, a persisted daily budget and an idempotency key. `COVENANT_COMPUTE_DAILY_BUDGET_MICRO_USDC` is mandatory. This documentation does not authorize funding or purchase:

```sh
# Only after deployment, budget and purchase approval:
covenant-compute knowledge compile ./repo \
  --remote \
  --repository https://github.com/OWNER/REPOSITORY \
  --commit FULL_40_CHARACTER_COMMIT \
  --compiler-image-sha256 d011ce03baa42e825e345bd028c4b9af82e210535958c4ce5b49eb8a0503745c \
  --price APPROVED_MICRO_USDC \
  --deadline-ms 300000 \
  --idempotency-key UNIQUE_APPROVED_REQUEST_KEY
```

Use `COVENANT_COMPUTE_COORDINATOR_URL` to select the operator's actual coordinator. Preserve an uncertain request's identity for reconciliation; do not retry it under a new key. A returned receipt and a valid portable snapshot must both verify before import. Subsequent search and viewer reads use the imported local store.

### Operator setup

The dedicated executor accepts knowledge compilation, not arbitrary shell commands. Build from the same source revision:

```sh
python3 deploy/knowledge-compiler/build-context.py \
  2> knowledge-build-inputs.json |
  docker build -f deploy/knowledge-compiler/Dockerfile \
    -t YOUR_REGISTRY/covenant-knowledge:YOUR_REVISION -
```

The compiler accepted in the paid pilot and dedicated operator activation is `covenant-knowledge@sha256:d011ce03baa42e825e345bd028c4b9af82e210535958c4ce5b49eb8a0503745c`. It is installed on the operator host; this is not a public registry download. An operator must install the reviewed image and pin its real immutable digest. A locally built replacement has its own digest and requires separate acceptance.

```sh
export COVENANT_COMPUTE_NODE_EXECUTOR=knowledge
export COVENANT_COMPUTE_KNOWLEDGE_IMAGE=YOUR_REGISTRY/covenant-knowledge@sha256:REAL_DIGEST
# Optional; default runtime is documented in the executor:
export COVENANT_COMPUTE_NODE_CONTAINER_RUNTIME=docker
```

The runner uses a nonroot container, read-only filesystem, no network, no host mounts and dropped capabilities. These isolation settings and signed receipts are not TEE attestation. Client release, the earlier paid pilot and current limited capacity have separate evidence. Private repository support, team authorization, new compiler digests and production performance claims need additional acceptance.

## Evidence and release discipline

The [measured local retrieval report](evidence/knowledge/benchmark-2026-10-20/report.md) includes raw results and reproduction commands. On the 9-file example, direct `rg` search was faster and returned fewer UTF-8 bytes in all seven matching cases; structured snapshot evidence is a different output contract. These results do not establish token savings or large-repository performance.

For a concrete agent handoff, use the [context recipe](compute-knowledge-context-recipe.md).

The behavioral research and its limits are recorded in [compute-knowledge-research.md](compute-knowledge-research.md). The publication sequence and claim register are in [compute-knowledge-rollout.md](compute-knowledge-rollout.md). Keep implementation tests, measured examples, deployment and funded acceptance distinct in all product copy.
