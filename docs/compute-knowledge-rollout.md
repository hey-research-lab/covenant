# Compute Knowledge: brand and rollout

Prepared 20 October 2026. **Drafts only. Nothing in this plan authorizes posting, scheduling, release publication, deployment or spending.** The current status is a local source implementation preview, not publicly released or deployed.

## Positioning

Use **Covenant Compute Knowledge** on first mention, then **Compute Knowledge**. It is a capability inside Covenant Compute, not a separate company, token or product ecosystem.

**Promise:** a searchable view of code with source evidence developers and agents can inspect.

**Primary audience:** developers who use coding agents in unfamiliar or changing repositories. Their immediate problem is finding the right symbol, checking the actual excerpt, understanding references and seeing what changed.

**Message:** “Codebase knowledge your agents can inspect.” Follow with a concrete workflow: compile locally, inspect cited results, compare revisions. Explain remote compilation only once its own evidence gate clears.

**Visual:** source folio → linked symbols → exact-source lens, connected by thin glass planes and an ordered violet–blue–mint field on black. Archivo Expanded 600 headlines, JetBrains Mono 500 labels, official Covenant mark. No invented performance numbers, LIVE badge, token-pricing story or visual suggestion that a private repository was uploaded.

## Publication sequence

Publish one feature at a time, in this order. A two-to-three-day gap is a planning suggestion after each gate clears, not an automatic schedule. If a gate is late, hold the post and keep development updates factual. Use replies to answer concrete workflow questions; do not manufacture engagement or imply endorsements.

Each draft below is a complete standalone X post under 280 characters. Post 1 is a development update. Posts 2–5 are **LOCAL RELEASE drafts**, held until a package is published and the linked guide/example works from a fresh install. Post 6 is a **REMOTE RELEASE draft**, held until actual deployment and funded acceptance. Post 7 waits for published measurements. Canonical links are proposed publication targets: the publisher must confirm HTTP 200 and the promised content before posting. None of the release drafts describes the current branch status.

### 1. Development

> We’re building Knowledge inside Covenant Compute. Codebase knowledge your agents can inspect: compile locally, follow source evidence and compare revisions. This is an implementation preview. We’ll share the working features as each release is ready.

**Gate:** original implementation exists; status and scope reviewed. **Asset:** 1080p master artwork with LOCAL PREVIEW and implementation disclaimer. **CTA:** follow the development sequence. **Success signal:** relevant developer questions and preview requests, recorded as actual counts.

### 2. Local compile

> Knowledge is available locally in Covenant Compute. Compile Rust, Python, JS/TS and Markdown, then search the saved source without a model key. Start with our original example and local guide: https://opencovenant.org/compute-knowledge

**Gate:** published installable local release, deployed feature page and guide, fresh-install acceptance; local build and language/exclusion/parser tests pass; demo uses actual saved records. **Asset:** local demo overview/search segment. **CTA:** inspect the example and local guide once those are published. **Success signal:** completed local compilations reported with tool version, not inferred from link clicks.

### 3. Citations + MCP

> Give your coding agent source it can inspect. Knowledge now pairs results with exact excerpts, paths and line ranges through six read-only MCP tools in Covenant Compute. Try the local workflow: https://opencovenant.org/compute-knowledge

**Gate:** local release/package and feature-page link verified; citations across supported input types and MCP scope/rejection tests pass. **Asset:** exact-source screenshot and short MCP transcript from a real local run. **CTA:** check one cited result against its snapshot. **Success signal:** citation checks or reported mismatches.

### 4. Dependencies

> Trace what connects to a symbol in Covenant Compute. Knowledge shows observed dependencies and unresolved references, so the limits stay visible. Follow a real trace from our example: https://opencovenant.org/compute-knowledge

**Gate:** local release/package and feature-page link verified; bounded trace, unresolved reference and project scoping tests pass. **Asset:** actual dependency screenshot; include unresolved rows. **CTA:** examine one dependency trace. **Success signal:** useful source paths discovered and concrete missing-resolution reports.

