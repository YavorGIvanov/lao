"""Offline contract checks; build workflow_support and lao first. No model requests."""
import json
from pathlib import Path
import subprocess
import signal
import tempfile
import unittest
from unittest.mock import patch
import collect


class WorkflowContract(unittest.TestCase):
    def setUp(self):
        self.oracle = collect.Oracle()
        self.fixture = self.oracle.call(op="prepare", task="web-dev-port")
        self.root = Path(self.fixture["root"]).resolve()
        collect.FIXTURES.add(self.root)
        self.temp = tempfile.TemporaryDirectory(prefix="lao-collector-test-")
        self.state = Path(self.temp.name).resolve()
        collect.worker_home(self.state / "worker", 1, "a" * 64)
        record = self.state / "worker/Library/Application Support/lao/install.json"
        data = json.loads(record.read_text())
        data["router"] = "safe"
        collect.atom(record, data)

    def tearDown(self):
        self.oracle.call(op="dispose")
        self.oracle.close()
        collect.cleanup()
        collect.FIXTURES.discard(self.root)
        self.temp.cleanup()

    def relay(self, requests):
        env = collect.environment()
        env["HOME"] = str(self.state / "worker")
        p = collect.spawn([collect.sys.executable, str(collect.DOCS / "collect.py"), "--mcp-relay",
                           "--allowed-file", "package.json", "--audit", str(self.state / "mcp.json")],
                          cwd=self.root, env=env, stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                          stderr=subprocess.DEVNULL)
        initialize = {"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {
            "protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": {"name": "offline", "version": "1"}}}
        try:
            output, _ = p.communicate(b"".join(json.dumps(r).encode() + b"\n" for r in [initialize] + requests), timeout=15)
            self.assertEqual(p.returncode, 0)
            return [json.loads(line) for line in output.splitlines()]
        finally:
            collect.stop(p)

    @staticmethod
    def packet(identifier, file):
        return {"jsonrpc": "2.0", "id": identifier, "method": "tools/call", "params": {
            "name": "execute", "arguments": {"objective": "offline-marker-never-echo", "allowed_paths": [file]}}}

    def test_cloud_deferral_cannot_replace_independent_task_verification(self):
        rows = self.relay([self.packet(2, "package.json")])
        self.assertEqual(rows[-1]["result"]["structuredContent"]["status"], "cloud")
        self.assertEqual(self.oracle.call(op="verify"), {"verified": False, "scope_ok": True})
        data = json.loads((self.root / "package.json").read_text())
        data["scripts"]["dev"] = "vite --port 5173"
        (self.root / "package.json").write_text(json.dumps(data))
        self.assertEqual(self.oracle.call(op="verify"), {"verified": True, "scope_ok": True})
        audit = json.loads((self.state / "mcp.json").read_text())
        self.assertEqual(audit["calls"], 1)
        self.assertEqual(audit["status"], "cloud")
        self.assertNotIn("offline-marker-never-echo", json.dumps(audit))

    def test_scope_or_repeated_packets_are_refused_without_side_effects(self):
        rows = self.relay([self.packet(2, "README.md"), self.packet(3, "package.json")])
        self.assertTrue(all(row["result"]["isError"] for row in rows[1:]))
        self.assertNotIn("offline-marker-never-echo", json.dumps(rows))
        self.assertEqual(self.oracle.call(op="verify"), {"verified": False, "scope_ok": True})
        audit = json.loads((self.state / "mcp.json").read_text())
        self.assertTrue(audit["refused"])
        self.assertEqual(audit["calls"], 2)
        # An actual out-of-scope file is independently rejected even after a worker response.
        (self.root / "extra.txt").write_text("unrequested")
        self.assertFalse(self.oracle.call(op="verify")["scope_ok"])

    def test_interrupted_parent_is_reaped_and_retains_sanitized_usage(self):
        script = """
import json, os, signal, time
print(json.dumps({'type': 'turn.completed', 'usage': {
    'input_tokens': 7, 'output_tokens': True, 'private': 'never-retain'}}), flush=True)
time.sleep(0.2)
os.kill(os.getppid(), signal.SIGUSR1)
time.sleep(30)
"""
        previous = signal.signal(signal.SIGUSR1, collect.interrupted)
        try:
            with patch.object(collect, "codex_args", return_value=[collect.sys.executable, "-c", script]):
                observed = collect.native("codex", self.root, "package.json", self.state, False,
                                          self.state / "worker", "unused")
            self.assertEqual(observed["parent_status"], "interrupted")
            self.assertEqual(observed["reason"], "interrupted")
            self.assertEqual(observed["tokens"], {"input_tokens": 7})
            self.assertEqual(collect.PROCESSES, {self.oracle.p})
            self.assertEqual(self.oracle.call(op="verify"), {"verified": False, "scope_ok": True})
        finally:
            signal.signal(signal.SIGUSR1, previous)
            collect.STOP_REQUESTED = False

    def test_resource_continuation_preserves_completed_rows_and_refuses_dispatched_rows(self):
        plan = [dict(harness="codex", cache="cold", task="web-dev-port", round=1, arm=arm, state="planned")
                for arm in ("baseline", "candidate")]
        metadata = {key: "fixed" for key in ("versions", "artifacts", "lockfile_sha256", "oracle_sha256",
                                             "lao_sha256", "host", "ram_bytes", "os")}
        first = dict(plan[0], state="completed", cleanup_ok=True, scope_ok=True, verified=False,
                     resource_before={"stable": True}, resource_after={"stable": True})
        report = dict(schema="lao-subscription-workflow-1", status="refused", refusal="resource_guard_refused",
                      expires_at_unix=collect.time.time() + 60, metadata=metadata, trials=[first, plan[1]])
        path = self.state / "paused.json"
        collect.atom(path, report)
        original = path.read_bytes()
        resumed = collect.continuation(path, plan, metadata)
        self.assertEqual(resumed["trials"][0], first)
        self.assertEqual(resumed["continuations"][0]["first_trial_index"], 1)
        self.assertEqual(path.read_bytes(), original)
        report["trials"][1]["state"] = "dispatched"
        collect.atom(path, report)
        with self.assertRaisesRegex(RuntimeError, "continuation_requires_untouched_suffix"):
            collect.continuation(path, plan, metadata)

    def test_cleanup_failure_cannot_hide_the_terminal_checkpoint(self):
        output = self.state / "failed.json"
        with (patch.object(collect, "preflight", return_value={"source_dirty": False}),
              patch.object(collect, "checked", side_effect=RuntimeError("pre_dispatch_failure")),
              patch.object(collect, "Oracle") as oracle):
            oracle.return_value.close.side_effect = PermissionError()
            with self.assertRaises(PermissionError):
                collect.run(output)
        report = json.loads(output.read_text())
        self.assertEqual(report["status"], "refused")
        self.assertEqual(report["refusal"], "pre_dispatch_failure")
        self.assertEqual(report["cleanup_error"], "PermissionError")
        self.assertTrue(all(r["state"] == "planned" for r in report["trials"]))

    def test_partial_finish_retains_invalid_pair_and_never_admits_scope_failure(self):
        metadata = {key: "fixed" for key in ("versions", "artifacts", "lockfile_sha256", "oracle_sha256",
                                             "lao_sha256", "host", "ram_bytes", "os")}
        plan = [dict(harness=h, cache=cache, task="web-dev-port", round=1, arm=arm, state="planned")
                for h, cache, arm in [("codex", "cold", "baseline"), ("codex", "cold", "candidate"),
                                      ("codex", "warm", "candidate"), ("codex", "warm", "baseline"),
                                      ("claude", "warm", "baseline"), ("claude", "warm", "candidate")]]
        rows = [dict(r, state="completed", cleanup_ok=True, scope_ok=True, verified=False,
                     resource_before={"stable": True, "ac_power": False},
                     resource_after={"stable": i != 2, "ac_power": False}) if i < 3 else r
                for i, r in enumerate(plan)]
        report = dict(schema="lao-subscription-workflow-1", status="refused",
                      refusal="postflight_resource_or_scope_refused", expires_at_unix=collect.time.time() + 60,
                      metadata=metadata, trials=rows)
        path = self.state / "partial.json"
        collect.atom(path, report)
        original = path.read_bytes()
        resumed = collect.continuation(path, plan, metadata, finish_partial=True)
        self.assertEqual(resumed["trials"], rows)
        self.assertEqual(path.read_bytes(), original)
        self.assertEqual(len(resumed["infrastructure_invalid_pairs"]), 1)
        self.assertEqual(resumed["infrastructure_invalid_pairs"][0]["cache"], "warm")
        rows[2]["scope_ok"] = False
        collect.atom(path, report)
        with self.assertRaisesRegex(RuntimeError, "continuation_requires_untouched_suffix"):
            collect.continuation(path, plan, metadata, finish_partial=True)


if __name__ == "__main__":
    unittest.main()
