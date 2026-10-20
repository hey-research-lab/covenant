#!/usr/bin/env python3
"""Local Docker + real OpenSSH smoke. Fake guest evidence is never TEE acceptance."""
import argparse
import base64
import copy
import http.server
import json
import os
from pathlib import Path
import secrets
import socketserver
import subprocess
import tempfile
import threading
import time
import urllib.error
import urllib.request
import uuid

import collector as c
from test_collector import EVENTS, INFO, POLICY


def run(*arguments, check=True, env=None, input_text=None):
    result = subprocess.run(arguments, text=True, capture_output=True, env=env,
                            input=input_text, timeout=60)
    if check and result.returncode:
        # Do not print command arguments: environment inputs can be confidential.
        raise RuntimeError(f"{arguments[0]} failed: {result.stderr[-2000:]}")
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--image", required=True)
    args = parser.parse_args()
    prefix = "covenant-attested-smoke-" + uuid.uuid4().hex[:12]
    containers, volumes = [], []
    guest_calls = []
    token = secrets.token_hex(32)
    now = int(time.time() * 1000)
    directory_context = tempfile.TemporaryDirectory(prefix="cc-smoke-", dir="/tmp")
    directory = Path(directory_context.name)
    guest_server = None
    try:
        for name in ("buyer", "wrong-buyer"):
            run("ssh-keygen", "-q", "-t", "ed25519", "-N", "", "-C", "",
                "-f", str(directory / name))
        buyer_public = " ".join((directory / "buyer.pub").read_text().split()[:2])
        policy = copy.deepcopy(POLICY)
        policy.update(buyer_ssh_public_key=buyer_public, lease_expires_at_ms=now + 300_000)
        encoded = base64.b64encode(c.canonical(policy)).decode()
        process_env = dict(os.environ, COVENANT_COLLECTOR_TOKEN=token,
                           COVENANT_LEASE_POLICY_B64=encoded)

        class GuestHandler(http.server.BaseHTTPRequestHandler):
            def log_message(self, *_):
                pass

            def reply(self, value):
                raw = json.dumps(value).encode()
                self.send_response(200)
                self.send_header("Content-Length", str(len(raw)))
                self.end_headers()
                self.wfile.write(raw)

            def do_GET(self):
                if self.path != "/Info":
                    self.send_error(404)
                    return
                self.reply(INFO)

            def do_POST(self):
                body = c.strict_json(self.rfile.read(int(self.headers["Content-Length"])))
                if self.path != "/GetQuote":
                    self.send_error(404)
                    return
                guest_calls.append(body)
                self.reply({"quote": "abcd", "report_data": body["report_data"],
                            "event_log": EVENTS, "vm_config": INFO["vm_config"]})

        guest_socket = str(directory / "guest.sock")
        guest_server = socketserver.UnixStreamServer(guest_socket, GuestHandler)
        os.chmod(guest_socket, 0o666)
        guest_thread = threading.Thread(target=guest_server.serve_forever, daemon=True)
        guest_thread.start()
        for suffix in ("public", "workspace", "state"):
            name = prefix + "-" + suffix
            run("docker", "volume", "create", name)
            volumes.append(name)
        common = [
            "--read-only", "--restart", "no", "--cap-drop", "ALL",
            "--security-opt", "no-new-privileges:true",
            "--env", "COVENANT_LEASE_POLICY_B64",
        ]
        ssh_name = prefix + "-ssh"
        containers.append(ssh_name)
        run("docker", "run", "-d", "--name", ssh_name, *common,
            "--user", "0:0", "--cap-add", "CHOWN", "--cap-add", "SETUID",
            "--cap-add", "SETGID", "--cap-add", "SYS_CHROOT",
            "--tmpfs", "/run:rw,nosuid,nodev,noexec,size=8m,mode=755",
            "--tmpfs", "/tmp:rw,nosuid,nodev,noexec,size=16m,mode=1777",
            "--mount", f"type=volume,src={volumes[0]},dst=/host-public",
            "--mount", f"type=volume,src={volumes[1]},dst=/workspace",
            "--pids-limit", "128", "--memory", "768m",
            "-p", "127.0.0.1::1022", args.image, "ssh", env=process_env)
        collector_name = prefix + "-collector"
        containers.append(collector_name)
        run("docker", "run", "-d", "--name", collector_name, *common,
            "--user", "10002:10002", "--env", "COVENANT_COLLECTOR_TOKEN",
            "--mount", f"type=bind,src={guest_socket},dst=/var/run/dstack.sock,readonly",
            "--mount", f"type=volume,src={volumes[0]},dst=/host-public,readonly",
            "--mount", f"type=volume,src={volumes[2]},dst=/state",
            "--pids-limit", "16", "--memory", "256m",
            "-p", "127.0.0.1::8090", args.image, "collector", env=process_env)

        def mapped_port(name, port):
            state = json.loads(run("docker", "inspect", name).stdout)[0]
            return int(state["NetworkSettings"]["Ports"][f"{port}/tcp"][0]["HostPort"])
        ssh_port = mapped_port(ssh_name, 1022)
        collector_port = mapped_port(collector_name, 8090)
        base = f"http://127.0.0.1:{collector_port}"

        def request_collector(path, body=None, authorization=token):
            headers = {"Content-Type": "application/json"}
            if authorization:
                headers["Authorization"] = "Bearer " + authorization
            request = urllib.request.Request(
                base + path, data=json.dumps(body).encode() if body is not None else None,
                headers=headers,
            )
            try:
                with urllib.request.urlopen(request, timeout=3) as response:
                    return response.status, json.load(response)
            except urllib.error.HTTPError as error:
                return error.code, json.load(error)

        identity = None
        for _ in range(60):
            try:
                status, value = request_collector("/identity")
                if status == 200:
                    identity = value["identity"]
                    break
            except (OSError, urllib.error.URLError):
                pass
            time.sleep(0.25)
        if identity is None:
            raise RuntimeError("SSH/collector readiness failed")
        assert request_collector("/identity", authorization=None)[0] == 401
        assert request_collector("/identity", authorization="00" * 32)[0] == 401
        known_hosts = directory / "known_hosts"
        key = identity["guest_channel_key"]
        known_hosts.write_text(f'[127.0.0.1]:{ssh_port} {key["algorithm"]} {key["key_base64"]}\n')
        ssh_options = [
            "-o", "BatchMode=yes", "-o", "IdentitiesOnly=yes",
            "-o", "StrictHostKeyChecking=yes", "-o", f"UserKnownHostsFile={known_hosts}",
            "-o", "ConnectTimeout=3", "-o", "LogLevel=ERROR",
        ]

        def ssh(command, key_name="buyer", check=True, input_text=None):
            return run("ssh", *ssh_options, "-i", str(directory / key_name),
                       "-p", str(ssh_port), "covenant@127.0.0.1", command,
                       check=check, input_text=input_text)
        assert ssh("id -u").stdout.strip() == "10001"
        assert ssh("pwd").stdout.strip() == "/workspace"
        assert ssh("cat /run/covenant/host_key", check=False).returncode != 0
        ssh("test ! -e /var/run/dstack.sock")
        ssh("test ! -w /host-public/host_key.pub")
        ssh("test ! -w /run/authorized_keys")
        ssh("test ! -w /opt/covenant/collector.py")
        ssh("test -z \"${COVENANT_COLLECTOR_TOKEN+x}\"")
        assert ssh("id", key_name="wrong-buyer", check=False).returncode != 0
        ssh("cat > /workspace/synthetic.txt", input_text="synthetic acceptance only\n")
        assert ssh("cat /workspace/synthetic.txt").stdout == "synthetic acceptance only\n"
        source = directory / "sftp-source"
        source.write_text("synthetic sftp\n")
        batch = directory / "sftp-batch"
        batch.write_text(f"put {source} /workspace/sftp.txt\n")
        run("sftp", *ssh_options, "-i", str(directory / "buyer"),
            "-P", str(ssh_port), "-b", str(batch), "covenant@127.0.0.1")
        assert ssh("cat /workspace/sftp.txt").stdout == "synthetic sftp\n"
        challenge = {
            "schema_version": 1, "challenge_id": str(uuid.uuid4()),
            "nonce_hex": secrets.token_hex(32), "issued_at_ms": int(time.time() * 1000),
            "expires_at_ms": int(time.time() * 1000) + 60_000,
            "scope": {field: policy[field] for field in (
                "job_id", "operator_pubkey_b58", "buyer_pubkey_b58",
                "lease_generation", "lease_expires_at_ms",
            )},
        }
        challenge["scope"].update({field: identity[field] for field in (
            "host", "port", "username", "guest_channel_key", "transport",
        )})
        challenge["scope"]["workload_sha256"] = INFO["compose_hash"]
        wrong = copy.deepcopy(challenge)
        wrong["scope"]["host"] = "unrelated.example"
        assert request_collector("/quote", wrong)[0] == 409
        assert guest_calls == []
        status, evidence = request_collector("/quote", challenge)
        assert status == 200 and evidence["verification_status"] == "unverified"
        assert evidence["dstack_quote"]["event_log"] == EVENTS
        assert len(guest_calls) == 1
        assert request_collector("/quote", challenge)[0] == 409
        run("docker", "restart", collector_name)
        # Docker may allocate a new ephemeral host port across a restart.
        collector_port = mapped_port(collector_name, 8090)
        base = f"http://127.0.0.1:{collector_port}"
        restarted = False
        for _ in range(30):
            try:
                if request_collector("/identity")[0] == 200:
                    restarted = True
                    break
            except (OSError, urllib.error.URLError):
                pass
            time.sleep(0.1)
        assert restarted, "collector did not become ready after restart"
        assert request_collector("/quote", challenge)[0] == 409
        assert len(guest_calls) == 1
        # Replacing a host-key observation must fail the actual SSH handshake.
        wrong_public = " ".join((directory / "wrong-buyer.pub").read_text().split()[:2])
        known_hosts.write_text(f"[127.0.0.1]:{ssh_port} {wrong_public}\n")
        assert ssh("id", check=False).returncode != 0
        # A distinct short lease proves process/session shutdown. Never mutate
        # the main lease policy or reuse its persistent consumption journal.
        expiry_policy = copy.deepcopy(policy)
        expiry_policy.update(job_id=str(uuid.uuid4()),
                             lease_expires_at_ms=int(time.time() * 1000) + 20_000)
        expiry_env = dict(process_env, COVENANT_LEASE_POLICY_B64=base64.b64encode(
            c.canonical(expiry_policy)).decode())
        for suffix in ("expiry-public", "expiry-state"):
            name = prefix + "-" + suffix
            run("docker", "volume", "create", name)
            volumes.append(name)
        expiry_ssh = prefix + "-expiry-ssh"
        containers.append(expiry_ssh)
        run("docker", "run", "-d", "--name", expiry_ssh, *common,
            "--user", "0:0", "--cap-add", "CHOWN", "--cap-add", "SETUID",
            "--cap-add", "SETGID", "--cap-add", "SYS_CHROOT",
            "--tmpfs", "/run:rw,nosuid,nodev,noexec,size=8m,mode=755",
            "--tmpfs", "/tmp:rw,nosuid,nodev,noexec,size=16m,mode=1777",
            "--mount", f"type=volume,src={volumes[-2]},dst=/host-public",
            "--mount", f"type=volume,src={volumes[1]},dst=/workspace",
            "-p", "127.0.0.1::1022", args.image, "ssh", env=expiry_env)
        expiry_collector = prefix + "-expiry-collector"
        containers.append(expiry_collector)
        run("docker", "run", "-d", "--name", expiry_collector, *common,
            "--user", "10002:10002", "--env", "COVENANT_COLLECTOR_TOKEN",
            "--mount", f"type=bind,src={guest_socket},dst=/var/run/dstack.sock,readonly",
            "--mount", f"type=volume,src={volumes[-2]},dst=/host-public,readonly",
            "--mount", f"type=volume,src={volumes[-1]},dst=/state",
            "-p", "127.0.0.1::8090", args.image, "collector", env=expiry_env)
        ssh_port = mapped_port(expiry_ssh, 1022)
        base = f"http://127.0.0.1:{mapped_port(expiry_collector, 8090)}"
        expiry_identity = None
        for _ in range(60):
            try:
                status, value = request_collector("/identity")
                if status == 200:
                    expiry_identity = value["identity"]
                    break
            except (OSError, urllib.error.URLError):
                pass
            time.sleep(0.1)
        assert expiry_identity is not None
        expiry_key = expiry_identity["guest_channel_key"]
        known_hosts.write_text(
            f'[127.0.0.1]:{ssh_port} {expiry_key["algorithm"]} {expiry_key["key_base64"]}\n')
        assert ssh("id -u").stdout.strip() == "10001"
        session = subprocess.Popen(
            ["ssh", *ssh_options, "-i", str(directory / "buyer"), "-p", str(ssh_port),
             "covenant@127.0.0.1", "sleep 30; printf should-not-complete"],
            text=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
        )
        try:
            stdout, _ = session.communicate(timeout=25)
        except subprocess.TimeoutExpired:
            session.kill()
            session.communicate()
            raise AssertionError("existing SSH session survived lease expiry")
        assert session.returncode != 0 and "should-not-complete" not in stdout
        assert ssh("id", check=False).returncode != 0
        for _ in range(30):
            states = [json.loads(run("docker", "inspect", name).stdout)[0]["State"]["Running"]
                      for name in (expiry_ssh, expiry_collector)]
            if not any(states):
                break
            time.sleep(0.1)
        assert not any(states), "lease roles remained running after expiry"
        print("PASS: real SSH/SFTP/auth/key isolation, mock collector/replay, lease expiry.")
        print("No genuine hardware evidence, Phala transport, lease integration or trust accepted.")
    except Exception:
        for name in containers:
            logs = run("docker", "logs", "--tail", "30", name, check=False)
            print(f"{name} diagnostic logs:\n{logs.stdout}{logs.stderr}")
        raise
    finally:
        for name in reversed(containers):
            run("docker", "rm", "-f", name, check=False)
        for name in reversed(volumes):
            run("docker", "volume", "rm", name, check=False)
        if guest_server is not None:
            guest_server.shutdown()
            guest_server.server_close()
        directory_context.cleanup()


if __name__ == "__main__":
    main()
