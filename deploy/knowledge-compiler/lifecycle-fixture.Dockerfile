# Test fixture only. It deliberately blocks/overflows to test the production
# executor against the same pinned runtime base used by the real compiler.
FROM debian:bookworm-slim@sha256:7c7b2c966bc9ee8cedfeef67e0e279108992c77681fa595db4a9d65c06ccc587
COPY --chmod=0555 deploy/knowledge-compiler/lifecycle-fixture.sh /usr/local/bin/covenant-knowledge-compiler
USER 65532:65532
ENTRYPOINT ["/usr/local/bin/covenant-knowledge-compiler"]
