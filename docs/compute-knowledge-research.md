# Compute Knowledge: bounded behavioral research

Research date: **20 October 2026**. This work observes public behavior and original examples to inform an independently implemented Covenant feature. No Ekos source, UI copy, fixtures or implementation algorithms are incorporated into the Covenant compiler.

## Tools and provenance

REA **6.7.0** was pinned for passive browser inspection. The public `https://ekos.dev/` capture was the marketing homepage, with documentation links and one small inline script. It did not establish application behavior. REA's local browser contract is documented in the [6.7.0 source](https://github.com/morluto/rea/blob/rea-agents-6.7.0/docs/browser-observation.md).

The official [Ekos v1.1.0 release](https://github.com/alexeyban/EKOS/releases/tag/v1.1.0) was downloaded from its published release assets. Its Apple Silicon archive SHA-256 matched both the release checksum file and GitHub's asset digest:

```text
d5bbac5578bb78c9e55e8d69fdc98788c8582fc4c13d0f1e42611bcd7400a153
```

Execution used an isolated `EKOS_HOME`, original small Rust/Python/JavaScript/TypeScript/Markdown fixtures and a disposable browser profile. No model credentials, global package installation, PATH change, agent configuration, login service, funded operation or external account was used. The local console and temporary browser were stopped afterward.

Research evidence is retained outside implementation source at:

```text
output/compute-knowledge-research/ (retained in the original local checkout)
  README.md
  REPRODUCE.md
  release-provenance.json
  behavior-evidence.json
  original-fixtures.json
  reproduce.py
  logs/
  reproduction-verification/results.json
  reproduction-verification/checks.json
```

`reproduce.py` was rerun: the 40-command verification completed and the recorded assertions reproduced. These local artifacts are not a public release link. The retained vendor executable/archive remain research artifacts and are not included in Covenant's distribution.

The repository includes a [sanitized observation manifest](evidence/knowledge/ekos-observation-manifest.json) with findings, command IDs, recorded arguments, exit codes, output hashes, release provenance and reproduction checks. It contains no vendor source, executable or artwork. `$RESEARCH` replaces the personal research directory. Some original logs record subcommand arguments with the selected project supplied by the research wrapper; the reproduction uses `EKOS_HOME=$RESEARCH/ekos-home ekos --project behavior …` (or `--project excluded` for the exclusion run).

## Observed behavior and independent requirements

| Area | Ekos v1.1.0 observation on the original fixture | Covenant requirement |
|---|---|---|
| Ingestion/search | All requested language families produced searchable objects. Baseline graph returned 26 objects and 12 edges. | Test admitted file/symbol/document records per language. |
| Citations | Rust and Python detail included paths, line ranges and excerpts. The observed TypeScript `queueTask` detail lacked an Evidence section/source range. | Every admitted result should expose source-bound evidence. |
| Relationships | Python and TypeScript imports produced file-to-import-name `DependsOn` edges. Original cross-file function calls did not produce `Calls` edges. | Distinguish resolved targets, unresolved names and bounded call analysis. |
| Exclusions | `.env` and `node_modules` were absent. A `.gitignore`-listed Python file remained searchable. Explicit security globs excluded it in a fresh project. | Apply Git and explicit exclusions before parsing; test synthetic canaries. |
| Coverage | JS/TS coverage reported `ZERO` despite searchable `JsSymbol` objects; exit status was 0 with `ok:false`. | Derive coverage from admitted records; preserve partial/failure diagnostics. |
| Updates/history | Edited Rust retained its object ID and changed excerpt. Audit showed two writes with run IDs, timestamps and artifact IDs. | Immutable revision snapshots and inspectable changes. Covenant uses snapshot-scoped IDs and path-based file diffs; it does not promise stable symbol IDs across span changes. |
| Deletion | A removed JS file and `statusLabel` remained searchable after full pipeline execution while freshness reported `fresh`. | Removed records must leave current queries while older snapshots retain history. |
| Parse failure | Malformed examples emitted `RUST003`, `PYTHON003`, `JS003`; valid records remained, pipeline exit status 0. | File diagnostics and valid partial results must coexist visibly. |
| Console | REA inspected workspace/dashboard pages, counts, warnings, freshness and navigation. | Use inspection flows as a functional reference, with original Covenant UI. |

These are observations of one pinned version, configuration and deliberately small corpus. They do not establish general defects, complete parity, comparative accuracy, recall, speed or token savings. They must not become attack copy or blanket competitor claims.

## Implementation and acceptance mapping

Command IDs below resolve in the [observation manifest](evidence/knowledge/ekos-observation-manifest.json). Named Covenant tests are independently authored and use Covenant's own fixtures. Test implementation and the final acceptance report establish the current branch behavior; upstream observations are requirements input, not proof that Covenant passed.

Core test names link to [the actual core test file](../agent-os/crates/covenant-knowledge/tests/knowledge.rs). CLI/MCP/viewer tests live in [the buyer integration test file](../agent-os/crates/covenant-compute-buyer/tests/knowledge_cli.rs). [Viewer acceptance](evidence/knowledge/viewer-acceptance.md) records the browser checks. No broad parity claim is made.

| Behavior | Upstream evidence / command IDs | Actual Covenant implementation | Acceptance test |
|---|---|---|---|
| Ingestion and searchable records | `initial-pipeline:03`–`:07`; `query-baseline:01`–`:03` | `covenant-knowledge/src/parser.rs` extracts typed symbols and sections; `store.rs` indexes names, paths and source. | `all_languages_have_resolvable_exact_source_evidence`; `knowledge_commands_compile_query_diff_and_verify_without_identity` |
| Source citations | `object-evidence-baseline:01`, `:02`, `:03` | `store.rs` returns source slices from immutable snapshot bytes, ranges and hashes. | `all_languages_have_resolvable_exact_source_evidence`; `committed_and_dirty_bytes_have_distinct_citations` |
| Markdown content | `object-evidence-baseline:07` graph includes document sections | `parser.rs` captures section bodies and respects fenced code markers. | `markdown_sections_include_body_and_respect_fence_markers` |
| Imports/references/calls | `object-evidence-baseline:04`, `:07` | `parser.rs` records site provenance and resolves bounded targets and retains unresolved/candidate references. | `expression_references_exports_and_multiple_imports_are_precise`; `missing_rust_import_is_not_resolved_to_its_parent`; `local_candidate_calls_are_not_runtime_proofs` |
| Exclusions | `query-baseline:04`–`:06`; `explicit-exclusions:08`, `:09` | `source.rs` handles Git/custom ignores, sensitive paths, symlinks, binary content and limits before extraction. | `ignored_sensitive_linked_and_binary_files_are_reported`; `invalid_paths_duplicate_paths_and_resource_overruns_refuse`; `committed_capture_refuses_subdirectory_scope_expansion` |
| Coverage and parse failure | `initial-pipeline:08`; `update-failure-history:04`, `:09`, `:12`, `:13` | `parser.rs` emits parse diagnostics; admitted objects and unresolved relationships feed coverage. | `malformed_syntax_and_dynamic_calls_are_explicit`; `all_languages_have_resolvable_exact_source_evidence` |
| Recompile/history | `update-failure-history:01`, `:10`, `:14`; `history-diff-cleanup:01` | `store.rs` keeps immutable snapshots, content-addressed parse reuse and path-based diff; symbol IDs can change with spans. | `same_source_and_incremental_rebuild_are_canonical`; `snapshot_and_project_isolation_survive_reopen`; `storage_upgrade_preserves_snapshots_and_failed_commit_is_atomic` |
| Deletion/current results | `update-failure-history:08`, `:11`, `:15` | New source sets exclude deleted paths; historical snapshots retain earlier records. | `deleted_and_renamed_files_disappear_only_from_new_snapshots` |
| Console/viewer | REA workspace/dashboard evidence IDs below | `knowledge_viewer.rs` and `knowledge_viewer/` implement an original loopback UI with session-bound reads. | `knowledge_viewer_requires_private_session_and_exact_origin_without_proxy_headers`; responsive [viewer acceptance](evidence/knowledge/viewer-acceptance.md) |
| Agent reads | Not exercised in upstream research | `knowledge_cli.rs` exposes six project-bound read-only MCP tools. | `knowledge_mcp_is_project_bound_readonly_and_bounds_frames` |
| Artifact integrity | No equivalent upstream acceptance tested | `lib.rs` validates hashes, schema and deterministic extraction before import; `store.rs` protects snapshot/event history. | `import_rejects_hash_consistent_fabricated_facts`; `index_reconstruction_and_append_only_storage`; `corrupted_valid_json_cache_falls_back_to_source`; `failed_capture_does_not_publish_partial_snapshot` |
| Performance | Not measured against Ekos | Separate original small-corpus measurement; output contracts differ from direct search. | [Reproducible local report](evidence/knowledge/benchmark-2026-10-20/report.md): direct `rg` was faster and returned fewer UTF-8 bytes in all seven matching cases. No efficiency or token-saving claim. |

## Browser evidence

REA public homepage inspection:

```text
inspect: ev_51d5dc4dff7a418c0b18713f8e4fa127b8118ca7b63e213c0eb4934a956e10ab
bundle:  ev_153d9e20bbc50579227d4d693e3e3715a6a45c2dd496a27dc4848110ccf88d68
```

REA isolated local console inspection:

```text
workspaces: ev_e80dadeb43bd3d5be7caec89e6b6a2db97ff8497c541941022c400b179fe9387
dashboard:  ev_5ba72f31f61246760bd5f39cc72da7f62d556530daf8e16e2d0827a3d992ed23
```

The local bundle analysis exceeded a 60-second caller budget; no successful local bundle analysis is claimed. Visible tabs for graph, history, commands, configuration or other capabilities were not treated as evidence that those entire features worked.

Not exercised: model-based QA, vector retrieval, scale benchmarks, private/team authorization, remote execution, external integrations or paid service acceptance.

## Reproduction boundary

Follow the saved `REPRODUCE.md` in the research folder to run the official binary against the original fixtures under a new isolated research home. Use the recorded checksums and version, keep setup changes inside that folder, and preserve stdout/stderr/exit status for each command. Avoid agent configuration or automatic background-service setup. Covenant acceptance uses its own fixtures and tests under `agent-os/crates/covenant-knowledge` and `examples/knowledge`, with separate evidence; it must not import the vendor runtime.
