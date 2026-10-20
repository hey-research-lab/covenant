# Native 0.5.0 release candidate acceptance

The actual four-binary Apple Silicon archive built and passed local installation
acceptance on 20 October 2026. This is an unsigned native candidate; it does not
establish the other three platform builds, public installation, publication,
deployment or funded acceptance.

| Artifact | Recorded value |
|---|---|
| Archive | `dist/covenant-compute-darwin-arm64.tar.gz` |
| Bytes | 16,318,494 |
| Archive SHA-256 | `8ad75e576ce5e52931cc147f5397fd9e9f1362618b9dc0ecd8b2b2c81c3cbd60` |
| Source revision | `3d4499211fabec97da0ee918dc281bd3a4226758` |
| Tracked source dirty at packaging | `false` |
| Target | `aarch64-apple-darwin` |
| All four command versions | `0.5.0` |
| Buyer SHA-256 | `16d5a702cbe298580a130f2707e84ff8e2bb9fc6a423ee31aa07306b423a74bf` |

The extracted archive and the freshly installed buyer each passed all 19 offline
Knowledge checks, with 18 exact citations verified. The installer harness passed
11 checks covering binary hashes, retained notices, paths containing spaces,
repeat installation, conflicting data refusal and checksum rejection. An
additional negative check confirmed that a directory occupying a binary path is
rejected before data or executable replacement.

The website-installer test used a temporary candidate pin and a local transport
stub serving these exact archive bytes. It did not download a published 0.5.0
release or modify the public 0.4.1 pins. No wallet or buyer state was created.

Reproduce from the reviewed source revision:

```sh
deploy/compute-release/dist.sh aarch64-apple-darwin
python3 deploy/compute-release/check-install.py \
  --archive dist/covenant-compute-darwin-arm64.tar.gz \
  --public-installer landing/public/install/compute.sh
```

The local archive is retained in `dist/`; it is not committed. Exact archive
hashes may differ on another build because build timestamps and archive metadata
are recorded. Inspect `BUILD_INFO.json` and binary hashes for each actual build.

Evidence: [build metadata](BUILD_INFO.json),
[installer acceptance](install-acceptance.json), and
[sanitized native build and package log](native-dist.log). Only paths in the
log were normalized to `$CHECKOUT` and `$TEMP`; test output is otherwise retained.

Four-platform CI is currently blocked on repository access. Read-only repository
permission checks through both the connected GitHub account and configured Git
credential helper reported `pull: true`, with `push`, `maintain` and `admin` all
false. See the [permission observation](github-access.json). No workflow dispatch
was attempted and no alternate account was tried. A repository collaborator must
run the four-platform validation and approved signing/publication.

The remaining gates are that CI run, review of its actual artifacts, the approved
tag/signing/publication flow, and fresh installation through the real signed
public release and promoted installer pins. Remote Docker and paid operator
acceptance have their own evidence and are not inferred from this local package.
