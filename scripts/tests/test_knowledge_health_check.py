"""Offline transition, bounded transport and durable-state checks.

Public response fixtures are captured; mutated scenarios and document bodies
are test inputs, never presented as live service observations.
"""
import contextlib
import copy
from email.message import Message
import importlib.util
import io
import json
import os
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

SCRIPT = Path(__file__).resolve().parents[1] / "knowledge-health-check.py"
SPEC = importlib.util.spec_from_file_location("knowledge_health_check", SCRIPT)
check = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(check)
CAPTURE = json.loads((Path(__file__).parent / "fixtures/knowledge-health/captured.json").read_text())
CUTOFF = check.cutoff_timestamp(check.DEFAULT_CUTOFF)
NOW = CUTOFF - 20 * 86400


def observations(**overrides):
    bodies = {name: json.dumps(CAPTURE[name]).encode() for name in ("capacity", "reputation", "health")}
    bodies.update(page=b"<html>Covenant Knowledge fixture</html>", guide=b"# Remote Knowledge compilation\nFixture")
    bodies.update(overrides)

    def fetch(name):
        result = bodies[name]
        if isinstance(result, Exception):
            raise result
        return result
    return check.observe(fetch)


def changed(name, mutate):
    value = copy.deepcopy(CAPTURE[name])
    mutate(value)
    return observations(**{name: json.dumps(value).encode()})


