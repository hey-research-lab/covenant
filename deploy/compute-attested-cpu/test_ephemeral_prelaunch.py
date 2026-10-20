"""Exercise hostile unmeasured inputs without executing guest system changes."""

import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest


class EphemeralPrelaunchInputTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        script = Path(__file__).with_name("ephemeral-pre-launch.sh").read_text()
        cls.functions = script.split("\ncovenant_prelaunch_main() {", 1)[0]

    def check_config(self, value):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "user-config.json"
            path.write_text(json.dumps(value))
            return subprocess.run(
                ["bash", "-c", self.functions + '\ncovenant_check_user_config "$1"',
                 "prelaunch-input-test", str(path)],
                capture_output=True, text=True, check=False,
            )

    def test_account_root_credentials_are_rejected(self):
        for value in (
            {"ssh_authorized_keys": "ssh-ed25519 attacker"},
            {"ssh_authorized_keys": ["attacker"]},
            {"root_password": "test-private-value"},
            {"root_public_key": "attacker"},
            ["not an object"],
        ):
            with self.subTest(value=value):
                result = self.check_config(value)
                self.assertNotEqual(result.returncode, 0)
                self.assertNotIn("test-private-value", result.stdout + result.stderr)

    def test_empty_preferences_are_accepted_without_importing_shell_content(self):
        for value in ({}, {"ssh_authorized_keys": ""}, {"ssh_authorized_keys": []}):
            self.assertEqual(self.check_config(value).returncode, 0)
        with tempfile.TemporaryDirectory() as directory:
            marker = Path(directory) / "must-not-exist"
            self.assertEqual(self.check_config({
                "default_gateway_domain": f"$(touch {marker})",
                "opaque_command": f"touch {marker}",
            }).returncode, 0)
            self.assertFalse(marker.exists())

    def test_inherited_root_credentials_refuse_without_echoing_values(self):
        for name in ("DSTACK_ROOT_PASSWORD", "DSTACK_ROOT_PUBLIC_KEY", "DSTACK_AUTHORIZED_KEYS"):
            env = os.environ.copy()
            env[name] = "test-private-value"
            result = subprocess.run(
                ["bash", "-c", self.functions + "\ncovenant_check_root_env"],
                capture_output=True, text=True, check=False, env=env,
            )
            self.assertNotEqual(result.returncode, 0)
            self.assertNotIn("test-private-value", result.stdout + result.stderr)


if __name__ == "__main__":
    unittest.main()