### 5. Updates + diff

> Code changed? Compare the evidence. Knowledge in Covenant Compute saves revision snapshots, shows source additions, edits and removals, and keeps earlier evidence inspectable. Try the before-and-after: https://opencovenant.org/compute-knowledge

**Gate:** local release/package and feature-page link verified; edit/delete/full-vs-cached/history tests pass. **Asset:** actual two-snapshot diff, with revision IDs retained in supporting evidence. **CTA:** compare a small local edit. **Success signal:** verified reproducible diffs, including deletions.

### 6. Remote

> Remote Knowledge compilation is available on Covenant Compute for pinned public GitHub revisions. Choose the compiler image; the buyer verifies the receipt and snapshot before local import. Guide and acceptance evidence: https://opencovenant.org/compute-knowledge

**Gate:** real immutable compiler image; updated coordinator and operator deployment; explicitly authorized funded acceptance; independently verified receipt and imported snapshot; replay/accounting acceptance; working guide and acceptance evidence at the canonical page. Signed-envelope, public-source matching, budget and failure tests must also pass. **Asset:** actual accepted remote run; the prepared LOCAL TEST storyboard is not a substitute. **CTA:** follow the public-source guide and inspect acceptance evidence. **Success signal:** reproduced accepted jobs with receipts, not impressions.

Optional development update while the gate remains closed: “We’re testing remote Knowledge compilation for pinned public source. The client verifies receipts and snapshots before import. Deployment and funded acceptance are still ahead.”


### 7. Measured results

> We published Knowledge’s local measurements, including where direct search wins. Inputs, commands and results are reproducible. This small fixture does not establish production performance or token savings. Report: https://opencovenant.org/compute-knowledge

**Current evidence:** [actual local report](evidence/knowledge/benchmark-2026-10-20/report.md), with direct-search wins disclosed. **Gate:** publish the actual measurement report with machine/tool versions, corpus definition and hashes, repeated cold/full and cached compile timings, query latency and result sizes. Link the actual report only when available. **Asset:** simple measured table using only recorded values. **CTA:** reproduce and report context. **Success signal:** independent reproductions, with disagreements retained.

## Claim register

“Implemented” always means this branch unless a release artifact and deployment record are linked. Test pass status belongs in the final QA report, not inferred from the presence of a test file.

| Claim | Implementation / evidence location | Required validation | Allowed wording now |
|---|---|---|---|
| Part of Compute | `agent-os/crates/covenant-compute-buyer/src/knowledge_cli.rs` | Buyer build and CLI integration | “inside Covenant Compute”; implementation preview |
| Local compilation without models or purchase | `covenant-knowledge/src/source.rs`, `parser.rs`; CLI local dispatch | Local run without credentials; no paid transport | “local commands require no model key or Compute purchase” |
| Five language/input names | `covenant-knowledge/src/parser.rs`, `examples/knowledge` | Per-language extraction + citation assertions | Rust/Python/JavaScript/TypeScript/Markdown; bounded extraction |
| Exact source evidence | `covenant-knowledge/src/store.rs` and core tests | Saved slice, hash, range and historical snapshot tests | Saved excerpt, path and line range; no correctness guarantee |
| Read-only MCP | `knowledge_cli.rs`, buyer tests | Six tools; project, argument and mutation rejection | Six read-only tools bound to one local project |
| Dependency tracing | Core resolver/query and fixtures | Resolved/unresolved, depth/ambiguity examples | “observed relationships”; no complete call-graph claim |
| Updates and history | Store/cache/diff and core tests | Edit/delete parity and corruption tests | Recompile, compare saved snapshots, retain historical evidence |
| Local viewer | `knowledge_viewer.rs`, viewer assets, browser QA | Loopback/token/auth/error/size tests and responsive QA | Local read-only viewer; not a hosted team workspace |
| Remote candidate | `knowledge_remote.rs`, protocol/node knowledge modules | Source matching, cap/budget, receipt, tamper/container failure tests | “remote implementation preview”; no availability claim |
| Container isolation | Node runner and `deploy/knowledge-compiler` | Runtime args + cleanup tests; actual image gate separate | Candidate runner restrictions; no TEE claim |
| Local measurements | [local report](evidence/knowledge/benchmark-2026-10-20/report.md) and raw results | Raw repeated results + reproducible commands | Only measured fixture numbers with corpus/machine caveat |
| Packaged/released | No published artifact in this task | Signed/checksummed release and fresh install | **Not released** |
| Deployed/paid accepted | No deployment or funded acceptance in this task | Explicit user authorization + independent receipt evidence | **Not deployed / not funded acceptance** |
| Token/cost savings, comparative superiority | No controlled experiment | Matched task quality and token/cost experiment | **Do not claim** |
| Private/team remote access | No shared daemon knowledge-query authorization adapter | Threat model and project authorization acceptance | **Follow-up scope** |

