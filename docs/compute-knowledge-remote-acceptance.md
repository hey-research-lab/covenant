# Remote Knowledge compilation acceptance

This branch implements a public-source pilot over existing Compute jobs. Local mocked tests exercise signed dispatch, the real source compiler, receipts, import validation, and purchase replay. They do not establish a deployed compiler image, a live operator, funded acceptance, or production performance.

## Contract and boundaries

| Item | Contract |
| --- | --- |
| Job | Nonstreaming `BatchJob`, paid per job |
| Required model | `knowledge-compile-v1:<64 lowercase hex OCI image digest>` |
| Matching | Exact in buyer capacity lookup, coordinator matching, and node admission; neither `any` nor `:latest` aliases qualify |
| Source | One JSON `knowledge_compile` block carrying version, canonical public GitHub repository, full 40-character commit, compiler image digest, source digest, and portable `SourceSet` |
| Source limits | 1–256 selected files; serialized `SourceSet` at most 512 KiB |
| Public verification | Anonymous HTTPS repository visibility check and byte comparison of every selected file against its pinned GitHub commit; no redirects |
| Local metadata | Capture diagnostics are removed before upload because ignored paths can reveal private filenames; parser diagnostics are regenerated |
| Result | One JSON `knowledge_result` block with compiler/source/snapshot digests and a self-contained snapshot containing its source |
| Result limit | Serialized typed result at most 3 MiB; executor stdout ceiling is 4 MiB |
| Transport | Existing signed job and authenticated receipt/output fetch, within existing 8 MiB HTTP frames; no public artifact bucket or raw SQLite transfer |
| Deadline | 1,000–600,000 ms |
| Verification | Existing receipt/output integrity checks, request/result source binding, then full deterministic local recompilation before store import |
| Queries | Remain in the local project-bound CLI, MCP, and viewer; there is no shared daemon project authorization adapter |

The node receives source as JSON on stdin. It runs only the pinned compiler entrypoint in a nonroot OCI container with no network, host mounts, or writable root filesystem; capabilities are dropped. Memory, CPU, process count, time, and output are bounded. Timeout, output failure, and cancellation remove the named container. Startup removes orphan containers bearing that operator's knowledge label. Source content never becomes a shell command.

The image digest identifies the requested compiler. Signed receipts bind the observed job and output. Neither constitutes TEE attestation or proof of semantic correctness. Because the buyer independently recompiles the returned snapshot, this preview makes no speedup claim.

Use `--commit` at the repository root to prepare release acceptance evidence. The selected commit is resolved as a commit, and source bytes are read from its tree without checkout hooks or filters. Working-directory `git.dirty` metadata has a narrower meaning: admitted raw source differs from HEAD, including additions and conservatively counted missing eligible tracked files. It is not `git status` and does not inspect index state, filter transformations, exclusions, or submodule internals. Git metadata reads disable lazy fetching and all network transports.

## Deployment order

1. Build the coordinator, node, buyer, and compiler from the same reviewed revision. Run the focused protocol, node, and buyer knowledge tests and the core library tests. Preserve the revision and test results.
2. Update the coordinator to the exact model matching and typed input validation implementation before advertising knowledge supply. The new model uses the existing `BatchJob` wire kind; older generic operators remain separate supply.
3. Build and install the real compiler image. Record the source revision, build logs, image manifest digest, platform, and binary version. A local tag alone is not sufficient: the executor requires an installed `name@sha256:digest` reference and never pulls on a buyer's deadline.
4. Exercise the actual image with a saved public fixture. Compare its snapshot with the local compiler and run import verification. Verify no network/mount access, nonroot identity, output limit failure, timeout removal, cancellation removal, and startup orphan cleanup using a real OCI runtime.
5. Start an updated operator with the installed digest. Verify health and that capacity advertises only the exact compiler model for `batch_job`, at the intended per-job ask. No wildcard or alternate model may be attached to this executor.
6. Use an updated buyer for an anonymous public-source dry run. Confirm the quote, source digest, pinned revision, limits, and exact matching operator before scheduling a funded test.
7. Perform one explicitly authorized funded acceptance purchase with the existing funded identity, persisted budget, and a new idempotency key. Save its signed job, receipt, output digest, imported snapshot ID, and accounting evidence. Repeat the identical command/key and prove the same job is recovered without a second execution or debit.

Build from the repository root:

```sh
docker build -f deploy/knowledge-compiler/Dockerfile \
  -t YOUR_REGISTRY/covenant-knowledge:REVIEWED_REVISION .
```

After obtaining and installing its real digest, configure the existing operator home and coordinator identity as usual, plus:

```sh
export COVENANT_COMPUTE_NODE_EXECUTOR=knowledge
export COVENANT_COMPUTE_NODE_JOB_KINDS=batch_job
export COVENANT_COMPUTE_KNOWLEDGE_IMAGE=YOUR_REGISTRY/covenant-knowledge@sha256:REAL_DIGEST
export COVENANT_COMPUTE_NODE_CONTAINER_RUNTIME=docker
export COVENANT_COMPUTE_NODE_PRICE_UNIT=per_job
export COVENANT_COMPUTE_NODE_PRICE_MICRO_USDC=APPROVED_ASK
# Omit COVENANT_COMPUTE_NODE_MODELS to derive the exact model from the image.
covenant-compute-node status
covenant-compute-node
```

These are deployment instructions, not evidence that deployment has occurred.

## Buyer gates and recovery

The normal buyer context supplies coordinator URL, identity, and per-call ceiling. Remote purchases additionally require `COVENANT_COMPUTE_DAILY_BUDGET_MICRO_USDC` and its durable budget journal. The value must agree with the persisted limit. Keep the existing `COVENANT_COMPUTE_BUDGET_HOME` namespace; do not replace it to bypass a hold or limit mismatch. Changing a persisted allowance uses the established budget command and its expected-limit guard.

An explicit `--price` must satisfy the exact operator's current ask and remain below `COVENANT_COMPUTE_MAX_PRICE_MICRO_USDC`. A supplied price does not bypass capacity matching or either budget.

```sh
covenant-compute knowledge compile ./PUBLIC_REPO \
  --commit FULL_COMMIT_SHA \
  --remote \
  --repository https://github.com/OWNER/REPOSITORY \
  --compiler-image-sha256 REAL_DIGEST \
  --deadline-ms 300000 \
  --dry-run \
  --json
```

After the independent funding and budget gates are satisfied, replace `--dry-run` with `--idempotency-key APPROVED_UNIQUE_KEY`; add an approved `--price` if required. The dry run reads public GitHub content and network capacity. It does not sign or submit a purchase, reserve a budget, or create a purchase journal.

The paid helper journals the original signed envelope before submission. The key is scoped by buyer identity and coordinator endpoint. Reusing it with different input, digest, price, or deadline is refused. A settled retry fetches the original receipt/output with authenticated reads. An unfinished retry retains the original envelope and durable budget requirement. Failed, rejected, and ambiguous outcomes do not silently create another job or reprice the request.

Keep the original source commit, compiler digest, deadline, key, purchase journal, and budget state for reconciliation. Do not switch keys or delete journals to make an uncertain request proceed. A paid artifact that fails semantic recompilation is recorded as the original settled purchase and refused for import; it does not trigger another purchase.

Release claims must remain limited to implemented and tested behavior until the real image, operator, funded receipt, replay, and accounting evidence above are recorded. Private repositories, shared project authorization, SQL lineage, hosted indexing, and performance claims need separate acceptance work.

## Reproduce the local test demonstration

The HTTP integration test can export a fresh evidence file for a clearly labeled demonstration. It runs an actual loopback coordinator and node, signs ephemeral fixture jobs and receipts, invokes the actual source compiler, verifies the result, and checks purchase replay. Funding and payout are mock implementations; the GitHub verifier and OCI invocation are substituted, and the production daily-budget guard is disabled only in this isolated fixture.

From `agent-os`:

```sh
KNOWLEDGE_EVIDENCE_DIR="$(mktemp -d "${TMPDIR:-/tmp}/knowledge-evidence.XXXXXX")"
COVENANT_KNOWLEDGE_TEST_EVIDENCE_PATH="$KNOWLEDGE_EVIDENCE_DIR/local-remote-evidence.json" \
  CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 CARGO_INCREMENTAL=0 \
  cargo test --locked -p covenant-compute-buyer --lib \
  knowledge_remote::tests::knowledge_remote_http_dry_run_purchase_replay_and_conflict \
  -- --exact
```

The export refuses to overwrite an existing file and contains no signing keys. Its schema explicitly records the substitutions and marks funded acceptance, real OCI execution, and actual public GitHub verification false. It includes the quote, typed request, signed fixture job and receipt, source snapshot, same-job replay identifier, one-execution count, and rejection checks.

A 30–45 second local-test walkthrough can show the submitted digest and quote, the fixture receipt and verified source snapshot, then the identical replay job ID and one-execution counter. Keep **LOCAL TEST — MOCK FUNDING — NOT DEPLOYED** visible throughout. This demonstration supports the local integration claim; replace it with independently recorded real-OCI and funded acceptance evidence before announcing the remote release.
