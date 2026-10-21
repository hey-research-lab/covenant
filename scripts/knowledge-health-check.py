#!/usr/bin/env python3
"""Observe public Knowledge/Kani health and emit changed conditions as JSON.

GET only; no credentials, purchases, signing, service control or notification
delivery. State is private and outside the checkout. Unix Python 3.10+ required.
"""
from __future__ import annotations

import argparse
import contextlib
import datetime as dt
import fcntl
import hashlib
import http.client
import json
import os
from pathlib import Path
import re
import signal
import ssl
import stat
import sys
import time
import uuid

OPERATOR = "CfaV6nB59aDnTJccD5WXUiQejg9prMvtQrkFqQZRhWmf"
KNOWLEDGE = "knowledge-compile-v1:d011ce03baa42e825e345bd028c4b9af82e210535958c4ce5b49eb8a0503745c"
KANI = "kani-v1:2bf73b40a77f602c334527fa79b6d1bc903c7a92394b4ab02e3889eeed9cda91"
DEFAULT_CUTOFF = "2026-11-17T22:10:13Z"
TIMEOUT = 8
MAX_STATE = 256 * 1024
COUNTERS = ("released", "faults", "canary_passed", "canary_failed", "disputed",
            "redundancy_agreed", "redundancy_disagreed")
FAULT_COUNTERS = ("faults", "canary_failed", "disputed", "redundancy_disagreed")
ENDPOINTS = {
    "capacity": ("compute-api.opencovenant.org", "/federation/capacity", 256 * 1024, "json"),
    "reputation": ("compute-api.opencovenant.org", f"/federation/operators/{OPERATOR}/reputation", 64 * 1024, "json"),
    "health": ("compute-api.opencovenant.org", "/health", 64 * 1024, "json"),
    "page": ("opencovenant.org", "/compute-knowledge", 1024 * 1024, "html"),
    "guide": ("raw.githubusercontent.com", "/open-covenant/covenant/compute-v0.5.1/docs/compute-knowledge-remote.md", 128 * 1024, "text"),
}
IDENTITY = hashlib.sha256(json.dumps({"operator": OPERATOR, "models": [KNOWLEDGE, KANI],
                                    "endpoints": ENDPOINTS}, sort_keys=True).encode()).hexdigest()
SIGNALS = tuple(name + "_observation" for name in ENDPOINTS) + (
    "knowledge_capacity", "kani_capacity", "knowledge_operator")


class Refusal(Exception):
    """Fixed diagnostics; never include remote bodies or secret environment data."""


def require(value, message):
    if not value:
        raise Refusal(message)


def natural(value):
    return type(value) is int and 0 <= value <= 2**64 - 1


def parse_json(blob):
    def unique(pairs):
        result = {}
        for key, value in pairs:
            require(key not in result, "duplicate_json_key")
            result[key] = value
        return result

    def invalid(_value):
        raise Refusal("invalid_json_number")

    try:
        return json.loads(blob.decode("utf-8"), object_pairs_hook=unique,
                          parse_constant=invalid)
    except (ValueError, UnicodeError, RecursionError):
        raise Refusal("invalid_json") from None


def utc(seconds):
    return dt.datetime.fromtimestamp(seconds, dt.timezone.utc).isoformat().replace("+00:00", "Z")


def cutoff_timestamp(value):
    require(bool(re.fullmatch(r"\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z", value)), "cutoff_must_be_utc_seconds")
    try:
        return int(dt.datetime.fromisoformat(value.replace("Z", "+00:00")).timestamp())
    except ValueError:
        raise Refusal("invalid_cutoff_date") from None


@contextlib.contextmanager
def deadline():
    def expired(_signum, _frame):
        raise Refusal("timeout")
    old = signal.signal(signal.SIGALRM, expired)
    signal.setitimer(signal.ITIMER_REAL, TIMEOUT)
    try:
        yield
    finally:
        signal.setitimer(signal.ITIMER_REAL, 0)
        signal.signal(signal.SIGALRM, old)


