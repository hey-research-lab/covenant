# Client release 0.5.2

This client adds guided local setup and read-only source freshness. Its
release notes are `notes/compute-v0.5.2.md`. The snapshot format and deployed
compiler capability remain unchanged. This client work does not authorize a
new paid acceptance job, operator deployment or stake change.

Run the candidate through the same four native jobs below before tagging.
Publication requires the 0.5.2 tag pipeline, independent signature/attestation
verification and fresh installation to complete. A candidate build is not a
published release.
Website and Homebrew pins are separate promotions; never infer them from a tag.
For validation on `codex/knowledge-onboarding`, dispatch `compute-release`
against the reviewed branch revision and verify its returned head SHA.
The configured push trigger on the older candidate branch is not needed.

The original 0.5.0 instructions below are retained as historical release
evidence. For the current operator admission cutoff, use the finalized
[21 October extension report](../../docs/evidence/knowledge/stake-extension-2026-10-21/report.md):
17 November 2026 at 22:10:13 UTC, subject to current health and eligibility.
The older October cutoff in the historical activation text is superseded.

# Knowledge release 0.5.0

The local Knowledge feature ships inside the existing four-command Compute
distribution. Remote operator availability and funded acceptance are separate
from client installation.

Limited dedicated remote capacity was separately
[accepted on 20 October 2026](../../docs/evidence/knowledge/remote-activation-2026-10-20/report.md):
one exact-compiler operator with one job in flight, cohosting and controlled
idle restart checks, and an installed 0.5.0 public-source preview with no new
purchase. The two earlier paid pilot jobs remain separate evidence. Initial
stake admission eligibility ends 25 October 2026 at 19:17:25 UTC unless extended;
always check current capacity and price.

## Published local release

On 20 October 2026, `compute-v0.5.0` was published from accepted commit
`8cbaeb7e423eead8bf12389820857437ad693717`. All four native candidate builds and
fresh installations passed before the tag was created. The tag pipeline then
built, installed, attested, signed and published all four platform archives.
Independent verification authenticated every public archive, checked its source
and binary manifest, and confirmed its bytes match the corresponding native CI
artifact. A fresh post-download Apple Silicon installation also passed.

See the [public release verification](../../docs/evidence/knowledge/public-release-0.5.0/report.md)
and its machine-readable record for the source, hashes, signatures, attestations,
native job links and precise installation scope. Website and Homebrew promotion
are separate checks. The actual public website installer subsequently passed a
fresh HTTPS installation of 0.5.0 with all four binary hashes, retained examples
and notices, 19 Knowledge checks, 18 citations and all six read-only MCP tools.
See [installer promotion](../../docs/evidence/knowledge/public-release-0.5.0/installer-promotion.md)
for that result and the separately blocked Homebrew update.

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
macOS, plus ARM64 and x86-64 static-musl Linux. A pull request, the configured
Knowledge candidate branch, or manual `workflow_dispatch` builds and tests
without publishing. The signing/release job
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

The signed 0.5.0 release and four archive signatures have now been independently
verified. Use `pin.sh` with a Homebrew tap checkout and the website installer:

```sh
deploy/compute-release/pin.sh 0.5.0 /path/to/homebrew-tap \
  landing/public/install/compute.sh
```

The pinning script first verifies the published checksum file with the exact
workflow identity and OIDC issuer. It requires `cosign`. Verification used the
official checksummed Cosign 2.4.1 and GitHub CLI 2.102.0 tools. Use the data-aware
installer body from the accepted release commit as well as the authenticated
pins: an older four-command-only website installer does not retain examples and
guides. Review the body change independently of the five pinned values.

Validate a fresh installation through each installation method before
advertising that method. Signed archive and website-script availability can be
announced independently of Homebrew promotion. The prepared 0.5.0 Homebrew
formula passed signature-backed pin and Ruby syntax checks, but the existing
credential's normal Git push was denied. The public tap remains at 0.4.1;
no 0.5.0 Homebrew install or tap CI result is claimed.

The offline installer retains examples, guides, build metadata and notices under
`~/.local/share/covenant-compute/0.5.0`. The prepared Homebrew formula retains
those supporting files in `pkgshare` when its promotion is authorized by the
repository. Local Knowledge requires no buyer identity or coordinator
configuration.
