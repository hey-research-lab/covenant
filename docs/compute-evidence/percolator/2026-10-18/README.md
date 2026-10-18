# Percolator work through Covenant Compute

Covenant Compute executed paid authorship, ordinary acceptance checks and native Kani jobs through signed orders and receipts. Both existing #241 withdrawal harnesses were proven. The new #239 appendix was authored and ordinarily checked through Covenant, but its Kani workflow is **incomplete: neither corrected route has an accepted proof, and no buggy control was run**. The #239 source PR remains unchanged.

Each signed archive preserves the exact order, operator receipt, returned artifacts, source mapping and offline verification instructions. Its directory name is the SHA-256 of `inventory.json`. The separate frozen-source supplement is explicitly unsigned local preparation.

## Completed jobs

| Work | Evidence | What completed |
| --- | --- | --- |
| Kani runner component | [Authorship and linked checker](9166c9ac0f89d1d4f7761cf3474d59e929a048764fff8d007f1f8946806c5fcf/README.md) | Accepted Covenant-authored runner patch; linked acceptance passed 39 fixture tests. Later local integration is distinguished from that paid patch. |
| Native admission canaries | [Generic check and tiny Kani proof](d1c6dc6b0da9ce9995caf04c69e8824dbc9872c21b53aeaf477dc315f943922d/README.md) | Both accepted through the signed native check path. |
| #241 withdrawal at and after expiry | [Both accepted Kani proofs](bbd7a6c440e5f8ba7cd8e1e7232eb2275fe206100d842a175da7f11f94f61ca2/README.md) | Existing source `035d4cb25eff1fce25d0a385ab92b700d8da3469`; both bounded properties proven and required covers reached. This is verification of earlier work. |
| #239 reduced public-route appendix | [Authorship and linked ordinary check](9542384342c68fcbfa6ff3b7ac5d2d69e961bcc3232018ed337d787b79983beb/README.md) | Author job `f79b455b-09e8-4a82-a971-fa00a3248100`; 159 ordinary specification tests and the protected buyer acceptance contract passed. No Kani proof is established by this acceptance. |
| Retained 12 GiB Kani profile | [Final paid smoke proof](517cd13898363b85acf4979a5e6f21519f0fef2327ae09ab070f2c8c14cf1e67/README.md) | Job `56f8bab8-d1e6-4231-9e76-7137574a3fa4` accepted after the seat returned to 16 GB; the tiny property was proven and its required cover reached at 12,288 MiB / 6 CPU / 4,096 MiB scratch. |

The #241 jobs are `b7a20990-a245-486e-ac4b-44d1331c272a` and `0425f25b-83bc-45ee-8fb9-cdcbc94d7b09`, each offering 50,000 micro-USDC. Each fixes expiry at slot 10 and current slot at 10 or 11, with backing 1 through 8, withdrawal 1 through backing, and claim 0 through backing minus withdrawal. Their signed orders bind the immutable image and 27,648 MiB / 6 CPU / 8,192 MiB scratch profile.

## #239 remains incomplete

The original engine fix and earlier harnesses predate the paid author job. Its exact returned patch, SHA-256 `8a1f74112d4097729afc18b2f83e9e7984e79fc55ead5dfcd4922ded261ac95b`, adds two narrowed public-route harnesses: long-side flat forfeit with independent long/short residues 0 through 3, and long-side flat auto-crank with one shared residue 0 through 3. Fixed weights, fixtures, successful-route covers and named assertions are recorded in the authored domain document; short-side, live-position and variable-weight cases are not claimed.

The unchanged patch was locally frozen on corrected commit `ea72a05d351207ce2eded5b517b049e186fed3aa` and buggy control `d51c9cd89aebc33842d1048e822b96c06cf47da8`. The [unsigned frozen-source supplement](cb7671edc403cf9511cb57527ae91639787b403ba8ee30c1aac5429c0093625d/README.md) lets readers reconstruct both original and frozen trees, verify the unchanged original proof prefixes, and compare identical appended bytes and domain documents. It does not claim a paid buggy-base execution or proof. The authored document's “not yet established” statement describes its authorship checkpoint.

Both corrected routes exhausted the available 27 GiB proof allocation; a separate forfeit retry was deliberately interrupted. All remain inconclusive. The larger 64 GB seat request was rejected with provider HTTP 403 quota exhaustion, and no 56 GiB proof profile was applied. Further #239 proof purchases stopped. Successful corrected proofs and matching buggy-base refutations remain prerequisites for amending the source PR; more memory is an untested possibility, not a promised solution.