def fetch(name):
    """Bound DNS/TLS/headers/body, reject redirects and never consult proxies."""
    host, route, maximum, kind = ENDPOINTS[name]
    connection = None
    try:
        with deadline():
            connection = http.client.HTTPSConnection(host, timeout=TIMEOUT,
                                                     context=ssl.create_default_context())
            connection.request("GET", route, headers={
                "User-Agent": "covenant-knowledge-preactivation/1",
                "Accept": "application/json" if kind == "json" else "text/html, text/plain",
                "Accept-Encoding": "identity",
            })
            response = connection.getresponse()
            require(response.status == 200, "http_" + str(response.status))
            lengths = response.headers.get_all("Content-Length", [])
            encodings = response.headers.get_all("Content-Encoding", [])
            require(len(lengths) <= 1 and len(encodings) <= 1, "ambiguous_headers")
            require(not encodings or encodings[0].lower() == "identity", "unexpected_content_encoding")
            expected = None
            if lengths:
                require(bool(re.fullmatch(r"[0-9]{1,9}", lengths[0])), "invalid_content_length")
                expected = int(lengths[0])
                require(expected <= maximum and not response.headers.get("Transfer-Encoding"), "oversized_or_ambiguous_length")
            body = bytearray()
            while True:
                chunk = response.read1(maximum + 1 - len(body))
                if not chunk:
                    break
                body.extend(chunk)
                require(len(body) <= maximum, "response_too_large")
            require(expected is None or len(body) == expected, "truncated_response")
            content_type = response.headers.get("Content-Type", "").lower().split(";")[0]
            require(content_type in ({"application/json"} if kind == "json" else
                                     {"text/html"} if kind == "html" else {"text/plain"}), "unexpected_content_type")
            return bytes(body)
    except (TimeoutError, OSError, http.client.HTTPException):
        raise Refusal("network_or_tls_failure") from None
    finally:
        if connection is not None:
            connection.close()


def normalize(name, blob):
    if name in ("page", "guide"):
        expected = (b"Covenant", b"Knowledge") if name == "page" else (b"# Remote Knowledge compilation",)
        require(all(part in blob for part in expected), "unexpected_document")
        return {"available": True}
    value = parse_json(blob)
    require(isinstance(value, dict), "expected_json_object")
    if name == "health":
        require(value.get("status") == "ok", "coordinator_health_not_ok")
        revision = value.get("revision")
        require(isinstance(revision, str) and bool(re.fullmatch(r"[0-9a-f]{40}", revision)), "invalid_revision")
        return {"revision": revision}
    if name == "capacity":
        rows = value.get("entries")
        require(isinstance(rows, list) and len(rows) <= 2048, "invalid_capacity_entries")
        selected = {}
        for row in rows:
            require(isinstance(row, dict) and isinstance(row.get("model"), str)
                    and len(row["model"]) <= 512 and isinstance(row.get("kind"), str)
                    and natural(row.get("operators")), "malformed_capacity_row")
            if row["model"] not in (KNOWLEDGE, KANI):
                continue
            require(row["model"] not in selected, "duplicate_exact_model")
            require(row["kind"] == ("batch_job" if row["model"] == KNOWLEDGE else "agent_check")
                    and row.get("min_ask_unit") == "per_job"
                    and natural(row.get("min_ask_micro_usdc"))
                    and natural(row.get("max_ask_micro_usdc"))
                    and row["min_ask_micro_usdc"] <= row["max_ask_micro_usdc"], "invalid_exact_model_contract")
            selected[row["model"]] = {key: row[key] for key in (
                "kind", "model", "operators", "min_ask_micro_usdc", "max_ask_micro_usdc", "min_ask_unit")}
        return selected
    require(value.get("operator_pubkey_b58") == OPERATOR, "operator_identity_mismatch")
    require(all(type(value.get(key)) is bool for key in ("registered", "staked", "live", "matchable")), "invalid_operator_flags")
    require(all(natural(value.get(key)) for key in COUNTERS), "invalid_operator_counters")
    require("seen_ms_ago" in value and (value["seen_ms_ago"] is None or natural(value["seen_ms_ago"])), "invalid_heartbeat_age")
    require(isinstance(value.get("status"), str) and bool(re.fullmatch(r"[a-z_]{1,40}", value["status"])), "invalid_operator_status")
    return {key: value[key] for key in (*COUNTERS, "registered", "staked", "live", "matchable", "status", "seen_ms_ago")}


