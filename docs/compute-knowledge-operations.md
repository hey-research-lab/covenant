# Knowledge operational observations

`scripts/knowledge-health-check.py` observes public Knowledge and Kani availability. It performs five HTTPS GETs and stores a private local baseline. It does not submit jobs, access wallet keys, sign transactions, extend stakes, change budgets, restart services, or deliver notifications. The calling automation decides whether to notify from its JSON output.

Run from the complete source checkout with Unix Python 3.10 or later:

```sh
python3 scripts/knowledge-health-check.py
```

The default state directory is `~/.local/state/covenant/knowledge-health`. Keep using the same directory across checks. To choose a different dedicated directory, supply an absolute path outside every Git checkout:

```sh
python3 scripts/knowledge-health-check.py --state-dir /absolute/private/knowledge-health
python3 scripts/knowledge-health-check.py --state-dir /absolute/private/knowledge-health --show-last
```

`--show-last` reads the last persisted report without network access. The checker does not create a recurring task. An hourly heartbeat was activated in the operator's Codex chat on 21 October 2026. It makes up to 120 public GETs per day while the host is available. This is a separate app automation, not something this script or a release installation creates. Scheduling depends on the host being available and does not provide continuous monitoring; a failure can remain undetected for up to the check interval, or longer while the host is unavailable.

## What it observes

