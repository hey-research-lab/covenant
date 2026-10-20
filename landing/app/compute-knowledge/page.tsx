import type { Metadata } from "next";
import { pageMetadata } from "../_metadata";
import Image from "next/image";
import localFont from "next/font/local";
import { SiteFooter } from "../SiteFooter";
import { SiteHeader } from "../SiteHeader";
import styles from "./page.module.css";

const display = localFont({ src: "./_assets/archivo-expanded-600.ttf", weight: "600", display: "swap" });
const mono = localFont({ src: "./_assets/jetbrains-mono-500.ttf", weight: "500", display: "swap" });
const title = "Compute Knowledge — Covenant Compute";
const description = "Covenant Compute’s local code knowledge tools: source citations, dependency inspection and revision snapshots for Rust, Python, JavaScript, TypeScript and Markdown.";

export const metadata: Metadata = pageMetadata(
  "/compute-knowledge",
  { title: "Codebase knowledge your agents can inspect.", section: "Compute Knowledge / local release" },
  { title, description },
);

const documentLink = "underline decoration-line-strong underline-offset-4 transition-colors hover:text-fg hover:decoration-fg-soft";

const steps = [
  { number: "01", title: "Compile locally.", body: "Extract symbols, document headings and references into a versioned snapshot. Local commands run without a model key or a Compute purchase.", tag: "Rust · Python · JS / TS · Markdown" },
  { number: "02", title: "Follow the source.", body: "Search names and source text, then inspect the file, line range and exact excerpt behind a result. Read the same evidence through the local viewer or MCP.", tag: "Citations · search · read-only MCP" },
  { number: "03", title: "Inspect what changed.", body: "Explore resolved and unresolved references. Compare saved snapshots to find added, modified and removed objects, with parser diagnostics kept visible.", tag: "Dependencies · snapshots · diff" },
];