def observe(fetcher=fetch):
    observations = {}
    for name, (host, route, _maximum, _kind) in ENDPOINTS.items():
        item = {"url": "https://" + host + route}
        try:
            blob = fetcher(name)
            item.update(ok=True, value=normalize(name, blob), sha256=hashlib.sha256(blob).hexdigest())
        except Refusal as error:
            item.update(ok=False, classification="observation_failed", reason=str(error))
        observations[name] = item
    return observations


def initial_state():
    return {"schema": 1, "identity": IDENTITY, "last_run": None, "sequence": 0,
            "signals": {}, "counter_baseline": None, "cutoff": None, "cutoff_stage": None,
            "last_report": None}


def validate_state(state):
    require(isinstance(state, dict) and state.get("schema") == 1
            and state.get("identity") == IDENTITY, "state_identity_or_schema_mismatch")
    require(set(state) == {"schema", "identity", "last_run", "sequence", "signals",
                           "counter_baseline", "cutoff", "cutoff_stage", "last_report"}, "invalid_state_fields")
    require(state.get("last_run") is None or natural(state["last_run"]), "invalid_state_timestamp")
    require(natural(state.get("sequence")) and isinstance(state.get("signals"), dict), "invalid_state_signals")
    require(all(key in SIGNALS and value in ("ok", "bad") for key, value in state["signals"].items()), "invalid_saved_signal")
    require(state.get("cutoff") is None or natural(state["cutoff"]), "invalid_saved_cutoff")
    require(state.get("cutoff_stage") in (None, "clear", "7d", "72h", "24h", "expired"), "invalid_saved_cutoff_stage")
    baseline = state.get("counter_baseline")
    if baseline is not None:
        require(isinstance(baseline, dict) and isinstance(baseline.get("revision"), str)
                and bool(re.fullmatch(r"[0-9a-f]{40}", baseline["revision"]))
                and isinstance(baseline.get("values"), dict)
                and set(baseline["values"]) == set(COUNTERS)
                and all(natural(x) for x in baseline["values"].values()), "invalid_saved_counters")
    report = state["last_report"]
    require(report is None or (isinstance(report, dict)
            and report.get("schema") == "covenant.knowledge.health-observation.v1"
            and type(report.get("notify")) is bool and isinstance(report.get("events"), list)), "invalid_saved_report")


def cutoff_stage(remaining):
    for seconds, stage in ((0, "expired"), (86400, "24h"), (259200, "72h"), (604800, "7d")):
        if remaining <= seconds:
            return stage
    return "clear"


