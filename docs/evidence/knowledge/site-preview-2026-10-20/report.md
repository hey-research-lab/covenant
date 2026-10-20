# Knowledge source-preview site release

This page release exposes the source preview and local workflow. It does not announce a signed Compute 0.5.0 release or persistent paid remote supply.

## Production baseline guard

Before triggering a site deployment, freshly read the live Render revision and `origin/main`, then compare **the complete `landing/` tree**, not only the last commit. A Compute deployment branch may have an older website baseline even when its new page builds successfully.

For a Knowledge-only site change, create a checkout from current `origin/main`, copy only the Knowledge route and assets, and add the single discovery link and sitemap entry in the current site structure. Preserve every existing route, release document, installer pin, lockfile, header, footer and brand asset. Use the current site's `pageMetadata()` and `/og/<page>.png` contract.

Run the bounded allowlist guard before every deployment:

```sh
python3 scripts/check-knowledge-site-baseline.py --baseline <fresh-production-or-main-commit> --candidate <reviewed-deploy-commit>
```

The guard rejects deletions, renames, paths outside the feature, and unrelated changes to the shared navigation/sitemap files. Inspect the complete diff as well. Passing a feature build alone is insufficient.

## Canceled unsafe baseline

The initial exact-commit request `dep-dbbpop8m7kps73d3lip0` targeted feature commit `7ebdf36a4480c7edb9e26708ab127118d3e47f8c`. The full baseline check found unrelated website reversions. Its pending build was canceled at 2026-10-20 16:54:12 UTC and the Render API subsequently confirmed `canceled`.

Production remained on deployment `dep-dbb7dem0tbcc73f3s3o0`, commit `3f50e7eed7d88c88f6d97193f8e2cae19cf2e58d`. No unsafe build was promoted. The corrected candidate starts at that freshly fetched `origin/main` revision and adds only the Knowledge feature.

## Acceptance before promotion

- Production build and the site's social-card check pass.
- Desktop, tablet and mobile show the restored lens and square graph nodes in a coherent glass-plane perspective; no horizontal overflow or page errors.
- `/compute-knowledge`, its page-specific social card, the hero asset and source-guide links resolve. Existing Compute pages remain available.
- Live service configuration, environment hash, plan, region, instance count and configured `main` branch remain unchanged.
- The exact reviewed source revision is journaled before one deployment request. An unknown request outcome is reconciled, never blindly resubmitted.
- The same checks run against the public URL after the service reports the exact revision live.
- The change must also reach the configured production branch so a later `main` auto-deploy cannot remove it.

The original spherical and flat-square artwork variants are retained in task output. The selected edit restores the original lens and uses foreshortened square glass nodes. The official mark and exact Archivo Expanded 600 / JetBrains Mono 500 text remain separately rendered. Export sizes are 1920×1080 and 3840×2160; the generated background is scaled for the 4K composition, while typography is rendered at export resolution.
