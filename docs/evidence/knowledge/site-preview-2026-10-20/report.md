# Knowledge public site release

The final page pairs the independently verified signed Compute 0.5.0 local release with its checksum-pinned installer. Paid remote pilot acceptance is reported separately; no persistent public remote supply is advertised. The source-preview candidate and canceled baseline below remain part of the release history.

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

## Inherited frozen-lock repair

The corrected page candidate `6d7fbe56b47841ca9a8ab6716142bd12bddd7a31` passed the landing CI job. The Mizuki `application` job `118765632840` failed before application checks: the root `pnpm-lock.yaml` omitted `landing`'s direct `sharp` dependency. The package manifest and lockfile both matched production; the existing root override already selected `sharp@0.35.5` and its complete snapshot was already present.

The repair adds only the three importer lines for `sharp`, preserving every resolved version and the separate landing lockfile. The baseline guard allows exactly that repair and rejects any other root lockfile change. No workflow conditions or required checks were weakened.

Locally, pnpm 10.31.0 `install --frozen-lockfile --lockfile-only --ignore-scripts` passes for all 25 workspace manifests. This validates the frozen resolution without relinking the shared local dependency directory; CI must still complete the normal full install. The page's 64 metadata/card tests and social-card audit (85 pages, 78 prerendered pages) also pass.

## Acceptance before promotion

- Production build and the site's social-card check pass.
- Desktop, tablet and mobile show the restored lens and square graph nodes in a coherent glass-plane perspective; no horizontal overflow or page errors.
- `/compute-knowledge`, its page-specific social card, the hero asset and source-guide links resolve. Existing Compute pages remain available.
- Live service configuration, environment hash, plan, region, instance count and configured `main` branch remain unchanged.
- The exact reviewed source revision is journaled before one deployment request. An unknown request outcome is reconciled, never blindly resubmitted.
- The same checks run against the public URL after the service reports the exact revision live.
- The change must also reach the configured production branch so a later `main` auto-deploy cannot remove it.

The original spherical and flat-square artwork variants are retained in task output. The selected edit restores the original lens and uses foreshortened square glass nodes. The official mark and exact Archivo Expanded 600 / JetBrains Mono 500 text remain separately rendered. Export sizes are 1920×1080 and 3840×2160; the generated background is scaled for the 4K composition, while typography is rendered at export resolution.

## Signed local release handoff

All four public archives were independently checked against their Cosign signatures, the exact release workflow/tag identity, signed checksum list, source/ref attestations and native CI artifacts. The [verification record](https://github.com/open-covenant/covenant/blob/3d4f2e86eedae3959ce98909ea3598660b30f8b4/docs/evidence/knowledge/public-release-0.5.0/report.md) retains the checks. The local page links the actual `compute-v0.5.0` release; its measurement reports are pinned to that tag.

The website installer SHA-256 is `18883da2515ea243ef7c95a92c1fac3a60f8c30f55d25df17c17c688efa0b0ef`. Outside its five release pins, its body matches the accepted release source. It delegates to the packaged offline installer so examples, guides and notices accompany the four commands. The baseline guard permits only this exact reviewed installer or the original production bytes. Homebrew promotion is separately blocked by repository write permissions; the page does not advertise Homebrew 0.5.0.

The remote copy links immutable paid evidence at `74c432f900e57d48557abf79a52891b6f28aac0c` and says that public remote capacity is pending. It offers no remote purchase CTA or standing price.

A fresh local workflow capture uses the actual installed public ARM64 macOS buyer, SHA-256 `6cbe4f6ec099a8d15be82ca51b5687732fbef790c1f93f019f51b91962fc85b6`, with two original-example snapshots and verified source citation bytes. The recording and real MCP transcript are retained in task output. No model, wallet, daemon or remote job is involved.

## Live deployment verified

The final release page and verified installer reached `main` by a normal protected fast-forward after the existing required checks passed. Render automatically deployed commit `5b86392ab80be15611cba4bec6cc24ee48b7de43` as `dep-dbbqblbl550s739b5d70`, live at 2026-10-20 17:35:52 UTC. No force push, protection change or duplicate manual deployment was used.

The [deployment record](deployment.json) confirms that service configuration and all 20 environment values remained unchanged. [Public route checks](public-routes.json) returned HTTP 200 for the feature, installer, social card, source artwork and existing Compute/brand routes. Public HTML contains the local 0.5.0 release copy and the explicit remote-capacity boundary; the preview-pending copy is absent.

[Live browser QA](browser-qa.json) passed at 1440×1000, 768×1024 and 390×844: no page errors or horizontal overflow, actual images loaded, correct canonical/social image, and both section links worked. Full-page and hero screenshots remain in task output. The codebase passed a production build, 64 metadata tests and the social-card audit before promotion.

The live HTTPS installer bytes match the independently reviewed hash above. An actual fresh install from that URL downloaded the public archive and passed all four matching binary hashes, 19 local Knowledge checks, 18 citation checks and the six read-only MCP tools without creating a home/wallet directory. Public installation evidence is retained with the signed-release report.

The release media uses the same verified public buyer hash. Its final local demo is 38.72 seconds, H.264 at 1920×1080/25fps, with no capture errors. The 255-character announcement, revised artwork and demo are ready for a separate human publishing action. No X post was sent or scheduled.