def transition(state, observations, timestamp, cutoff):
    validate_state(state)
    require(natural(timestamp) and natural(cutoff), "invalid_clock_or_cutoff")
    require(state["last_run"] is None or timestamp >= state["last_run"], "clock_moved_backwards")
    state = json.loads(json.dumps(state))
    first = state["last_run"] is None
    events, signals = [], {}

    def event(kind, subject, severity, details=None):
        state["sequence"] += 1
        events.append({"id": IDENTITY[:16] + ":" + str(state["sequence"]), "kind": kind,
                       "subject": subject, "severity": severity, "details": details or {}})

    def signal_state(name, current, failure_kind, details=None):
        previous = state["signals"].get(name)
        if current is None:
            signals[name] = {"status": "unknown", "last_confirmed": previous}
            return
        if current != previous:
            if current == "bad":
                event(failure_kind, name, "warning", details)
            elif previous == "bad":
                event("recovered", name, "notice")
        state["signals"][name] = current
        signals[name] = {"status": current}

    for name in ENDPOINTS:
        item = observations[name]
        signal_state(name + "_observation", "ok" if item["ok"] else "bad",
                     "observation_failed", {"reason": item.get("reason", "")})
    capacity = observations["capacity"]
    for subject, model in (("knowledge_capacity", KNOWLEDGE), ("kani_capacity", KANI)):
        row = capacity.get("value", {}).get(model) if capacity["ok"] else None
        current = ("ok" if row and row["operators"] > 0 else "bad") if capacity["ok"] else None
        signal_state(subject, current, "confirmed_capacity_missing", {"model": model,
                     "meaning": "Exact-model capacity not advertised; not proof of host failure."})
    reputation = observations["reputation"]
    flags = ("registered", "staked", "live", "matchable")
    operator_ok = all(reputation["value"][key] for key in flags) if reputation["ok"] else None
    signal_state("knowledge_operator", None if operator_ok is None else "ok" if operator_ok else "bad",
                 "operator_not_matchable", {"false_flags": [key for key in flags if reputation["ok"] and not reputation["value"][key]]})
    health = observations["health"]
    if reputation["ok"] and health["ok"]:
        values = {key: reputation["value"][key] for key in COUNTERS}
        revision = health["value"]["revision"]
        previous = state["counter_baseline"]
        if previous is not None:
            if revision != previous["revision"]:
                event("counter_baseline_reset", "coordinator_revision_changed", "warning",
                      {"previous_revision": previous["revision"], "revision": revision,
                       "meaning": "New baseline; no retroactive fault delta inferred."})
            elif any(values[key] < previous["values"][key] for key in COUNTERS):
                event("counter_baseline_reset", "public_counter_decreased", "warning",
                      {"decreased": [key for key in COUNTERS if values[key] < previous["values"][key]],
                       "meaning": "Reset or inconsistent observation; not recovery or negative faults."})
            else:
                changes = {key: {"before": previous["values"][key], "after": values[key],
                                  "delta": values[key] - previous["values"][key]}
                           for key in FAULT_COUNTERS if values[key] > previous["values"][key]}
                if changes:
                    event("public_fault_counters_increased", "knowledge_operator", "warning",
                          {"changes": changes, "overlap": "Breakout counters overlap faults; do not sum them."})
        state["counter_baseline"] = {"revision": revision, "values": values}
    stage = cutoff_stage(cutoff - timestamp)
    if state["cutoff"] is not None and state["cutoff"] != cutoff:
        event("configured_cutoff_changed", "stake_admission", "notice",
              {"previous_cutoff": utc(state["cutoff"]), "cutoff": utc(cutoff),
               "meaning": "Local configuration change; no chain verification or extension performed."})
        state["cutoff_stage"] = None
    if stage != state["cutoff_stage"] and stage != "clear":
        event("stake_admission_expired" if stage == "expired" else "stake_admission_reminder",
              "stake_admission", "critical" if stage == "expired" else "warning",
              {"stage": stage, "cutoff_utc": utc(cutoff), "remaining_seconds": max(0, cutoff - timestamp)})
    state.update(last_run=timestamp, cutoff=cutoff, cutoff_stage=stage)
    report = {"schema": "covenant.knowledge.health-observation.v1", "observed_at": utc(timestamp),
              "initialized": first, "notify": bool(events), "events": events,
              "signals": signals, "observations": observations,
              "stake_admission": {"configured_cutoff": utc(cutoff), "stage": stage,
                                  "remaining_seconds": max(0, cutoff - timestamp)},
              "coverage": "Public advertised capacity and Cfa reputation only; no job submission, host telemetry, queue-depth, private failure metrics or automatic renewal."}
    state["last_report"] = report
    return state, report


