# Knowledge release candidate 0.5.0

The local Knowledge feature ships inside the existing four-command Compute
distribution. Remote operator availability and funded acceptance are separate
from client installation.

## Candidate validation

Build and verify one platform using the existing release path:

```sh
deploy/compute-release/dist.sh aarch64-apple-darwin
python3 deploy/compute-release/check-install.py \
  --archive dist/covenant-compute-darwin-arm64.tar.gz \
  --public-installer landing/public/install/compute.sh
```

`dist.sh` builds all four binaries with locked dependencies. It checks their
versions and help, the packaged audit files, the extracted offline Knowledge
workflow, and a fresh installation. `BUILD_INFO.json` identifies the source
revision, dirty tracked-source state, target, lockfile hashes and four binary
hashes. It is supporting metadata, not a signature or attestation.

`check-install.py` verifies the real native archive, installed binary identity,
source citations, retained license notices, repeat installation and conflicting
data rejection. Its website-installer check uses a temporary candidate pin and
a local download transport stub. It does not download a published 0.5.0 release
or change public installer pins.

Use a clean reviewed source commit for the final candidate. Preserve the archive,
its SHA-256, `BUILD_INFO.json`, build log and installer acceptance JSON together.
Rebuild and reverify if any shipping source or packaged file changes.

## Platform and publication gates

The `compute-release` workflow has four platform jobs: Apple Silicon and Intel
macOS, plus ARM64 and x86-64 static-musl Linux. A pull request or manual
`workflow_dispatch` builds and tests without publishing. The signing/release job
requires a **push** of a `compute-v*` tag; selecting a tag for a manual validation
run does not publish.

Before publication, require all four jobs on the reviewed candidate revision.
One successful native archive does not establish the other three targets.
Download and retain the four CI archives and their acceptance logs. Do not create
the release tag until its target commit and the candidate evidence are approved.

The signing job needs GitHub Actions `contents: write`, `attestations: write`,
and `id-token: write`. It uses GitHub build provenance and Sigstore keyless
signatures, with the certificate identity:

```text
https://github.com/open-covenant/covenant/.github/workflows/compute-release.yml@refs/tags/compute-v0.5.0
```

It publishes archives, checksum file, signatures and certificates. An existing
release with that tag causes refusal; do not replace a published release.
After publication, verify each archive's signature, checksum and GitHub
attestation using the commands in `notes/compute-v0.5.0.md`.

## Installer promotion

Read-only GitHub checks on 20 October 2026 found published `compute-v0.4.1`
and no `compute-v0.5.0` tag or release. Both the public website installer and
Homebrew formula were pinned to 0.4.1. These observations must be refreshed before
publishing.

Keep those public pins unchanged until the actual 0.5.0 signed artifacts exist.
Then use `pin.sh` with a Homebrew tap checkout and the website installer:

```sh
deploy/compute-release/pin.sh 0.5.0 /path/to/homebrew-tap \
  landing/public/install/compute.sh
```

The pinning script first verifies the published checksum file with the exact
workflow identity and OIDC issuer. It requires `cosign`; this local shell did
not have `cosign` or `gh` installed at the time of the audit. The active GitHub
workflow was readable as ID `387958034`; authenticated dispatch capability was
not established. The connected GitHub tools exposed workflow reads and retries,
but no new-workflow dispatch tool.

Validate a fresh installation through the real published installer and Homebrew
before announcing availability. The offline installer retains examples, guides,
build metadata and notices under `~/.local/share/covenant-compute/0.5.0`; Homebrew
retains those supporting files in `pkgshare`. Local Knowledge requires no buyer
identity or coordinator configuration.