class TransitionTests(unittest.TestCase):
    def setUp(self):
        self.state, self.report = check.transition(check.initial_state(), observations(), NOW, CUTOFF)

    def run_next(self, obs=None, timestamp=NOW + 1, cutoff=CUTOFF):
        self.state, self.report = check.transition(self.state, observations() if obs is None else obs, timestamp, cutoff)
        return self.report

    def test_first_healthy_observation_and_repeat_are_quiet(self):
        self.assertTrue(self.report["initialized"])
        self.assertFalse(self.report["notify"])
        self.assertFalse(self.run_next()["events"])

    def test_first_nonzero_faults_are_baseline_not_retroactive_alerts(self):
        obs = changed("reputation", lambda value: value.update(faults=4, canary_failed=2))
        state, report = check.transition(check.initial_state(), obs, NOW, CUTOFF)
        self.assertFalse(report["notify"])
        self.assertEqual(state["counter_baseline"]["values"]["faults"], 4)

    def test_missing_exact_row_warns_once_then_recovers_once(self):
        missing = changed("capacity", lambda value: value["entries"].pop(0))
        events = self.run_next(missing)["events"]
        self.assertEqual([(x["kind"], x["subject"]) for x in events], [("confirmed_capacity_missing", "knowledge_capacity")])
        self.assertFalse(self.run_next(missing, NOW + 2)["notify"])
        self.assertEqual(self.run_next(timestamp=NOW + 3)["events"][0]["kind"], "recovered")
        self.assertFalse(self.run_next(timestamp=NOW + 4)["notify"])

    def test_zero_capacity_is_confirmed_missing(self):
        obs = changed("capacity", lambda value: value["entries"][1].update(operators=0))
        self.assertEqual(self.run_next(obs)["events"][0]["subject"], "kani_capacity")

    def test_network_failure_is_unknown_not_missing_or_recovery(self):
        missing = changed("capacity", lambda value: value["entries"].clear())
        self.run_next(missing)
        failure = observations(capacity=check.Refusal("timeout"))
        report = self.run_next(failure, NOW + 2)
        self.assertEqual([x["kind"] for x in report["events"]], ["observation_failed"])
        self.assertEqual(report["signals"]["knowledge_capacity"], {"status": "unknown", "last_confirmed": "bad"})
        self.assertFalse(self.run_next(failure, NOW + 3)["notify"])
        self.assertEqual(len(self.run_next(timestamp=NOW + 4)["events"]), 3)

    def test_operator_not_matchable_and_recovery(self):
        obs = changed("reputation", lambda value: value.update(live=False, matchable=False))
        event = self.run_next(obs)["events"][0]
        self.assertEqual(event["kind"], "operator_not_matchable")
        self.assertEqual(event["details"]["false_flags"], ["live", "matchable"])
        self.assertFalse(self.run_next(obs, NOW + 2)["notify"])
        self.assertEqual(self.run_next(timestamp=NOW + 3)["events"][0]["kind"], "recovered")

    def test_overlapping_counter_increases_are_one_event_not_summed(self):
        obs = changed("reputation", lambda value: value.update(faults=2, canary_failed=1, disputed=1))
        report = self.run_next(obs)
        self.assertEqual(len(report["events"]), 1)
        delta = report["events"][0]["details"]["changes"]
        self.assertEqual(delta["faults"]["delta"], 2)
        self.assertEqual(delta["canary_failed"]["delta"], 1)
        self.assertFalse(self.run_next(obs, NOW + 2)["notify"])

    def test_released_increase_is_recorded_quietly(self):
        self.assertFalse(self.run_next(changed("reputation", lambda value: value.update(released=2)))["notify"])
        self.assertEqual(self.state["counter_baseline"]["values"]["released"], 2)

    def test_counter_decrease_is_explicit_reset_then_new_baseline(self):
        self.run_next(changed("reputation", lambda value: value.update(faults=4)))
        reset = self.run_next(timestamp=NOW + 2)
        self.assertEqual(reset["events"][0]["kind"], "counter_baseline_reset")
        self.assertEqual(reset["events"][0]["subject"], "public_counter_decreased")
        self.assertFalse(self.run_next(timestamp=NOW + 3)["notify"])
        event = self.run_next(changed("reputation", lambda value: value.update(faults=1)), NOW + 4)["events"][0]
        self.assertEqual(event["details"]["changes"]["faults"]["delta"], 1)

    def test_revision_change_rebaselines_without_inferred_delta(self):
        obs = changed("reputation", lambda value: value.update(faults=4))
        obs["health"]["value"]["revision"] = "a" * 40
        events = self.run_next(obs)["events"]
        self.assertEqual([x["kind"] for x in events], ["counter_baseline_reset"])
        self.assertFalse(self.run_next(obs, NOW + 2)["notify"])

    def test_failed_health_preserves_counter_baseline(self):
        obs = changed("reputation", lambda value: value.update(faults=1))
        obs["health"] = observations(health=check.Refusal("http_503"))["health"]
        self.run_next(obs)
        self.assertEqual(self.state["counter_baseline"]["values"]["faults"], 0)
        obs["health"] = observations()["health"]
        kinds = [x["kind"] for x in self.run_next(obs, NOW + 2)["events"]]
        self.assertEqual(kinds, ["recovered", "public_fault_counters_increased"])

    def test_document_failure_and_recovery_are_edge_triggered(self):
        obs = observations(page=b"<html>Unexpected page</html>")
        self.assertEqual(self.run_next(obs)["events"][0]["subject"], "page_observation")
        self.assertFalse(self.run_next(obs, NOW + 2)["notify"])
        self.assertEqual(self.run_next(timestamp=NOW + 3)["events"][0]["kind"], "recovered")

    def test_cutoff_exact_boundaries_and_unchanged_suppression(self):
        for seconds, stage in ((604801, "clear"), (604800, "7d"), (259201, "7d"), (259200, "72h"), (86401, "72h"), (86400, "24h"), (1, "24h"), (0, "expired"), (-1, "expired")):
            with self.subTest(seconds=seconds):
                self.assertEqual(check.cutoff_stage(seconds), stage)
        for offset, stage in ((604800, "7d"), (259200, "72h"), (86400, "24h"), (0, "expired")):
            report = self.run_next(timestamp=CUTOFF - offset)
            self.assertEqual(report["events"][0]["details"]["stage"], stage)
            self.assertFalse(self.run_next(timestamp=CUTOFF - offset)["notify"])

    def test_late_first_run_emits_only_most_urgent_reminder(self):
        _, report = check.transition(check.initial_state(), observations(), CUTOFF - 100, CUTOFF)
        self.assertEqual(len(report["events"]), 1)
        self.assertEqual(report["events"][0]["details"]["stage"], "24h")

    def test_cutoff_reconfiguration_is_explicit_not_chain_proof(self):
        self.run_next(timestamp=CUTOFF - 1)
        report = self.run_next(timestamp=CUTOFF, cutoff=CUTOFF + 30 * 86400)
        self.assertEqual([x["kind"] for x in report["events"]], ["configured_cutoff_changed"])
        self.assertEqual(report["stake_admission"]["stage"], "clear")

    def test_backward_clock_refuses_without_mutating_state(self):
        before = copy.deepcopy(self.state)
        with self.assertRaisesRegex(check.Refusal, "clock_moved_backwards"):
            self.run_next(timestamp=NOW - 1)
        self.assertEqual(self.state, before)

    def test_event_ids_stable_in_saved_report_and_increase(self):
        first = self.run_next(observations(page=check.Refusal("http_503")))["events"][0]["id"]
        self.assertEqual(self.state["last_report"]["events"][0]["id"], first)
        second = self.run_next(timestamp=NOW + 2)["events"][0]["id"]
        self.assertEqual(int(second.split(":")[1]), int(first.split(":")[1]) + 1)


