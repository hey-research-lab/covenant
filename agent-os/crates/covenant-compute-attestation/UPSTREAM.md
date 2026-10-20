# Upstream verifier provenance

Adapted from Prism Network, Apache-2.0, commit
`4e32eb5eb1398fd9d11dcd29aa5a96e3f2f646ec`, `crates/attestation`.
See LICENSE.prism and NOTICE.prism. Vendor roots, reference measurements and
test vectors retain upstream provenance; they are not Covenant production
acceptance or evidence of a live Covenant deployment.

Changes: crate/package imports, a minimal compatibility type module extracted
from upstream protocol, and separate Covenant UUID/session/authentication
bindings. Cryptographic verifier checks and compiled vendor references are
retained. The root source files carry upstream comments; references to Prism
services and rollout status describe upstream, not Covenant. Test private keys
are public synthetic fixtures solely for adversarial certificate tests.
