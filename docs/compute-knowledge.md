# Covenant Compute Knowledge

**Status, 20 October 2026: source implementation preview.** Local compiler, CLI, MCP and viewer are implemented. This is not a published binary release or deployment announcement. The remote compiler path remains gated on an operator image, deployment and separately authorized acceptance. No speed, accuracy, token savings or production availability claim follows from implementation tests.

Compute Knowledge turns supported repository files into saved, queryable snapshots. It is part of `covenant-compute`, with no model API key required for local compilation or queries. It does not run the source repository's build scripts, tests or programs.

## Build and compile locally

Use this implementation branch and a Rust toolchain compatible with the workspace:

```sh
cargo build --manifest-path agent-os/Cargo.toml \
  -p covenant-compute-buyer --bin covenant-compute

# Store the index outside the source tree.
export COVENANT_KNOWLEDGE_HOME="$HOME/.covenant/knowledge"
./agent-os/target/debug/covenant-compute knowledge compile ./examples/knowledge
```

To use the shorter commands below, add the development binary directory to this shell:

```sh
export PATH="$PWD/agent-os/target/debug:$PATH"
```

The command prints JSON including the project and snapshot IDs. Local source roots become separate projects. The optional `--store /absolute/path` overrides the store for one command. Keep the store outside the compiled directory; a store inside the source is rejected.

Supported extraction covers Rust, Python, JavaScript, TypeScript and Markdown headings. Tree-sitter parsers recover syntax and a bounded set of imports, references and calls. This is not whole-program type checking. Dynamic calls, external targets and ambiguous names can remain unresolved.

Source capture respects Git ignore rules, `.covenantignore`, built-in sensitive-path and generated-directory exclusions, and file/count/byte limits. Symlinks are not followed. Default limits are 20,000 files, 1 MiB per file and 64 MiB total source. Treat the built-in exclusions as a precaution, not a comprehensive secret scanner. Review capture diagnostics before using a repository outside your own machine.

## Search, citations and dependencies

```sh
covenant-compute knowledge status
covenant-compute knowledge query "schedule_task" --project PROJECT_ID --limit 10
covenant-compute knowledge show OBJECT_ID --project PROJECT_ID
covenant-compute knowledge impact OBJECT_ID --project PROJECT_ID \
  --depth 2 --direction both
```

Replace uppercase placeholders with IDs from the preceding JSON. Search is lexical retrieval over admitted names, paths and source text; it does not call a language model. Source evidence includes the saved file, exact excerpt, line range and source hash. Dependencies distinguish resolved targets from unresolved references. The impact command explores bounded graph relationships; it does not prove all runtime callers have been found.

Use `--snapshot SNAPSHOT_ID` on read commands to select historical evidence. Without it, queries use the selected project's latest snapshot. If more than one project exists, pass `--project PROJECT_ID` explicitly.

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

## Remote compilation preview

See the [remote acceptance and deployment guide](compute-knowledge-remote-acceptance.md) for the exact coordinator, image, operator and funded acceptance order.

The remote path submits the supported source subset from a **public GitHub repository at a full pinned commit**. Before submission, anonymous GitHub checks establish public visibility and compare every selected source file byte-for-byte with that commit. Dirty or unpublished selected source is rejected. Requests are bounded to 1–256 files and 512 KiB of source JSON; results are capped at 3 MiB.

The client strips local capture diagnostics before upload so excluded filenames do not enter the request. The compiler regenerates parser diagnostics for the submitted public subset. Remote coverage therefore describes that subset, not every file on the developer's machine.

Start with a preview:

```sh
covenant-compute knowledge compile ./repo \
  --remote --dry-run \
  --repository https://github.com/OWNER/REPOSITORY \
  --commit FULL_40_CHARACTER_COMMIT \
  --compiler-image-sha256 FULL_64_CHARACTER_IMAGE_DIGEST
```

The dry run does not purchase execution. Read the resulting request and constraints. It still performs public-repository validation.

Actual purchase additionally requires an authorized coordinator, configured buyer credentials, the existing per-call cap, a persisted daily budget and an idempotency key. `COVENANT_COMPUTE_DAILY_BUDGET_MICRO_USDC` is mandatory. This documentation does not authorize funding or purchase:

```sh
# Only after deployment, budget and purchase approval:
covenant-compute knowledge compile ./repo \
  --remote \
  --repository https://github.com/OWNER/REPOSITORY \
  --commit FULL_40_CHARACTER_COMMIT \
  --compiler-image-sha256 FULL_64_CHARACTER_IMAGE_DIGEST \
  --price APPROVED_MICRO_USDC \
  --deadline-ms 300000 \
  --idempotency-key UNIQUE_APPROVED_REQUEST_KEY
```

Use `COVENANT_COMPUTE_COORDINATOR_URL` to select the operator's actual coordinator. Preserve an uncertain request's identity for reconciliation; do not retry it under a new key. A returned receipt and a valid portable snapshot must both verify before import. Subsequent search and viewer reads use the imported local store.

### Operator setup

The dedicated executor accepts knowledge compilation, not arbitrary shell commands. Build from the same source revision:

```sh
docker build -f deploy/knowledge-compiler/Dockerfile \
  -t YOUR_REGISTRY/covenant-knowledge:YOUR_REVISION .
```

An operator must inspect, publish or install the resulting image and pin its real immutable digest. No digest is supplied here because no production compiler image has been built or deployed as part of this preview.

```sh
export COVENANT_COMPUTE_NODE_EXECUTOR=knowledge
export COVENANT_COMPUTE_KNOWLEDGE_IMAGE=YOUR_REGISTRY/covenant-knowledge@sha256:REAL_DIGEST
# Optional; default runtime is documented in the executor:
export COVENANT_COMPUTE_NODE_CONTAINER_RUNTIME=docker
```

The runner uses a nonroot container, read-only filesystem, no network, no host mounts and dropped capabilities. These isolation settings and signed receipts are not TEE attestation. Public or paid acceptance, image deployment, release packaging, private repository support, team authorization and production benchmarks remain separate gates.

## Evidence and release discipline

The [measured local retrieval report](evidence/knowledge/benchmark-2026-10-20/report.md) includes raw results and reproduction commands. On the 9-file example, direct `rg` search was faster and returned fewer UTF-8 bytes in all seven matching cases; structured snapshot evidence is a different output contract. These results do not establish token savings or large-repository performance.

For a concrete agent handoff, use the [context recipe](compute-knowledge-context-recipe.md).

The behavioral research and its limits are recorded in [compute-knowledge-research.md](compute-knowledge-research.md). The publication sequence and claim register are in [compute-knowledge-rollout.md](compute-knowledge-rollout.md). Keep implementation tests, measured examples, deployment and funded acceptance distinct in all product copy.