class SchemaTests(unittest.TestCase):
    def test_malformed_json_is_unknown(self):
        for blob in (b"{", b"[]", b'{"entries":NaN}', b'{"entries":[],"entries":[]}', b"\xff"):
            with self.subTest(blob=blob):
                obs = observations(capacity=blob)
                _, report = check.transition(check.initial_state(), obs, NOW, CUTOFF)
                self.assertEqual(report["signals"]["knowledge_capacity"]["status"], "unknown")
                self.assertEqual(report["events"][0]["kind"], "observation_failed")

    def test_bad_exact_capacity_schema_is_unknown(self):
        for key, value in (("operators", True), ("operators", -1), ("operators", 1.1), ("kind", "inference"), ("min_ask_unit", "per_token"), ("min_ask_micro_usdc", -1), ("max_ask_micro_usdc", 1)):
            with self.subTest(key=key, value=value):
                self.assertFalse(changed("capacity", lambda data: data["entries"][0].update({key: value}))["capacity"]["ok"])

    def test_duplicate_exact_model_is_rejected(self):
        obs = changed("capacity", lambda data: data["entries"].append(data["entries"][0]))
        self.assertFalse(obs["capacity"]["ok"])

    def test_other_model_does_not_substitute_for_pinned_model(self):
        obs = changed("capacity", lambda data: data["entries"][0].update(model="knowledge-compile-v1:" + "a" * 64))
        _, report = check.transition(check.initial_state(), obs, NOW, CUTOFF)
        self.assertEqual(report["events"][0]["kind"], "confirmed_capacity_missing")

    def test_reputation_bad_fields_rejected(self):
        for key, value in (("operator_pubkey_b58", "wrong"), ("faults", -1), ("faults", True), ("staked", 1), ("status", "invalid value"), ("seen_ms_ago", -1)):
            with self.subTest(key=key):
                self.assertFalse(changed("reputation", lambda data: data.update({key: value}))["reputation"]["ok"])
        self.assertFalse(changed("reputation", lambda data: data.pop("seen_ms_ago"))["reputation"]["ok"])

    def test_health_schema_rejected(self):
        for values in ({"status": "fail"}, {"revision": "main"}, {"revision": None}):
            self.assertFalse(changed("health", lambda data: data.update(values))["health"]["ok"])

    def test_cutoff_timezone_and_impossible_dates_rejected(self):
        self.assertEqual(check.utc(CUTOFF), "2026-11-17T22:10:13Z")
        for value in ("2026-11-17", "2026-11-17T23:10:13+01:00", "2026-02-30T22:10:13Z"):
            with self.assertRaises(check.Refusal):
                check.cutoff_timestamp(value)


class FakeResponse:
    def __init__(self, body=b'{"entries":[]}', status=200, **headers):
        self.status = status
        self.headers = Message()
        self.headers["Content-Type"] = headers.pop("Content-Type", "application/json")
        for name, values in headers.items():
            for value in values if isinstance(values, list) else [values]:
                self.headers[name] = value
        self.body = io.BytesIO(body)

    def read1(self, maximum):
        return self.body.read(maximum)


class TransportTests(unittest.TestCase):
    def fetch(self, response):
        with patch.object(check.http.client, "HTTPSConnection") as cls:
            connection = cls.return_value
            connection.getresponse.return_value = response
            result = check.fetch("capacity")
            self.assertEqual(connection.request.call_args.args[0], "GET")
            self.assertNotIn("Authorization", connection.request.call_args.kwargs["headers"])
            connection.close.assert_called_once()
            return result

    def test_valid_bounded_get(self):
        self.assertEqual(self.fetch(FakeResponse(**{"Content-Length": "14"})), b'{"entries":[]}')

    def test_redirect_not_followed(self):
        with self.assertRaisesRegex(check.Refusal, "http_302"):
            self.fetch(FakeResponse(status=302))

    def test_network_failure_classified(self):
        with patch.object(check.http.client, "HTTPSConnection", side_effect=TimeoutError):
            with self.assertRaisesRegex(check.Refusal, "network_or_tls_failure"):
                check.fetch("capacity")

    def test_oversized_truncated_encoded_ambiguous_and_wrong_type_refused(self):
        cases = [FakeResponse(body=b"x" * (256 * 1024 + 1)),
                 FakeResponse(**{"Content-Length": "262145"}),
                 FakeResponse(**{"Content-Length": "20"}),
                 FakeResponse(**{"Content-Length": ["14", "14"]}),
                 FakeResponse(**{"Content-Length": "14", "Transfer-Encoding": "chunked"}),
                 FakeResponse(**{"Content-Encoding": "gzip"}),
                 FakeResponse(**{"Content-Type": "text/html"})]
        for response in cases:
            with self.subTest(headers=str(response.headers)):
                with self.assertRaises(check.Refusal):
                    self.fetch(response)


class StateTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.directory = Path(self.temporary.name) / "state"
        self.addCleanup(self.temporary.cleanup)

    def test_atomic_private_state_roundtrip_and_saved_report(self):
        with check.state_lock(self.directory):
            state, report = check.transition(check.load_state(self.directory), observations(), NOW, CUTOFF)
            check.save_state(self.directory, state)
            self.assertEqual(check.load_state(self.directory)["last_report"], report)
        self.assertEqual((self.directory / "state.json").stat().st_mode & 0o777, 0o600)
        self.assertEqual(self.directory.stat().st_mode & 0o777, 0o700)
        self.assertEqual(set(x.name for x in self.directory.iterdir()), {"state.json", "monitor.lock"})

    def test_corrupt_state_is_not_reset(self):
        with check.state_lock(self.directory):
            file = self.directory / "state.json"
            file.write_text("broken")
            file.chmod(0o600)
            with self.assertRaises(check.Refusal):
                check.load_state(self.directory)
            self.assertEqual(file.read_text(), "broken")

    def test_wrong_identity_schema_and_unknown_fields_rejected(self):
        for key, value in (("identity", "wrong"), ("schema", 2), ("new_field", "unexpected")):
            state = check.initial_state()
            state[key] = value
            with self.assertRaises(check.Refusal):
                check.validate_state(state)

    def test_relative_repository_and_unrelated_directories_refused(self):
        with self.assertRaises(check.Refusal), check.state_lock(Path("relative")):
            pass
        with self.assertRaises(check.Refusal), check.state_lock(SCRIPT.parent / "private-state"):
            pass
        self.directory.mkdir(mode=0o700)
        (self.directory / "unrelated").write_text("preserve")
        with self.assertRaises(check.Refusal), check.state_lock(self.directory):
            pass
        self.assertEqual((self.directory / "unrelated").read_text(), "preserve")

    def test_symlink_state_refused_without_touching_target(self):
        with check.state_lock(self.directory):
            target = Path(self.temporary.name) / "target"
            target.write_text("preserve")
            (self.directory / "state.json").symlink_to(target)
            with self.assertRaises(OSError):
                check.load_state(self.directory)
            self.assertEqual(target.read_text(), "preserve")

    def test_fifo_state_refused_without_blocking(self):
        with check.state_lock(self.directory):
            os.mkfifo(self.directory / "state.json", 0o600)
            with check.deadline(), self.assertRaisesRegex(check.Refusal, "state_file_must_be_private_regular"):
                check.load_state(self.directory)

    def test_concurrent_check_refuses_without_reset(self):
        with check.state_lock(self.directory):
            with self.assertRaisesRegex(check.Refusal, "another_check_is_running"), check.state_lock(self.directory):
                pass

    def test_failed_atomic_replace_preserves_existing_state(self):
        with check.state_lock(self.directory):
            original = check.initial_state()
            check.save_state(self.directory, original)
            new, _ = check.transition(original, observations(), NOW, CUTOFF)
            with patch.object(check.os, "replace", side_effect=OSError("fixture")):
                with self.assertRaises(OSError):
                    check.save_state(self.directory, new)
            self.assertEqual(check.load_state(self.directory), original)

    def test_main_persists_before_print_and_show_last_has_no_network(self):
        args = ["--state-dir", str(self.directory)]
        output = io.StringIO()
        with patch.object(check, "observe", return_value=observations()), patch.object(check.time, "time", return_value=NOW), contextlib.redirect_stdout(output):
            self.assertEqual(check.main(args), 0)
        self.assertEqual(json.loads(output.getvalue()), check.load_state(self.directory)["last_report"])
        with patch.object(check, "observe", side_effect=AssertionError("network forbidden")), contextlib.redirect_stdout(io.StringIO()):
            self.assertEqual(check.main(args + ["--show-last"]), 0)


if __name__ == "__main__":
    unittest.main()
