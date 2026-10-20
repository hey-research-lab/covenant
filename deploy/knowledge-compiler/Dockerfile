# Build from the repository root. Operators must publish and configure an
# immutable image@sha256 reference; the node never pulls a mutable tag.
FROM rust:1.98-slim-bookworm@sha256:ff521445a372125ed4f76e1453a1f8098f2d05332d1601d30db1c1f62757e730 AS build
RUN apt-get update && apt-get install -y --no-install-recommends gcc libc6-dev \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /source
COPY agent-os/ ./agent-os/
WORKDIR /source/agent-os
ENV CARGO_INCREMENTAL=0 CARGO_PROFILE_RELEASE_DEBUG=0
RUN cargo build --locked --release -j 2 -p covenant-knowledge --bin covenant-knowledge-compiler \
    && cp target/release/covenant-knowledge-compiler /usr/local/bin/covenant-knowledge-compiler \
    && rm -rf target /usr/local/cargo/registry

FROM debian:bookworm-slim@sha256:7c7b2c966bc9ee8cedfeef67e0e279108992c77681fa595db4a9d65c06ccc587
COPY --from=build /usr/local/bin/covenant-knowledge-compiler /usr/local/bin/covenant-knowledge-compiler
USER 65532:65532
ENTRYPOINT ["/usr/local/bin/covenant-knowledge-compiler"]