## Assets and demo

Local output directory: `output/compute-knowledge-launch/` (kept outside the commit, not automatically published).

- `visuals/compute-knowledge-1920.png`: 1920×1080 master.
- `visuals/compute-knowledge-4k.png`: 3840×2160 composition.
- `render-manifest.json`: actual dimensions, typography and source image provenance. The generated raster is 1672×941, scaled within the composition; type and mark are rendered at output resolution.
- `render.mjs`: exact-font compositor.
- `demo/local-preview.mp4` and `.webm`: completed 35.24-second local viewer capture.
- `demo/01-overview.png` through `06-verify.png`: real viewer frames.
- `demo/evidence.json`: recording conditions, scope and errors.
- `remote-demo/remote-local-test.mp4`: completed 34.28-second walkthrough of actual local HTTP integration-test evidence, clearly labeled mock payment/verifier, no OCI execution and no funded acceptance.
- `remote-demo-storyboard.md`: held release capture plan to record again after real acceptance.
- `qa/landing-checks.json`: responsive/browser checks.

The demo uses an original small billing example, two real compiled snapshots and real query responses. A constant **LOCAL IMPLEMENTATION PREVIEW · ORIGINAL EXAMPLE** label separates the recording from deployment evidence. Chapter labels are editing overlays; source data is unmodified.

Suggested 38-second cut: 0–5s snapshot/coverage; 5–12s search; 12–18s exact excerpt; 18–25s relationships including unresolved entries; 25–32s actual diff; 32–38s stored-evidence verification. It is a workflow demonstration, not a performance test.

## Measurement plan

Use one campaign tag per post, such as `utm_campaign=compute_knowledge` and
`utm_content=post_2_local`. Preserve the 48–72-hour spacing only after each
feature's gate clears. Record actual conversions separately from impressions.
The local CLI currently sends no analytics; do not infer successful use from a
download or introduce background uploads to obtain these numbers.

| Funnel step | Evidence to collect during the opt-in builder pilot |
|---|---|
| Example install | Download or source-build confirmation with platform and binary hash |
| First successful compile | Voluntarily shared success/coverage summary, without repository content or private paths |
| First cited query | Confirmation that one citation opened and matched its saved source; record mismatches |
| Repeat use | A second successful session or recompile on a later day from the same consenting pilot participant |
| Completed Compute workflow | Verified task outcome and its receipt identifier, counted separately from a quote or preview |

Keep weekly counts with denominators: install → compile, compile → cited query,
and cited query → repeat use. Add a short failure reason for each drop-off.
Track pricing and performance only against the dated claim register, and retain
the corpus and binary hashes for any measured comparison.

## Release checklist for the publisher

Before each post, confirm its evidence gate, current release state, asset label, exact links and final character count. No draft includes an endorsement, performance comparison or promise of savings. Before changing “preview” to “available,” record the release package, deployed feature URL and any separate remote acceptance evidence. Keep the seven-stage sequence; do not bundle unaccepted remote claims into the local announcement.
