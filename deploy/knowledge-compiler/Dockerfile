# Build from the repository root. Operators must publish and configure an
# immutable image@sha256 reference; the node never pulls a mutable tag.
FROM rust:1.98-bookworm AS build
WORKDIR /source
COPY agent-os/ ./agent-os/
WORKDIR /source/agent-os
RUN cargo build --locked --release -p covenant-knowledge --bin covenant-knowledge-compiler

FROM debian:bookworm-slim
COPY --from=build /source/agent-os/target/release/covenant-knowledge-compiler /usr/local/bin/covenant-knowledge-compiler
USER 65532:65532
ENTRYPOINT ["/usr/local/bin/covenant-knowledge-compiler"]
