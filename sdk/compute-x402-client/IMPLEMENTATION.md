# x402 AgentKit client qualification

## Version 0.2.0: LangChain and LangGraph

### Current candidate: trusted system policy

The adapter also accepts an optional application-controlled `systemPrompt`.
It is captured at tool creation, validated with the client request schema and
sent as an exact system message before the model-provided user prompt. The
model-facing tool schema still accepts only `prompt`. Omitting the policy
preserves the original user-only messages. The support demo supplies a fixed
drafting policy that prohibits claims of account access or actions taken; model
instructions do not establish factual correctness or replace human review.

All **32 tests passed from source and from the installed package** on 2026-10-18.
The additional checks prove exact signed policy content, resistance to mutated
application options and model arguments, rejection of a changed or removed
policy under the same purchase UUID, and refusal of oversized combined UTF-8
messages before network access, reservation or signing. The complete earlier
framework, receipt and SQLite restart tests passed without skips.

Current artifact: `/tmp/covenant-langchain-policy-gq8lelzp/covenant-compute-x402-client-0.2.0.tgz`,
SHA-256 `77ef86146f982432f75a83d227afb8c7a8f36049bb23d4eba29ae0da0cb606cf`.
Every packaged file matches the source and installed bytes. This candidate
updated only the package in the previously clean consumer environment at
`/tmp/covenant-langchain-accepted-lmglpy2z`; framework dependency versions and
the native SQLite binding remain unchanged. The new candidate directory holds
`qualification.json`, source/installed test logs and the install log. The first
candidate's tarball, qualification record and test logs are retained unchanged.
These tests use the local fixture and do not perform paid operations; funded
acceptance and public release evidence are recorded separately.

### Initial candidate qualification

Qualified locally on 2026-10-18 with Node 24.20.0. The adapter exports an official
LangChain `DynamicStructuredTool` over the accepted x402 client. Its model-facing
schema contains only `prompt`; a trusted application fixes the purchase UUID,
model, generation settings, wallet, deployment identities and spending ceilings.
One tool instance represents one logical purchase. A new model tool-call ID or
graph attempt does not allocate another purchase. The full verified client
response is retained in `ToolMessage.artifact`, with a compact answer/payment
summary in the model-facing content.

Pinned framework packages: `@langchain/core` 1.2.18,
`@langchain/langgraph` 1.5.1 and `langchain` 1.5.18. Optional demo dependencies
are `@langchain/openai` 1.6.2 and
`@langchain/langgraph-checkpoint-sqlite` 1.1.0. The existing payment dependency
versions and AgentKit API remain unchanged.

All **30 tests passed from source**, including the actual official facilitator,
and the same **30 passed against a fresh consumer installation** of the packed
artifact outside the checkout. No tests were skipped. New checks cover:

- Real `createAgent` and `createReactAgent` registration/execution, receipt
  artifacts and completed checkpoint replay.
- Unknown or forbidden model arguments rejected before network, reservation or
  wallet signing; application settings cannot be changed by later option mutation.
- Original authorization reuse across client restart and concurrent recovery;
  changed prompts/generation settings under the same purchase UUID fail before
  network access. Tampered answers never become verified artifacts.
- Support-demo plan, purchase, replay and finish in separate Node processes
  using the official SQLite checkpointer. A 202 pending response refuses finish,
  resumes the original pre-tool checkpoint, and retains the same signed payment.
  Completed replay returns the exact artifact without another network request.

The framework planner in these tests is a scripted offline fixture. It validates
framework execution and persistence, not live model selection. Wallet keys are
generated unfunded fixtures, and RPC/payment outcomes are provided by the local
test server. No public transaction, provider inference or paid operation was run
for these tests.

Qualified artifact: `covenant-compute-x402-client-0.2.0.tgz`, SHA-256
`4328500321a9bf4960980e5666d68f0f0be0c7f1bff904ccb2d2caaa48fcd33e`.
Every installed package file was compared byte-for-byte with the tarball and
source. The retained consumer environment is
`/tmp/covenant-langchain-accepted-lmglpy2z`; `qualification.json` contains the
per-file hashes and `installed-tests.log` contains the results. Its optional
demo dependencies were installed explicitly and its `better-sqlite3` native
binding was rebuilt and exercised on Node 24. An earlier clean-install attempt
ran out of disk space and was discarded; the successful fresh installation and
tests were performed only after recovering disk headroom.

The exact tarball and installed environment are retained for separate funded
release acceptance. Neither publication nor production deployment is established
by this local qualification. The immutable 0.1.0 release remains unchanged.

## Version 0.1.0 qualification history

Validated locally on 2026-10-18 using Node 24.20.0. This file records implementation evidence, not deployment or funded execution.

The direct dependency versions are Coinbase AgentKit 0.10.4, official x402 core/SVM 2.28.0, Solana Kit 8.5.0 and web3.js 1.98.4. `npm-shrinkwrap.json` pins the full graph and is included in the published tarball. No `--legacy-peer-deps`, `--force`, private package mirrors or modified dependency source was needed.

18 focused tests passed from source. The same 18 passed against a packed tarball installed in `/tmp/covenant-x402-packed-check`, outside the source checkout. The installed tests import the public package exports for the client and AgentKit action. Both runs exercised:

- AgentKit registration and real `SolanaKeypairWalletProvider.signTransaction` invocation;
- official x402 client transaction construction and actual gateway facilitator verification, with simulation mocked and broadcasting prohibited;
- exact standard memo/request binding and a real payer signature;
- real Rust `BuyerExecutor.prepare` and `SignedWorkReceipt` fixture verification;
- invalid network, asset, recipient, price, digest, memo and expiry rejected before wallet signing or paid dispatch;
- exact authorization reuse after unknown network outcome and client restart;
- completed result reuse, changed-request refusal and persistent cumulative spend limits;
- simultaneous independent Node processes sharing one SQLite ceiling;
- launch limits of 32 messages and 4,096 UTF-8 JSON bytes enforced before quoting, reserving or signing;
- duplicate JSON keys, changed signed output and invalid receipt signatures rejected.

The cross-process test found an initialization race: SQLite's busy timeout must be set before changing the journal mode. The implementation and test now cover that ordering.

AgentKit 0.10.4's initialization analytics hook starts an unawaited request that produced an unhandled HTTP-400 rejection during the initial test. `CovenantSolanaKeypairWalletProvider` suppresses that hook only; signing and custody inherit Coinbase's original wallet implementation.

Reproduce source and cross-component checks from this directory:

```sh
npm ci --ignore-scripts --no-audit --no-fund
COVENANT_X402_BRIDGE_PATH="$(pwd)/../../services/compute-x402-facilitator/bridge.mjs" npm test
```

The package tarball tested on this date is `/tmp/covenant-x402-packed-check/covenant-compute-x402-client-0.1.0.tgz`, SHA-256 `815cc93d7861447730c695c00f5ad0825e609f16f958ec8ef5cf117cc2bfdcbd`. This is a local artifact, not a publication URL.

No mainnet payment, production inference request, external provider charge, deployment or public package publication was performed during this client qualification. Those gates belong in the release's production evidence after the funded run.