## Failed executions retained

| Attempt | Signed result | Interpretation |
| --- | --- | --- |
| [#241 first attempt](e45e484093bd11eda6784f3b13b73b4d352bdf77263a9512f457ea25cface48d/README.md) | `tool_error` | Cargo build scripts could not execute on the scratch mount. The corrected executor was used by later distinct jobs. |
| [#241 second attempt](97e7fd29b7390e08298e6ec24d86e4e6ef4e3cb93f1dd189835d21fd41d3196d/README.md) | `tool_error` | Integration-test compilation required the explicit `fuzz` feature. Later accepted plans supplied it. |
| [#239 forfeit, source unwind 24](e0d4a2d0ccb0f3f4652cc9f6cc4ef939d4e1153c74cbe6be1fe90d7b901f100e/README.md) | `out_of_memory`, acceptance false | Job `002d3579-64f0-4f48-90c1-37a18c8d50d0`; 27 GiB exhausted after about 13 minutes. |
| [#239 forfeit, plan unwind 17](72952c8a2ae0d0e21774dc3002a19a89c34555ec430a8a17426f5c6307583c16/README.md) | `tool_error`, acceptance false; OOM and timeout false | Job `0bd8b774-78e7-44bd-a52d-40286b63efba`; the CBMC child was deliberately interrupted while the runner retained raw output. The operator event is an unsigned operational record, distinct from the signed failed execution. |
| [#239 auto-crank, plan unwind 17](cfb229d55f1350c5c74e64d0925fe178df331115c633a7b980ae2d87f49cb6a8/README.md) | `out_of_memory`, acceptance false; timeout false | Job `6040695e-4349-4fa9-98c9-dbdbd8b61e51`; 27 GiB exhausted after about 16 minutes with the original 20-minute solver budget. |

These failed attempts establish no proof, counterexample or required cover. All available raw output is preserved; an incomplete property report is not accepted as proof evidence. The later unwind-17 plans changed the bound without changing the source, asserted properties or required covers, and retained unwinding checks.

## Retained service and repeatability

At closeout the operator returned the seat to a 16 GB instance and retained the 12 GiB / 6 CPU / 4 GiB scratch Kani profile. The final paid smoke above separately exercises its receipt path. Provider shape, service health and quota observations are operational records, not claims independently attested by an operator signature. This smaller capability cannot satisfy the larger Percolator orders.

Follow each archive's README to verify its inventory, signatures, exact source mapping and available raw execution evidence offline; verification does not purchase compute. The #241 archive also provides a pinned CLI source-build guide and a rerun-command generator that defaults to preview. A paid rerun needs a fresh idempotency key, funded budget and the exact advertised image/resource capability. A successful archive verification reproduces the evidence checks, not a fresh solver execution.

The native path was implemented in Covenant PRs [#322](https://github.com/open-covenant/covenant/pull/322), [#328](https://github.com/open-covenant/covenant/pull/328) and [#331](https://github.com/open-covenant/covenant/pull/331). The pilot node uses `4dd036a4d59612326095768ff4b4f272aa48562d`; the compatible coordinator uses `983172ddd04ac2de70136125e2088f22c6edf505`. The CLI binary SHA-256 is `b2fc00d2dcb5010b316afc907187e95d7d71f68d9cf51205265acc0827c225cf`, actually built from `6502c3bb406b757c80410bf30014b08dc448e882`; the archives record scoped source equivalence to the public source used by the guide. A `0.2.0` version string alone does not establish that a downloaded binary includes these native commands.

Publication-branch integration fetched full landing history, reconciled two documentation inventories after an upstream base merge, and applied narrow gateway Clippy and test lock-scope repairs. Those local changes did not redeploy the pilot services or change the signed job source pins. Hosted CI results are available on [PR #334](https://github.com/open-covenant/covenant/pull/334/checks); archive verification is a separate gate.

Operator signatures authenticate bound claims and artifacts. They do not independently establish hardware isolation, a complete coordinator job inventory or blockchain settlement. A proven result covers only its selected harness and bounds. Authorship, ordinary acceptance, Kani verification, local integration, deployment and publication remain separate facts.