@contextlib.contextmanager
def state_lock(directory):
    require(directory.is_absolute(), "state_directory_must_be_absolute")
    require(not directory.is_symlink(), "state_directory_symlink")
    resolved = directory.resolve()
    require(not any((parent / ".git").exists() for parent in (resolved, *resolved.parents)), "state_must_be_outside_repository")
    directory.mkdir(parents=True, exist_ok=True, mode=0o700)
    metadata = directory.stat()
    require(stat.S_ISDIR(metadata.st_mode) and metadata.st_mode & 0o077 == 0, "state_directory_must_be_private")
    require(set(x.name for x in directory.iterdir()) <= {"state.json", "monitor.lock"}, "state_directory_contains_unrelated_files")
    fd = os.open(directory / "monitor.lock", os.O_RDWR | os.O_CREAT | os.O_NOFOLLOW | os.O_NONBLOCK, 0o600)
    try:
        mode = os.fstat(fd).st_mode
        require(stat.S_ISREG(mode) and mode & 0o077 == 0, "lock_must_be_private_regular")
        try:
            fcntl.flock(fd, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except BlockingIOError:
            raise Refusal("another_check_is_running") from None
        yield
    finally:
        os.close(fd)


def load_state(directory):
    file = directory / "state.json"
    if not file.exists() and not file.is_symlink():
        return initial_state()
    fd = os.open(file, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK)
    try:
        metadata = os.fstat(fd)
        require(stat.S_ISREG(metadata.st_mode) and metadata.st_mode & 0o077 == 0, "state_file_must_be_private_regular")
        with os.fdopen(fd, "rb", closefd=False) as stream:
            blob = stream.read(MAX_STATE + 1)
        require(len(blob) <= MAX_STATE, "state_too_large")
        state = parse_json(blob)
        validate_state(state)
        return state
    finally:
        os.close(fd)


def save_state(directory, state):
    blob = (json.dumps(state, sort_keys=True, indent=2) + "\n").encode()
    require(len(blob) <= MAX_STATE, "state_too_large")
    temporary = directory / (".state-" + uuid.uuid4().hex)
    fd = os.open(temporary, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    try:
        with os.fdopen(fd, "wb") as stream:
            stream.write(blob)
            stream.flush()
            os.fsync(stream.fileno())
        os.replace(temporary, directory / "state.json")
        parent = os.open(directory, os.O_RDONLY)
        try:
            os.fsync(parent)
        finally:
            os.close(parent)
    finally:
        if temporary.exists():
            temporary.unlink()


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--state-dir", type=Path, default=Path.home() / ".local/state/covenant/knowledge-health")
    parser.add_argument("--admission-cutoff", default=DEFAULT_CUTOFF, help="Reviewed UTC cutoff; never extended automatically")
    parser.add_argument("--show-last", action="store_true", help="Read the last durable report without network calls")
    args = parser.parse_args(argv)
    try:
        cutoff = cutoff_timestamp(args.admission_cutoff)
        with state_lock(args.state_dir):
            state = load_state(args.state_dir)
            if args.show_last:
                require(state["last_report"] is not None, "no_saved_report")
                report = state["last_report"]
            else:
                observations = observe()
                state, report = transition(state, observations, int(time.time()), cutoff)
                save_state(args.state_dir, state)
        print(json.dumps(report, indent=2, sort_keys=True))
        return 0
    except (Refusal, OSError) as error:
        # Local state failure is not healthy, missing capacity, or a new baseline.
        print(json.dumps({"schema": "covenant.knowledge.health-checker-error.v1",
                          "notify": True, "classification": "checker_error",
                          "automatic_state_reset": False,
                          "reason": str(error) if isinstance(error, Refusal) else "local_io_failure",
                          "message": "Check could not safely read or persist its local state; inspect locally. No automatic reset."}))
        return 2


if __name__ == "__main__":
    sys.exit(main())
