Covenant
Copyright 2026 Covenant contributors

Licensed under the Apache License, Version 2.0. See LICENSE for the full license text.

This repository includes third-party dependencies managed by Cargo, pnpm, and GitHub Actions. Their licenses are documented in their respective package metadata and lockfiles.

Covenant Compute also includes Cloudflare's security-audit-skill, revision
c1c8a8c1471069fb0e188eeaff69b8e8db6564a8, licensed under MIT.
Copyright (c) 2025-2026 Cloudflare, Inc.
The unchanged source and full license are in
agent-os/crates/covenant-compute-buyer/skills/security-audit/upstream/.
Binary releases include SECURITY_AUDIT_LICENSE; the buyer can also print it with
`covenant-compute audit-skill --read upstream/LICENSE`.

Covenant Compute's knowledge compiler includes the MIT-licensed Tree-sitter
runtime 0.25.10 and grammars tree-sitter-rust 0.24.2,
tree-sitter-javascript 0.25.0, tree-sitter-typescript 0.23.2 (including TSX),
and tree-sitter-python 0.25.0. Their original copyright and permission notices,
plus Tree-sitter's Unicode dependency notices, are preserved in
deploy/compute-release/licenses/knowledge/. The sources.json manifest records
the exact upstream revisions, URLs and SHA-256 hashes of those license files.
Binary packages carry the same files under licenses/knowledge/.

The local knowledge viewer embeds Archivo and JetBrains Mono fonts under the
SIL Open Font License 1.1. Their unchanged license texts are included beside
the viewer assets and in the knowledge license directory described above.
