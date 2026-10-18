# PR #239 frozen-source supplement — unsigned local preparation

This supplement makes the later local dual-base freeze independently reconstructible. It is **not an operator receipt, a Kani result, or a claim that the buggy control ran**. The source PR remains unchanged. Existing signed archives are unchanged.

First verify the [signed authorship and linked-check archive](https://github.com/open-covenant/covenant/tree/b16987549e3dd5d620c6f7b917b5a164c1b00c4d/docs/compute-evidence/percolator/2026-10-18/9542384342c68fcbfa6ff3b7ac5d2d69e961bcc3232018ed337d787b79983beb). Its authenticated returned patch must be byte-identical to `returned.patch` here: SHA-256 `8a1f74112d4097729afc18b2f83e9e7984e79fc55ead5dfcd4922ded261ac95b`. That archive authenticates the original input, task, returned patch and ordinary checker claims; it does not authenticate this later local freeze as a paid proof.

`manifest.json` maps both original bases, local frozen commits, history-free snapshots, source trees, and exact proof-prefix hashes. The four `*-commit.raw` files retain the original/base and frozen commit objects. The corrected bundle is byte-identical to the signed source bundle in the [OOM archive](https://github.com/open-covenant/covenant/tree/c1a0ed3f163e35f9fb23b1c8d190ec08c6c11cbf/docs/compute-evidence/percolator/2026-10-18/e0d4a2d0ccb0f3f4652cc9f6cc4ef939d4e1153c74cbe6be1fe90d7b901f100e); the buggy bundle is newly prepared unsigned local data. Both bundle commits have no parents, so neither exports repository history. The exact separately retained frozen commits retain their original parent hashes in their raw bytes.

| Variant | Snapshot commit | Original base tree | Frozen tree | Original proof-prefix bytes |
| --- | --- | --- | --- | --- |
| corrected | `895f9a8f0164d199584a1035297d7f5e0d00f8d4` | `6b2882d92ffad6e8df932acae4a0a3d356a1d372` | `5703a742d9b3fca8038ae4f93ec74e0be2b7fc46` | 790518 |
| buggy | `9e8a17aa181d06916396cff0b8a0c6ffb46a9bb8` | `8e02f621e2f8b800c3bd81ccaca30889ec165975` | `6d0fc1d3af8a192a04bff97eb21e923dc05e42b7` | 790038 |

## Reconstruct without executing source or buying compute

Check `SHA256SUMS` from this directory. For each variant, use a fresh empty review directory and the corresponding snapshot from the table. `git bundle unbundle` is intentional: the byte-preserved corrected bundle advertises `refs/covenant/snapshot`, which default `git clone` does not check out.

```sh
shasum -a 256 -c SHA256SUMS
SUPPLEMENT_DIR="$PWD"
VARIANT=corrected
SNAPSHOT_COMMIT=895f9a8f0164d199584a1035297d7f5e0d00f8d4
REVIEW_DIR="$(mktemp -d)"
git init "$REVIEW_DIR"
git -C "$REVIEW_DIR" bundle unbundle "$SUPPLEMENT_DIR/$VARIANT-source.bundle"
git -C "$REVIEW_DIR" checkout --detach "$SNAPSHOT_COMMIT"
git -C "$REVIEW_DIR" rev-parse 'HEAD^{tree}'
git hash-object -t commit "$VARIANT-base-commit.raw"
git hash-object -t commit "$VARIANT-frozen-commit.raw"
git -C "$REVIEW_DIR" apply --reverse --index "$SUPPLEMENT_DIR/returned.patch"
git -C "$REVIEW_DIR" write-tree
git -C "$REVIEW_DIR" diff --cached --name-only HEAD
git -C "$REVIEW_DIR" apply --index "$SUPPLEMENT_DIR/returned.patch"
git -C "$REVIEW_DIR" write-tree
```

The first and last trees must equal the frozen tree; the reverse-applied tree must equal the original base tree. Raw commit hashes must equal `manifest.json` and their first `tree` headers must match the corresponding trees. The reverse diff must name only `tests/proofs_v16.rs` and `docs/proofs/pr239-route-domains.md`.

The full original proof prefix must remain byte-identical after patch application. The final 8,218 bytes must equal `identical-appendix.rs` on both variants, and the new domain document must equal `route-domains.md`. Their exact hashes are in the manifest. `reconstruction-review.json` records these checks performed in disposable offline Git repositories, with no Rust source executed. Readers can repeat them independently; that report itself is unsigned.

No proof acceptance, counterexample, cover, hardware isolation or settlement is established by this supplement. Successful signed Kani evidence remains a separate requirement.
