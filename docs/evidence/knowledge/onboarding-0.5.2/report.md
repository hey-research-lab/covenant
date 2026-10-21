# Knowledge 0.5.2 development validation

These checks establish the local candidate behavior. Signed multi-platform
publication, distribution promotion and any remote paid acceptance are separate
gates. This work submits no paid job and changes no operator deployment or stake.

Guided `knowledge setup` compiles explicitly, verifies a captured citation and
emits an absolute executable/store configuration bound to one project. It leaves
client settings unchanged. CLI status, existing MCP status and the loopback
viewer report current, stale or unavailable source without advancing history or
writing the parser cache. Current means admitted source and supported capture
policy matched at observation time, not whole-program correctness.

## Candidate checks

- [Focused validation](focused-validation.json): 28 core tests, nine CLI/MCP/viewer
  tests, two mocked remote tests; strict Clippy for Knowledge and buyer on all
  targets. Existing remote enum storage is boxed in memory to satisfy Clippy;
  serialized contracts and payment behavior are unchanged.
- [Development package smoke](development-smoke.json): 25 checks and 19 exact
  citations; executes the emitted MCP arguments and exercises all six read-only
  tools, rejected mutation/project overrides, and current → stale → current.
- [Compatibility](compatibility.json): released 0.5.1 and candidate 0.5.2 produce
  identical canonical content for the original example; 0.5.2 verifies the older
  client's store. Parser versions, artifact schema and the deployed image
  capability remain unchanged.
- [Viewer checks](viewer-checks.json): actual loopback UI at 1440×1000 and
  390×844, all three freshness states, no JavaScript errors or horizontal
  overflow. Desktop and mobile screenshots were visually reviewed locally.
- [Health checker validation](health-validation.json): 37 offline tests for
  observation failures, confirmed capacity changes, recovery, counter resets,
  cutoff reminders and durable state. This public-data monitor cannot observe
  all private job failures or host metrics.

Independent review found an excluded-ignore-file edge case: rules could be
parsed from a NUL-containing file whose bytes were absent from captured evidence.
Freshness now reports unavailable for excluded ignore-policy evidence. Tests also
cover missing and replaced roots, bounded traversal, no Git invocation during
status, unchanged stored history/cache, Unicode truncation and CRLF/Markdown
line ranges. Evidence truncation now counts Unicode characters consistently with
the displayed fragment. A truncated excerpt does not claim full-range display.

The [operations guide](../../../compute-knowledge-operations.md) describes the
separate hourly app heartbeat, public observation limits and fixed admission
cutoff. The [Homebrew 0.5.1 candidate](../homebrew-0.5.1/report.md) is separately
tested and still requires repository write access or maintainer publication.
