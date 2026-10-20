# Knowledge compiler container

This is the dedicated, deterministic compiler image for the typed
`knowledge-compile-v1:<OCI digest>` contract. The node accepts source JSON on
stdin. It does not run repository scripts or install repository dependencies.

Build from the repository root with Docker Engine and its containerd image store:

```sh
mkdir -p output/knowledge-container
python3 deploy/knowledge-compiler/build-context.py \
  > output/knowledge-container/context.tar.gz \
  2> output/knowledge-container/build-inputs.json
docker build --progress=plain \
  -f deploy/knowledge-compiler/Dockerfile \
  -t covenant-knowledge:reviewed \
  - < output/knowledge-container/context.tar.gz
docker image inspect covenant-knowledge:reviewed \
  --format '{{json .RepoDigests}}'
```

The staging script admits only tracked Rust source, Cargo manifests and the
lockfile. It records each input's SHA-256 and rejects symlinks and oversized
inputs. Cargo workspace members remain present so `--locked` preserves the
repository's dependency resolution. The Dockerfile builds only
`covenant-knowledge-compiler`. It pins both base images and removes the build
target and downloaded registry after compilation. The compiler image contains
no buyer key or source repository.

Use the exact returned `name@sha256:...` reference. A tag or an image config ID
is insufficient. If the Docker image store does not retain a repository digest
for a local build, use a private local registry to obtain a manifest digest
before testing. Do not claim deployment from a local image build.

## Real Docker acceptance

The buyer test uses an HTTP coordinator, signed jobs and receipts, the production
node executor, and deterministic import verification. Payment and the public
GitHub verifier remain test fixtures. Use new, absolute evidence paths:

```sh
export COVENANT_KNOWLEDGE_TEST_DOCKER_RUNTIME=docker
export COVENANT_KNOWLEDGE_TEST_DOCKER_IMAGE='NAME@sha256:COMPILER_DIGEST'
export COVENANT_KNOWLEDGE_TEST_EVIDENCE_PATH='/ABSOLUTE/NEW/http-evidence.json'
cd agent-os
CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 CARGO_INCREMENTAL=0 \
  cargo test --locked -p covenant-compute-buyer --lib \
  knowledge_remote::tests::knowledge_remote_http_dry_run_purchase_replay_and_conflict \
  -- --nocapture
```

The lifecycle image is deliberately faulty test data. It runs a fixed shell
fixture which blocks or emits oversized output; it never evaluates source text.
Build it from the repository root:

```sh
tar -czf output/knowledge-container/lifecycle-context.tar.gz \
  deploy/knowledge-compiler/lifecycle-fixture.Dockerfile \
  deploy/knowledge-compiler/lifecycle-fixture.sh
docker build -f deploy/knowledge-compiler/lifecycle-fixture.Dockerfile \
  -t covenant-knowledge-lifecycle:reviewed \
  - < output/knowledge-container/lifecycle-context.tar.gz
docker image inspect covenant-knowledge-lifecycle:reviewed \
  --format '{{json .RepoDigests}}'
```

Then run the opt-in test from `agent-os`:

```sh
export COVENANT_KNOWLEDGE_TEST_FAULT_IMAGE='NAME@sha256:FIXTURE_DIGEST'
export COVENANT_KNOWLEDGE_DOCKER_LIFECYCLE_EVIDENCE_PATH='/ABSOLUTE/NEW/lifecycle.json'
CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 CARGO_INCREMENTAL=0 \
  cargo test --locked -p covenant-compute-node --lib \
  knowledge::docker_tests::real_docker_isolation_timeout_output_cancel_and_scoped_orphans \
  -- --ignored --nocapture
```

This inspects actual Docker limits and probes the container's UID, writable root,
routes, effective capabilities and no-new-privileges state. It checks timeout,
output-limit and cancellation cleanup, and verifies that the orphan sweep leaves
another operator's container intact. It does not prove kernel isolation against
an exploit, paid settlement, an externally deployed operator, or a second CPU
architecture.

The 2026-10-20 acceptance ran on Linux arm64 inside a mount-free Lima VZ VM with
Docker Engine 29.9.1. The accepted compiler manifest was
`sha256:d011ce03baa42e825e345bd028c4b9af82e210535958c4ce5b49eb8a0503745c`;
the lifecycle fixture was
`sha256:0ba52586e6fead44e9e00ed57384a58066d6add5a7139d4bd0f0098da7d72a47`.
Both are local test artifacts. No external payment, registry publication or
production deployment was performed.