| Source | Check | Limit |
| --- | --- | --- |
| [Public capacity](https://compute-api.opencovenant.org/federation/capacity) | Exact Knowledge and Kani models, correct capability and `per_job` pricing contract, positive advertised operator count | Aggregate capability rows do not identify a particular operator, free execution slots, queue depth, or host health. |
| [Dedicated Knowledge operator reputation](https://compute-api.opencovenant.org/federation/operators/CfaV6nB59aDnTJccD5WXUiQejg9prMvtQrkFqQZRhWmf/reputation) | Exact identity, registered/staked/live/matchable flags, public counters | These fields do not expose every job error, host restarts, out-of-memory events, or stake expiry. |
| [Coordinator health](https://compute-api.opencovenant.org/health) | Successful health response and source revision | A changed revision establishes a fresh counter baseline; it is not proof of a healthy execution path. |
| [Knowledge page](https://opencovenant.org/compute-knowledge) | HTTP 200, HTML, expected Covenant/Knowledge markers | Availability only; this does not verify every claim, link, layout, or installer asset. |
| [Tagged remote guide](https://raw.githubusercontent.com/open-covenant/covenant/compute-v0.5.1/docs/compute-knowledge-remote.md) | HTTP 200, text, expected document heading | The explicitly pinned guide is monitored; a newer release is not selected automatically. |

Pinned identities:

```text
Knowledge model: knowledge-compile-v1:d011ce03baa42e825e345bd028c4b9af82e210535958c4ce5b49eb8a0503745c
Knowledge operator: CfaV6nB59aDnTJccD5WXUiQejg9prMvtQrkFqQZRhWmf
Kani model: kani-v1:2bf73b40a77f602c334527fa79b6d1bc903c7a92394b4ab02e3889eeed9cda91
```

Kani is observed through aggregate exact-model capacity. The checker does not claim that this proves the original Kani host is healthy. These GETs are not a compile trial or paid acceptance; those remain separate evidence in [the validation record](compute-knowledge-validation.md).

## Alert behavior

Each run returns JSON containing `notify`, `events`, current `signals`, and the bounded `observations`. An ordinary completed check exits 0, including when it finds a service problem. The caller must read `notify`; process success alone does not mean the service is healthy.

| Event | Meaning |
| --- | --- |
| `observation_failed` | HTTP failure, timeout, TLS/network error, oversized response, or invalid schema. Dependent service state is **unknown**, not zero capacity or healthy. |
| `confirmed_capacity_missing` | A valid capacity document has no exact matching row or reports zero operators. This confirms missing advertised capacity at the observation time, not a host outage. |
| `operator_not_matchable` | At least one of the dedicated operator's registration, stake, live, or matchable flags is false. |
| `public_fault_counters_increased` | One or more public `faults`, `canary_failed`, `disputed`, or `redundancy_disagreed` counters increased within the same baseline. Breakout counters can overlap `faults`; do not sum them. |
| `counter_baseline_reset` | Coordinator revision changed or a public counter decreased. A new baseline is recorded explicitly; no negative fault delta or recovery is inferred. |
| `recovered` | A previously failed observation, missing capacity, or unmatchable operator is observed as healthy again. |
| `stake_admission_reminder` / `stake_admission_expired` | A configured stake-admission threshold was crossed. This is a time-based reminder, not a fresh chain query. |
| `configured_cutoff_changed` | An explicit local cutoff change was supplied. It does not prove that an extension was executed. |

The first valid reputation observation establishes the baseline quietly, even when historical faults are nonzero. New fault changes produce one event. Unchanged failures and unchanged reminder stages are suppressed; a recovery is emitted once. `released` and successful canary/redundancy counters are retained to detect resets, but their increases do not trigger an alert. Released results are not a count of all executions or evidence of demand.

After a failed fetch, the affected signal remains unknown and retains its last confirmed state. A failed observation cannot turn a previous outage into a recovery. Counters advance only when both reputation and coordinator health are valid. There is no public all-errors metric; undisclosed failures and outages entirely between checks may be missed.

## Stake reminders

The configured admission cutoff is **2026-11-17 22:10:13 UTC**, or **17 November 2026 23:10:13 Europe/Copenhagen**. The underlying lock expires two days later, on **19 November at 22:10:13 UTC**. These dates come from the [finalized stake-extension evidence](evidence/knowledge/stake-extension-2026-10-21/report.md), not from the reputation endpoint.

Reminders are emitted on the first run at or after seven days, 72 hours, and 24 hours before cutoff, then once at expiry. If monitoring starts late or misses multiple thresholds, only the most urgent current stage is emitted. The checker never renews the stake. After a separately authorized, finalized extension, review the chain evidence and explicitly configure the new cutoff:

```sh
python3 scripts/knowledge-health-check.py --admission-cutoff YYYY-MM-DDTHH:MM:SSZ
```

Do not change this date merely to silence an alert. The check refuses a backward local clock instead of resetting state.

## State and notification boundaries

The state directory must be private, dedicated to this checker, and outside a repository. It contains only `monitor.lock` and `state.json`. A nonblocking lock prevents overlapping runs; state is mode 0600 and replaced atomically after file and directory `fsync`. Invalid or mismatched state, symbolic-link state files, unrelated files, and unsafe permissions are refused without automatic reset. Network observations do not touch any buyer journal or financial state.

Every event has a stable local ID. The complete report is persisted before stdout. If the process stops after persistence but before the caller receives stdout, use `--show-last` **before another run** to recover that report. A later run replaces the saved report. This is local alert-state persistence, not exactly-once delivery; the calling automation should retain delivered event IDs and preserve uncertain delivery outcomes.

A checker-local error exits 2 and returns `classification: checker_error` with `notify: true`. Since a broken state store may be unable to persist deduplication, the caller must suppress identical repeated checker errors itself. Preserve the existing state while investigating; do not delete it to obtain a green result. A leftover `.state-*` file after abrupt termination is also a local error requiring inspection, not a reason to discard the baseline.

Requests use certificate-verified HTTPS with fixed public hosts, no credentials, no proxy environment, no redirects, no retries, an eight-second whole-request deadline, and bounded response sizes (64 KiB–1 MiB). Five sequential GETs take at most approximately 40 seconds plus local processing. Response bodies are reduced to selected public fields and hashes; no HTML, arbitrary server error text, or private data is persisted.

For a heartbeat, run the command once and notify only when `notify` is true or the checker cannot run. Describe failed observations as unknown, missing capacity as missing advertised capacity, and reminders as action-needed dates. Keep healthy and unchanged runs quiet. Human review is required for any remediation that would change funding, stake custody, service configuration, or deployment.

## Validation

```sh
python3 -m unittest discover -s scripts/tests -p test_knowledge_health_check.py -v
```

Tests use a selected-field capture from the [dedicated acceptance capacity record](evidence/knowledge/dedicated-acceptance-2026-10-21/capacity-after.json), plus explicitly synthetic mutations and document responses. They cover first baselines, edge-triggered failures/recovery, overlapping fault counters, malformed schemas, network errors, counter decreases and coordinator revisions, exact cutoff thresholds/expiry, and durable state refusal/atomic replacement. Tests submit no network requests or jobs.