export default function ComputeKnowledgePage() {
  return (
    <>
      <SiteHeader />
      <main id="main-content" className={`${styles.page} ${mono.className}`}>
        <section className={styles.hero} aria-labelledby="knowledge-title">
          <div className={styles.heroArt}>
            <Image src="/compute-knowledge/field-square-perspective.webp" alt="" fill priority sizes="(max-width: 900px) 100vw, 1240px" />
          </div>
          <div className={styles.heroContent}>
            <p className={styles.eyebrow}><span className={styles.dot} />Local release · Compute 0.5.1</p>
            <p className={styles.product}>Covenant Compute / Knowledge</p>
            <h1 id="knowledge-title" className={display.className}>Codebase knowledge<br /><span>your agents<br />can inspect.</span></h1>
            <p className={styles.lead}>A code knowledge layer inside Covenant Compute. Give agents and developers a searchable view of a repository, with evidence they can inspect.</p>
            <div className={styles.actions}>
              <a className="btn btn-primary" href="#local-preview">Explore the local workflow <span aria-hidden="true">↗</span></a>
              <a className="btn btn-secondary" href="#availability">See what is available <span aria-hidden="true">↓</span></a>
            </div>
            <p className={styles.releaseNote}>Available for macOS and Linux, on ARM64 and x86-64.</p>
          </div>
        </section>

        <div className={styles.languageStrip} aria-label="Supported input languages">
          <span>Source in.</span><span>Rust</span><span>Python</span><span>JavaScript</span><span>TypeScript</span><span>Markdown</span><span>Evidence out.</span>
        </div>

        <section className={styles.section} aria-labelledby="workflow-title">
          <p className={styles.eyebrow}>From repository to evidence</p>
          <h2 id="workflow-title" className={display.className}>Context you can open.</h2>
          <div className={styles.steps}>
            {steps.map(step => (
              <article key={step.number} className={styles.step}>
                <span className={styles.number}>{step.number}</span>
                <h3 className={display.className}>{step.title}</h3>
                <p>{step.body}</p>
                <span className={styles.tag}>{step.tag}</span>
              </article>
            ))}
          </div>
        </section>

        <section id="local-preview" className={`${styles.section} ${styles.local}`} aria-labelledby="local-title">
          <div>
            <p className={styles.eyebrow}>Start with a local repository</p>
            <h2 id="local-title" className={display.className}>One snapshot.<br />A shared view.</h2>
            <p className={styles.copy}>Compile once, then inspect the same saved source through the CLI, a browser viewer or a read-only MCP connection. Historical queries stay attached to the snapshot you select.</p>
            <p className={styles.small}>Install the signed Compute 0.5.1 release, then compile the included example or your own repository. The compile command returns your project ID; the viewer prints its private loopback URL.</p>
          <p className={styles.small}><a className="btn btn-secondary" href="https://github.com/open-covenant/covenant/releases/tag/compute-v0.5.1">Download Compute 0.5.1 ↗</a><br /><span>Signed release, four native builds. Guides and examples are included.</span></p>
          </div>
          <div className={styles.terminal}>
            <div className={styles.terminalBar}><span>LOCAL WORKFLOW</span><span>COMPUTE 0.5.1</span></div>
            <pre tabIndex={0} aria-label="Example local knowledge commands"><code>{String.raw`# Install the checksum-pinned release
curl -fsSL https://opencovenant.org/install/compute.sh | sh
export PATH="$HOME/.local/bin:$PATH"

# Compile the included original example
covenant-compute knowledge compile \
  "$HOME/.local/share/covenant-compute/0.5.1/examples/knowledge"

# Use the project ID returned by compile
covenant-compute knowledge query "calculate_total" \
  --project <id>

# Open the saved project
covenant-compute knowledge serve --project <id>

# Connect a read-only agent tool
covenant-compute knowledge mcp --project <id>`}</code></pre>
            <div className={styles.terminalFooter}>Local compilation does not execute repository code.</div>
          </div>
        </section>

        <section className={`${styles.section} ${styles.evidence}`} aria-labelledby="evidence-title">
          <div className={styles.evidenceHeading}>
            <p className={styles.eyebrow}>Make the limits visible</p>
            <h2 id="evidence-title" className={display.className}>Evidence has boundaries.</h2>
          </div>
          <div className={styles.boundaries}>
            <div><span className={styles.marker}>/ SOURCE</span><h3>Exact excerpts.</h3><p>A citation identifies saved source and its line range. It helps you check an answer; it does not prove the code is correct.</p></div>
            <div><span className={styles.marker}>/ COVERAGE</span><h3>Visible gaps.</h3><p>Unsupported files, exclusions and parser errors remain diagnostics. Dependency resolution is bounded; dynamic behavior can remain unresolved.</p></div>
            <div><span className={styles.marker}>/ INTEGRITY</span><h3>Inspectable snapshots.</h3><p>Verification rechecks snapshot content and extraction. It is not a security audit, completeness guarantee or claim of trusted execution.</p></div>
          </div>
        </section>

        <section id="availability" className={`${styles.section} ${styles.availability}`} aria-labelledby="availability-title">
          <div>
            <p className={styles.eyebrow}>Built into Compute</p>
            <h2 id="availability-title" className={display.className}>Local tools.<br />Remote compilation.</h2>
            <p className={styles.copy}>Compile a pinned public GitHub revision through Covenant Compute, then inspect the verified snapshot locally. Remote input accepts 1–256 selected files and at most 512 KiB of source JSON, including filenames, text and metadata.</p>
            <div className={`${styles.actions} mt-6`}>
              <a className="btn btn-primary" href="https://github.com/open-covenant/covenant/blob/6483edca0eaa5476d72f84f8da571df4d4dab7b9/docs/compute-knowledge-remote.md">Preview a remote request <span aria-hidden="true">↗</span></a>
              <a className="btn btn-secondary" href="https://github.com/open-covenant/covenant/blob/6483edca0eaa5476d72f84f8da571df4d4dab7b9/docs/evidence/knowledge/dedicated-acceptance-2026-10-21/report.md">Read paid acceptance <span aria-hidden="true">↗</span></a>
            </div>
            <p className={styles.small}>One dedicated operator offers the pinned compiler, with one job in flight at a time. Preview the current quote and capacity before buying. The finalized stake extension sets the admission cutoff at 17 November 2026 at 22:10:13 UTC, with stake unlock on 19 November at the same time. Continued availability depends on current capacity and operator health; no automatic renewal is scheduled. <a className={documentLink} href="https://github.com/open-covenant/covenant/blob/6483edca0eaa5476d72f84f8da571df4d4dab7b9/docs/evidence/knowledge/stake-extension-2026-10-21/report.md">Read stake extension ↗</a></p>
            <p className={styles.small}>
              <a className={documentLink} href="https://github.com/open-covenant/covenant/blob/compute-v0.5.0/docs/evidence/knowledge/benchmark-release-2026-10-20/report.md">Optimized preview measurements ↗</a><br />
              <a className={documentLink} href="https://github.com/open-covenant/covenant/blob/compute-v0.5.0/docs/evidence/knowledge/benchmark-2026-10-20/report.md">Earlier debug measurements ↗</a><br />
              Both runs used the same small fixture. Direct search returned fewer bytes in all seven matching cases. Knowledge had lower median command latency in the optimized run; direct search was faster in the debug run. The optimized run overlapped regression tests. These measurements establish neither production performance nor token savings. Build-time reports from the release source.
            </p>
            <p className={styles.small}>Two earlier paid pilot jobs established receipt, snapshot and interrupted-client recovery evidence. A subsequent job on the dedicated operator passed receipt, artifact/import and settlement checks. Its identical-key settled replay returned the same job without another charge; this later run tested normal completion and settled replay. <a className={documentLink} href="https://github.com/open-covenant/covenant/blob/e5745c920e5ad17b0af16ce20478bd1cce77e0e9/docs/evidence/knowledge/paid-pilot-2026-10-20/report.md">Read the bounded pilot evidence ↗</a></p>
          </div>
          <dl className={styles.statusList}>
            <div><dt>Local workflow</dt><dd>Available in Compute 0.5.1</dd></div>
            <div><dt>Release package</dt><dd>Signed macOS + Linux builds</dd></div>
            <div><dt>Remote compiler</dt><dd>Limited capacity · one operator</dd></div>
            <div><dt>Measured results</dt><dd>Reproducible fixture reports</dd></div>
          </dl>
        </section>
        <section className={styles.closing} aria-label="Follow the rollout">
          <p className={styles.eyebrow}>Covenant Compute / Knowledge</p>
          <h2 className={display.className}>Build context.<br />Keep the evidence.</h2>
          <a className="btn btn-primary" href="https://x.com/OpenCovenant" target="_blank" rel="noopener noreferrer">Follow development <span aria-hidden="true">↗</span></a>
        </section>
      </main>
      <SiteFooter />
    </>
  );
}
