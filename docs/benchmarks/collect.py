#!/usr/bin/env python3
"""Opt-in subscription workflow collection on the pinned public corpus (macOS)."""
import argparse
import hashlib
import http.client
import json
import os
from pathlib import Path
import selectors
import re
import shutil
import signal
import socket
import subprocess
import statistics
import sys
import tempfile
import time

ROOT = Path(__file__).resolve().parents[2]
DOCS = ROOT / "docs/benchmarks"
SUPPORT = ROOT / "target/debug/examples/workflow_support"
GATE = ROOT / "target/debug/examples/workflow_gate"
LAO = ROOT / "target/debug/lao"
MODEL = "gpt-6-astra"
CLAUDE_MODEL = "claude-opus-5"
COMMON = ("Complete the objective within the declared writable file. Review the resulting diff "
          "and check that all other settings are preserved. Stop when the objective is satisfied. "
          "Do not install dependencies, run project scripts, access the network, or edit any other file.")
HYBRID = ("Before editing, call lao.execute once if you can formulate a bounded implementation packet "
          "with exact writable paths. Keep planning and review in this harness. If Cloud is returned, "
          "continue here. After Local returns, inspect the actual diff and verify the objective, including "
          "when the worker failed. Repair partial or incorrect edits within the original scope; do not "
          "replay the packet or bypass a sandbox refusal.")
PROCESSES = set()
FIXTURES = set()
STOP_REQUESTED = False


def digest(path):
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for b in iter(lambda: f.read(1024 * 1024), b""):
            h.update(b)
    return h.hexdigest()


def task_digest(path):
    try:
        meta = path.lstat()
        if path.is_symlink() or not path.is_file() or meta.st_nlink != 1 or meta.st_size > 1024 * 1024:
            return None
        return digest(path)
    except OSError:
        return None


def environment():
    # Never inherit provider tokens, proxy settings, user hooks or shell startup state.
    return {k: os.environ[k] for k in ("HOME", "PATH", "TMPDIR", "USER", "LOGNAME", "LANG") if k in os.environ}


def spawn(args, **kw):
    p = subprocess.Popen(args, start_new_session=True, **kw)
    PROCESSES.add(p)
    return p


def stop(p):
    try:
        os.killpg(p.pid, signal.SIGKILL)
    except ProcessLookupError:
        pass
    p.wait(timeout=5)
    PROCESSES.discard(p)
    for stream in (p.stdin, p.stdout, p.stderr):
        if stream is not None:
            stream.close()


def cleanup():
    for p in list(PROCESSES):
        stop(p)
    for root in FIXTURES:
        shutil.rmtree(root, ignore_errors=True)


def interrupted(_sig, _frame):
    global STOP_REQUESTED
    STOP_REQUESTED = True


def checked(args, timeout=15):
    p = subprocess.run(args, capture_output=True, env=environment(), timeout=timeout)
    if p.returncode or len(p.stdout) + len(p.stderr) > 1024 * 1024:
        raise RuntimeError("preflight_command_failed")
    return p.stdout + p.stderr


def atom(path, value):
    part = path.with_suffix(path.suffix + ".part")
    with open(part, "w", encoding="utf-8") as f:
        os.chmod(part, 0o600)
        json.dump(value, f, indent=2)
        f.write("\n")
        f.flush()
        os.fsync(f.fileno())
    os.replace(part, path)


class Oracle:
    def __init__(self, executable=SUPPORT):
        self.p = spawn([str(executable)], stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                       stderr=subprocess.DEVNULL, env=environment())

    def call(self, **request):
        self.p.stdin.write(json.dumps(request).encode() + b"\n")
        self.p.stdin.flush()
        with selectors.DefaultSelector() as sel:
            sel.register(self.p.stdout, selectors.EVENT_READ)
            if not sel.select(45):
                raise RuntimeError("oracle_timeout")
        line = self.p.stdout.readline(65537)
        if not line:
            self.p.wait(timeout=5)
            raise RuntimeError("oracle_failed")
        if len(line) > 65536:
            raise RuntimeError("oracle_failed")
        return json.loads(line)

    def close(self):
        if self.p.args == [str(SUPPORT)]:
            try:
                self.p.stdin.close()
            except BrokenPipeError:
                pass
            try:
                self.p.wait(timeout=5)
            except subprocess.TimeoutExpired:
                pass
            else:
                PROCESSES.discard(self.p)
                self.p.stdout.close()
                return
        stop(self.p)


def rate_limits():
    p = spawn(["codex", "app-server", "-c", 'forced_login_method="chatgpt"'],
              stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL,
              env=environment())
    try:
        for obj in [dict(id=1, method="initialize", params=dict(clientInfo=dict(name="lao-public-workflow", version="1"))),
                    dict(method="initialized", params={}), dict(id=2, method="account/rateLimits/read", params={})]:
            p.stdin.write(json.dumps(obj).encode() + b"\n")
            p.stdin.flush()
        end = time.monotonic() + 15
        with selectors.DefaultSelector() as sel:
            sel.register(p.stdout, selectors.EVENT_READ)
            while time.monotonic() < end:
                if not sel.select(1):
                    continue
                line = p.stdout.readline(65537)
                if not line or len(line) > 65536:
                    break
                d = json.loads(line)
                if d.get("id") == 2:
                    r = d.get("result", {})
                    limits = r.get("rateLimits", {})
                    credits = limits.get("credits") or {}
                    windows = [limits[k]["usedPercent"] for k in ("primary", "secondary") if limits.get(k)]
                    safe = (r.get("ordinaryUsageAllowed") is True and windows and max(windows) < 90
                            and credits.get("hasCredits") is False and credits.get("balance") == "0")
                    return {"within_subscription": bool(safe), "used_percent": windows,
                            "paid_credits_available": credits.get("hasCredits")}
        raise RuntimeError("subscription_limits_unavailable")
    finally:
        stop(p)


def health():
    def number(key):
        return int(checked(["/usr/sbin/sysctl", "-n", key]).strip())
    pressure = number("kern.memorystatus_vm_pressure_level")
    free_percent = number("kern.memorystatus_level")
    power = checked(["/usr/bin/pmset", "-g", "batt"]).decode()
    thermal = checked(["/usr/bin/pmset", "-g", "therm"]).decode()
    thermal_ok = "No thermal warning level has been recorded" in thermal and "No performance warning level has been recorded" in thermal
    battery = re.search(r"(\d+)%;", power)
    battery_percent = int(battery.group(1)) if battery else None
    power_ok = "AC Power" in power or battery_percent is not None and battery_percent >= 50
    return {"pressure": pressure, "available_percent": free_percent,
            "ac_power": "AC Power" in power, "battery_percent": battery_percent, "thermal_ok": thermal_ok,
            "stable": pressure == 1 and free_percent >= 35 and power_ok and thermal_ok}


def toml(value):
    if isinstance(value, dict):
        return "{" + ",".join(json.dumps(k) + "=" + toml(v) for k, v in value.items()) + "}"
    if isinstance(value, list):
        return "[" + ",".join(toml(v) for v in value) + "]"
    return json.dumps(value)


def codex_args(root, file, state, hybrid, worker_home):
    config = {
        "forced_login_method": "chatgpt", "model_provider": "openai", "model_reasoning_effort": "medium",
        "model_context_window": 32768,
        "approval_policy": "never", "default_permissions": "campaign", "allow_login_shell": False,
        "permissions.campaign": {"filesystem": {":minimal": "read", str(root): "read", str(root / file): "write"},
                                 "network": {"enabled": False}},
        "shell_environment_policy": {"inherit": "none", "set": {"PATH": "/usr/bin:/bin", "HOME": str(state), "GIT_OPTIONAL_LOCKS": "0"}},
        "developer_instructions": COMMON + "\nThe campaign sandbox grants read access to this workspace and write access to "
        + str(root / file) + ". A Cloud tool result is a routing decision, not a sandbox refusal; complete the edit using this harness within that exact file permission."
        + ("\n" + HYBRID if hybrid else ""),
        "project_doc_max_bytes": 0, "web_search": "disabled", "log_dir": str(state / "logs"),
        "features": {"apps": False, "plugins": False, "hooks": False, "codex_hooks": False,
                     "memories": False, "shell_snapshot": False, "multi_agent": False,
                     "skip_host_skill_discovery": True, "fast_mode": False},
    }
    if hybrid:
        config["mcp_servers.lao"] = {"command": sys.executable, "args": [str(Path(__file__).resolve()), "--mcp-relay", "--allowed-file", file, "--audit", str(state / "mcp.json")], "cwd": str(root),
            "env": {"HOME": str(worker_home)}, "enabled_tools": ["execute"], "required": True,
            "tool_timeout_sec": 610, "tools": {"execute": {"approval_mode": "approve"}}}
    args = ["codex", "exec", "--ignore-user-config", "--ignore-rules", "--ephemeral", "--strict-config",
            "--json", "--color", "never", "--model", MODEL, "-C", str(root)]
    for k, v in config.items():
        args.extend(["-c", k + "=" + toml(v)])
    return args + ["-"]


def claude_args(root, file, state, hybrid, worker_home):
    path = str(root / file)
    allow = ["Read(/" + path + ")", "Edit(/" + path + ")", "Write(/" + path + ")"]
    if hybrid:
        allow.append("mcp__lao__execute")
    settings = {"disableAllHooks": True, "permissions": {"defaultMode": "dontAsk", "allow": allow},
                "env": {"CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC": "1", "CLAUDE_CODE_DISABLE_AUTO_MEMORY": "1"}}
    mcp = {"mcpServers": {}}
    if hybrid:
        mcp["mcpServers"]["lao"] = {"command": sys.executable, "args": [str(Path(__file__).resolve()), "--mcp-relay", "--allowed-file", file, "--audit", str(state / "mcp.json")], "env": {"HOME": str(worker_home)}}
    return ["claude", "--print", "--restricted", "--setting-sources", "", "--settings", json.dumps(settings),
            "--disable-slash-commands", "--strict-mcp-config", "--mcp-config", json.dumps(mcp),
            "--no-session-persistence", "--no-chrome", "--tools", "Read,Edit,Write",
            "--permission-mode", "dontAsk", "--output-format", "stream-json", "--verbose",
            "--model", CLAUDE_MODEL, "--effort", "high", "--max-budget-usd", "0.75",
            "--append-system-prompt", COMMON + ("\n" + HYBRID if hybrid else "")]


def native(harness, root, file, state, hybrid, worker_home, prompt, deadline=900):
    args = (codex_args if harness == "codex" else claude_args)(root, file, state, hybrid, worker_home)
    env = environment()
    env.update({"CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC": "1", "CLAUDE_CODE_DISABLE_AUTO_MEMORY": "1"})
    p = spawn(args, cwd=root, env=env, stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    p.stdin.write(prompt.encode())
    p.stdin.close()
    observed = {"parent_status": "failed", "local_worker_status": None, "tokens": {},
                "mcp_calls": 0, "failed_tools": 0, "model": None,
                "requested_model": MODEL if harness == "codex" else CLAUDE_MODEL, "reason": None}
    end = time.monotonic() + deadline
    buffers = {p.stdout: b"", p.stderr: b""}
    total = 0
    try:
        with selectors.DefaultSelector() as sel:
            for stream in buffers:
                os.set_blocking(stream.fileno(), False)
                sel.register(stream, selectors.EVENT_READ)
            while sel.get_map():
                if STOP_REQUESTED:
                    observed["parent_status"] = "interrupted"
                    observed["reason"] = "interrupted"
                    break
                if time.monotonic() >= end:
                    observed["parent_status"] = "timeout"
                    break
                for key, _ in sel.select(0.2):
                    b = os.read(key.fileobj.fileno(), 65536)
                    total += len(b)
                    if total > 4 * 1024 * 1024:
                        raise RuntimeError("native_output_limit")
                    if not b:
                        sel.unregister(key.fileobj)
                        continue
                    if key.fileobj is p.stderr:
                        # Classify only fixed diagnostic codes; never retain client text.
                        text = b.lower()
                        for needle, reason in [(b"unknown field", "config_unknown_field"), (b"error loading", "config_load"),
                                               (b"usage limit", "quota"), (b"rate limit", "quota"), (b"not logged", "auth")]:
                            if needle in text:
                                observed["reason"] = reason
                        continue
                    buffers[key.fileobj] += b
                    while b"\n" in buffers[key.fileobj]:
                        line, buffers[key.fileobj] = buffers[key.fileobj].split(b"\n", 1)
                        if not line:
                            continue
                        event = json.loads(line)
                        kind = event.get("type")
                        item = event.get("item", {})
                        if kind == "item.completed" and item.get("type") in ("command_execution", "file_change", "mcp_tool_call"):
                            if item.get("status") == "failed" or item.get("exit_code", 0) not in (0, None) or item.get("error"):
                                observed["failed_tools"] += 1
                        if kind == "turn.completed":
                            observed["parent_status"] = "complete"
                            observed["tokens"] = {k: v for k, v in event.get("usage", {}).items()
                                                  if k in ("input_tokens", "cached_input_tokens", "output_tokens",
                                                           "cache_write_input_tokens", "reasoning_output_tokens")
                                                  and type(v) is int and v >= 0}
                        if kind in ("turn.failed", "error"):
                            msg = json.dumps(event).lower()
                            if any(x in msg for x in ("usage limit", "rate limit", "quota")):
                                observed["reason"] = "quota"
                        if harness == "claude":
                            if kind == "user":
                                observed["failed_tools"] += sum(block.get("type") == "tool_result" and block.get("is_error") is True
                                                                for block in event.get("message", {}).get("content", []) if isinstance(block, dict))
                            if kind == "system" and event.get("subtype") == "init":
                                observed["model"] = event.get("model")
                            if kind == "result":
                                observed["parent_status"] = "failed" if event.get("is_error") else "complete"
                                observed["tokens"] = {k: v for k, v in event.get("usage", {}).items()
                                                      if k in ("input_tokens", "output_tokens", "cache_creation_input_tokens", "cache_read_input_tokens")}
                                if event.get("subtype") == "error_max_budget_usd":
                                    observed["reason"] = "cost_limit"
        if observed["parent_status"] not in ("timeout", "interrupted"):
            code = p.wait(timeout=5)
            if code:
                observed["parent_status"] = "failed"
    finally:
        stop(p)
    if observed["parent_status"] != "complete":
        # Production OpenCode's parent-death watcher must settle before verification.
        time.sleep(2)
    audit = state / "mcp.json"
    packet = json.loads(audit.read_text()) if audit.exists() else {"calls": 0}
    observed["mcp_calls"] = packet["calls"]
    observed["local_worker_status"] = packet.get("status")
    observed["route"] = "cloud"
    if packet.get("status") in ("complete", "agent_failed", "timed_out"):
        observed["route"] = "local" if task_digest(root / file) == packet.get("after_sha256") else "local_then_cloud"
    if observed["reason"] == "interrupted":
        return observed
    if packet["calls"] > 1 or packet.get("refused"):
        observed["reason"] = "packet_scope_refused"
    if packet.get("status") == "dispatched":
        observed["reason"] = "packet_incomplete"
    return observed


def worker_home(path, gate_port, caller):
    home = Path(os.environ["HOME"])
    cache = path / "Library/Caches/lao"
    cache.mkdir(parents=True)
    for name in ("routers", "workers"):
        (cache / name).symlink_to(home / "Library/Caches/lao" / name, target_is_directory=True)
    state = path / "Library/Application Support/lao"
    state.mkdir(parents=True)
    atom(state / "install.json", {"phase": "installed", "port": gate_port,
         "codex": {"path": str(path / "unused.toml"), "existed": False, "mode": 384},
         "claude": None, "claude_mcp": None, "router": "semantic", "router_addr": None, "router_key": None})
    key = state / "worker.key"
    key.write_text(caller)
    key.chmod(0o600)


def warm(endpoint):
    host, port = endpoint["addr"].split(":")
    c = http.client.HTTPConnection(host, int(port), timeout=90)
    try:
        payload = json.dumps({"model": "lao-local", "messages": [{"role": "user", "content": "Reply OK."}],
                              "max_tokens": 1, "temperature": 0})
        c.request("POST", "/v1/chat/completions", payload,
                  {"Authorization": "Bearer " + endpoint["bearer"], "Content-Type": "application/json"})
        r = c.getresponse()
        if r.status != 200 or len(r.read(65537)) > 65536:
            raise RuntimeError("warmup_failed")
    finally:
        c.close()


def artifact_paths():
    cache = Path(os.environ["HOME"]) / "Library/Caches/lao"
    artifacts = {"gate": GATE, "codex": Path(shutil.which("codex")).resolve(),
                 "claude": Path(shutil.which("claude")).resolve(),
                 "runtime": cache / "runtimes/llama-b10280/llama-server",
                 "model_tokenizer_template": cache / "models/Qwen3-4B-Q4_K_M.gguf",
                 "worker": cache / "workers/opencode/opencode-v1.18.25/opencode",
                 "worker_support_lockfile": cache / "workers/opencode/opencode-v1.18.25/config/opencode/package-lock.json"}
    for name in ("config.json", "tokenizer.json", "model.safetensors"):
        artifacts["router_" + name] = cache / "routers/minilm" / name
    return artifacts


def preflight():
    versions = {h: checked([h, "--version"]).decode().strip() for h in ("codex", "claude")}
    if versions != {"codex": "codex-cli 0.154.0", "claude": "2.1.251 (Claude Code)"}:
        raise RuntimeError("harness_version_drift")
    if b"ChatGPT" not in checked(["codex", "login", "status"]):
        raise RuntimeError("codex_subscription_required")
    auth = json.loads(checked(["claude", "auth", "status"]))
    if not auth.get("loggedIn") or auth.get("authMethod") != "claude.ai" or auth.get("apiProvider") != "firstParty":
        raise RuntimeError("claude_subscription_required")
    limits = rate_limits()
    if not limits["within_subscription"]:
        raise RuntimeError("subscription_guard_refused")
    if not health()["stable"]:
        raise RuntimeError("resource_guard_refused")
    artifacts = artifact_paths()
    return {"versions": versions, "subscription": limits, "resources": health(),
            "artifacts": {label: digest(path) for label, path in artifacts.items()},
            "source_dirty": bool(checked(["git", "-C", str(ROOT), "status", "--porcelain"])),
            "fixture_manifest_sha256": digest(DOCS / "workflow.sha256"),
            "source_revision": checked(["git", "-C", str(ROOT), "rev-parse", "HEAD"]).decode().strip(),
            "lockfile_sha256": digest(ROOT / "Cargo.lock"), "collector_sha256": digest(Path(__file__)),
            "oracle_sha256": digest(SUPPORT), "lao_sha256": digest(LAO),
            "host": checked(["/usr/sbin/sysctl", "-n", "hw.model"]).decode().strip(),
            "ram_bytes": int(checked(["/usr/sbin/sysctl", "-n", "hw.memsize"]).strip()),
            "os": checked(["/usr/bin/sw_vers", "-productVersion"]).decode().strip()}


def continuation(path, plan, metadata, finish_partial=False):
    report = json.loads(path.read_text())
    refusals = ("resource_guard_refused", "setup_cleanup_failed_before_dispatch")
    if finish_partial:
        refusals += ("postflight_resource_or_scope_refused",)
    if (report.get("schema") != "lao-subscription-workflow-1" or report.get("status") != "refused"
            or report.get("refusal") not in refusals
            or time.time() >= report["expires_at_unix"]):
        raise RuntimeError("continuation_not_eligible")
    previous = report["metadata"]
    for key in ("versions", "artifacts", "lockfile_sha256", "oracle_sha256", "lao_sha256", "host", "ram_bytes", "os"):
        if previous[key] != metadata[key]:
            raise RuntimeError("continuation_artifact_drift")
    rows = report["trials"]
    if len(rows) != len(plan):
        raise RuntimeError("continuation_schedule_drift")
    pending = False
    for row, expected in zip(rows, plan):
        if any(row[key] != expected[key] for key in ("harness", "cache", "task", "round", "arm")):
            raise RuntimeError("continuation_schedule_drift")
        if row["state"] == "planned":
            pending = True
        elif (row["state"] != "completed" or pending or not row.get("cleanup_ok") or not row.get("scope_ok")
              or not finish_partial and (not row["resource_before"]["stable"] or not row["resource_after"]["stable"])):
            raise RuntimeError("continuation_requires_untouched_suffix")
    if not pending:
        raise RuntimeError("continuation_has_no_pending_rows")
    if finish_partial:
        if (not all(r["state"] == "completed" for r in rows if r["harness"] == "codex" and r["cache"] == "cold")
                or not all(r["state"] == "planned" for r in rows if r["harness"] == "claude")):
            raise RuntimeError("partial_finish_requires_complete_cold_and_untouched_smoke")
        report["partial_finish"] = "User authorized closing the unfinished Codex warm cohort and completing only the untouched Claude smoke."
        report["infrastructure_invalid_pairs"] = [
            {k: r[k] for k in ("harness", "cache", "task", "round")}
            for r in rows if r["state"] == "completed" and
            (not r["resource_before"]["stable"] or not r["resource_after"]["stable"]
             or r["resource_before"]["ac_power"] != r["resource_after"]["ac_power"])]
    report.setdefault("continuations", []).append({"from_report_sha256": digest(path),
        "prior_refusal": report.pop("refusal"), "first_trial_index": sum(r["state"] == "completed" for r in rows),
        "resumed_at_unix": int(time.time()), "collector_metadata": metadata})
    report["status"] = "planned"
    return report


def run(output, connectivity=False, resume=None, finish_partial=False):
    if finish_partial and (not resume or connectivity):
        raise RuntimeError("partial_finish_requires_campaign_checkpoint")
    tasks = json.loads((DOCS / "tasks.json").read_text()) + json.loads((DOCS / "workflow-tasks.json").read_text())
    plan = []
    for harness, caches, selected, rounds in [("codex", ["cold", "warm"], tasks, 3),
                                              ("claude", ["warm"], [tasks[0], tasks[2]], 1)]:
        for cache in caches:
            for round_no in range(1, rounds + 1):
                for index, task in enumerate(selected):
                    arms = ["baseline", "candidate"] if ((round_no - 1) * len(selected) + index) % 2 == 0 else ["candidate", "baseline"]
                    for arm in arms:
                        plan.append(dict(harness=harness, cache=cache, task=task["id"], round=round_no, arm=arm, state="planned"))
    metadata = preflight()
    if not connectivity and metadata["source_dirty"]:
        raise RuntimeError("dirty_source")
    report = {"schema": "lao-subscription-workflow-1", "status": "planned", "budget_usd_max": 5,
              "registered_at_unix": int(time.time()), "expires_at_unix": int(time.time()) + 86400,
              "boundary": "parent_dispatch_through_independent_verification",
              "claim": "public manifest pilot; no promotion or inferred monetary savings",
              "token_limit": "native subscriptions; observed at completion, no hard per-request token reservation",
              "claude_scope": "two-task single-round warm smoke; separate from Codex comparison",
              "billing": "subscriptions_only_no_api_or_credit_purchase", "metadata": metadata, "trials": plan}
    if connectivity:
        plan = [dict(harness=h, cache="none", task="connectivity", round=1, arm="candidate", state="planned") for h in ("codex", "claude")]
        report["trials"] = plan
        report["kind"] = "connectivity_only"
    if resume:
        if connectivity:
            raise RuntimeError("cannot_resume_connectivity")
        report = continuation(resume, plan, metadata, finish_partial)
        plan = report["trials"]
    output.parent.mkdir(parents=True, exist_ok=True)
    if output.exists():
        raise RuntimeError("refuse_report_overwrite")
    atom(output, report)
    oracle = Oracle()
    try:
        for trial in plan:
            if trial["state"] == "completed":
                continue
            if finish_partial and trial["harness"] == "codex":
                continue
            arm_started = time.monotonic_ns()
            if STOP_REQUESTED:
                raise RuntimeError("interrupted")
            if time.time() >= report["expires_at_unix"]:
                raise RuntimeError("registration_expired")
            actual_version = checked([trial["harness"], "--version"]).decode().strip()
            if actual_version != metadata["versions"][trial["harness"]] or digest(Path(__file__)) != metadata["collector_sha256"]:
                raise RuntimeError("configuration_drift")
            if any(digest(path) != metadata["artifacts"][label] for label, path in artifact_paths().items()):
                raise RuntimeError("artifact_drift")
            limits = rate_limits()
            if not limits["within_subscription"]:
                raise RuntimeError("subscription_guard_refused")
            before = health()
            if not before["stable"]:
                raise RuntimeError("resource_guard_refused")
            gate = None
            root = None
            with tempfile.TemporaryDirectory(prefix="lao-arm-") as temp:
                state = Path(temp).resolve()
                os.chmod(state, 0o700)
                fixture = oracle.call(op="prepare", task=tasks[0]["id"] if connectivity else trial["task"])
                root = Path(fixture["root"]).resolve()
                FIXTURES.add(root)
                try:
                    if connectivity:
                        worker_home(state / "worker", 1, os.urandom(32).hex())
                    else:
                        endpoint = {}
                        if trial["cache"] == "warm":
                            resources = health()
                            ram = metadata["ram_bytes"]
                            reserve = max(8 * 1024 ** 3, ram * 35 // 100)
                            # Match the pinned model's 5 GiB allowance and Light reserve before loading.
                            if not resources["stable"] or ram * resources["available_percent"] // 100 - reserve < 5 * 1024 ** 3:
                                raise RuntimeError("resource_guard_refused")
                            endpoint = oracle.call(op="start")
                            warm(endpoint)
                        gate = Oracle(GATE)
                        caller = os.urandom(32).hex()
                        address = gate.call(op="gate", cache=trial["cache"], **endpoint, caller=caller)
                        worker_home(state / "worker", address["port"], caller)
                    before = health()
                    if STOP_REQUESTED:
                        raise RuntimeError("interrupted")
                    if not before["stable"]:
                        raise RuntimeError("dispatch_resource_guard_refused")
                    peers = [r for r in plan if r is not trial and r["state"] == "completed"
                             and all(r[k] == trial[k] for k in ("harness", "cache", "task", "round"))]
                    if any(r["resource_before"]["ac_power"] != before["ac_power"] for r in peers):
                        raise RuntimeError("pair_power_source_drift")
                    trial["state"] = "dispatched"
                    atom(output, report)
                    prompt = "Reply OK. Do not call tools." if connectivity else fixture["objective"] + "\nWritable file: " + fixture["file"]
                    started = time.monotonic_ns()
                    trial["setup_ms"] = (started - arm_started) // 1_000_000
                    observed = native(trial["harness"], root, fixture["file"], state,
                                      trial["arm"] == "candidate", state / "worker", prompt)
                    verdict = oracle.call(op="verify")
                    elapsed = max(1, (time.monotonic_ns() - started) // 1_000_000)
                    after = health()
                    trial.update(observed, **verdict, elapsed_ms=elapsed, resource_before=before, resource_after=after,
                                 observed_harness=actual_version, subscription_before=limits, state="completed")
                    if trial["harness"] == "claude" and observed["model"] != CLAUDE_MODEL:
                        raise RuntimeError("model_identity_drift")
                    print(json.dumps({k: trial.get(k) for k in ("harness", "cache", "task", "round", "arm", "parent_status", "route", "local_worker_status", "verified", "scope_ok", "elapsed_ms", "reason")}), flush=True)
                    if not after["stable"] or before["ac_power"] != after["ac_power"] or not verdict["scope_ok"]:
                        raise RuntimeError("postflight_resource_or_scope_refused")
                    if observed["reason"] in ("quota", "cost_limit"):
                        raise RuntimeError("execution_guard_refused")
                    if observed["reason"] == "interrupted":
                        raise RuntimeError("interrupted")
                    if connectivity and observed["parent_status"] != "complete":
                        raise RuntimeError("connectivity_failed")
                finally:
                    if gate:
                        gate.close()
                        try:
                            socket.create_connection(("127.0.0.1", address["port"]), timeout=0.2).close()
                        except OSError:
                            pass
                        else:
                            raise RuntimeError("gate_listener_survived")
                    if oracle.p.poll() is None:
                        if not connectivity:
                            oracle.call(op="stop")
                        oracle.call(op="dispose")
                    elif sys.exc_info()[0] is None:
                        raise RuntimeError("oracle_exited_during_cleanup")
                    if root.exists():
                        raise RuntimeError("fixture_cleanup_failed")
                    FIXTURES.discard(root)
                    trial["total_elapsed_ms"] = (time.monotonic_ns() - arm_started) // 1_000_000
                    trial["cleanup_ms"] = trial["total_elapsed_ms"] - trial.get("setup_ms", 0) - trial.get("elapsed_ms", 0)
                    trial["cleanup_ok"] = True
                    atom(output, report)
        report["status"] = "partial" if finish_partial else "complete"
        report["subscription_after"] = rate_limits()
    except (Exception, KeyboardInterrupt) as error:
        report["status"] = "refused"
        report["refusal"] = str(error) if isinstance(error, RuntimeError) else type(error).__name__
        raise
    finally:
        try:
            oracle.close()
        except Exception as error:
            report["status"] = "refused"
            report.setdefault("refusal", "oracle_cleanup_failed")
            report["cleanup_error"] = type(error).__name__
            raise
        finally:
            atom(output, report)
            write_summary(output, report)


def write_summary(output, report):
    rows = report["trials"]
    complete = [r for r in rows if r["state"] == "completed"]
    lines = ["# Subscription workflow report", "",
             "Status: **" + report["status"] + "**. " + str(len(complete)) + "/" + str(len(rows)) + " planned arm executions completed.", "",
             "Primary time runs from native parent dispatch through independent verification, including routing, local failures, review and repair. Setup and cleanup are recorded separately in the JSON. All completed attempts, including failures, enter the medians.", "",
             "| Harness | Local cache | Arm | Verified / planned | Parent completed | Verified Local only | Median primary seconds | Median total seconds |",
             "|---|---|---|---:|---:|---:|---:|---:|"]
    for harness, cache, arm in dict.fromkeys((r["harness"], r["cache"], r["arm"]) for r in rows):
        planned = [r for r in rows if (r["harness"], r["cache"], r["arm"]) == (harness, cache, arm)]
        done = [r for r in planned if r["state"] == "completed"]
        passed = [r for r in done if r.get("verified") and r.get("scope_ok")]
        median = lambda key: f"{statistics.median(r[key] for r in done) / 1000:.3f}" if done else "—"
        lines.append(f"| {harness} | {cache} | {arm} | {len(passed)}/{len(planned)} | "
                     f"{sum(r['parent_status'] == 'complete' for r in done)} | "
                     f"{sum(r['route'] == 'local' for r in passed)} | {median('elapsed_ms')} | {median('total_elapsed_ms')} |")
    lines += ["", "## Outcomes needing review", "",
              "| Harness | Cache | Task | Round | Arm | Parent | Local worker | Verified | Scope | Reason |",
              "|---|---|---|---:|---|---|---|---|---|---|"]
    failures = [r for r in complete if not r.get("verified") or not r.get("scope_ok") or r.get("parent_status") != "complete" or r.get("reason")]
    for r in failures:
        lines.append("| " + " | ".join(str(r.get(k) if r.get(k) is not None else "—") for k in
                     ("harness", "cache", "task", "round", "arm", "parent_status", "local_worker_status", "verified", "scope_ok", "reason")) + " |")
    if not failures:
        lines.append("| — | — | None among completed rows | — | — | — | — | — | — | — |")
    lines += ["", "## Limits", "",
              "- Codex is the main 12-task comparison. Claude is a separate two-task, single-round warm smoke.",
              "- The corpus contains JSON manifest excerpts, not full-project builds. Express is a two-task repository holdout, not an unseen task family.",
              "- Subscription authentication was required; no API fallback or credit purchase was enabled. The authorized additional-spend ceiling was $5. Native token observations are in the JSON; subscription tokens are not converted to dollar savings.",
              "- Codex account usage percentages are shared with concurrent account activity. They cannot attribute quota consumption or savings to this campaign alone.",
              "- Native CLIs report tokens at completion. This collector does not enforce the draft's per-request token reservation or scan the native encrypted provider traffic. It constrains source access and records the resulting scope independently.",
              "- Cold resets apply to the isolated local model, not provider-side caches. Warmup uses a fixed one-token local request. Resource and battery observations accompany every completed arm.",
              "- Failed workers can leave correct edits; parent completion is not the task verdict. No promotion, tail-percentile or general savings claim follows from this small pilot.", ""]
    if report.get("refusal"):
        lines += ["Refusal: `" + report["refusal"] + "`. Unexecuted rows remain planned in the JSON; they were not silently dropped.", ""]
    if report.get("continuations"):
        lines += ["Resource-pause continuations are recorded with original report hashes, starting indices and collector identities. Completed rows were retained unchanged; only the untouched schedule suffix ran. The product binaries and model/harness settings remained pinned, while collector orchestration revisions are recorded separately.", ""]
    if report.get("partial_finish"):
        lines += [report["partial_finish"], "", "Unexecuted Codex warm rows remain planned in the JSON and are closed from further execution. Infrastructure-invalid pairs remain identified separately and cannot support a paired comparison. Only the complete stable cold cohort is eligible for a full Codex paired export.", ""]
    output.with_suffix(".md").write_text("\n".join(lines))


def relay(file, audit):
    """Forward production MCP unchanged, enforcing the outer campaign's exact scope."""
    if file not in ("package.json", "settings.json", "products.json"):
        raise RuntimeError("relay_scope")
    p = subprocess.Popen([str(LAO), "mcp"], stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                         stderr=subprocess.DEVNULL, env=environment())
    calls = 0
    try:
        for line in iter(lambda: sys.stdin.buffer.readline(65537), b""):
            if len(line) > 65536:
                raise RuntimeError("relay_request_limit")
            request = json.loads(line)
            is_call = request.get("method") == "tools/call"
            if is_call:
                calls += 1
                params = request.get("params", {})
                allowed = calls == 1 and params.get("name") == "execute" and params.get("arguments", {}).get("allowed_paths") == [file]
                atom(audit, {"calls": calls, "status": "dispatched", "refused": not allowed})
                if not allowed:
                    response = {"jsonrpc": "2.0", "id": request.get("id"), "result": {"isError": True,
                                "content": [{"type": "text", "text": "Campaign packet scope refused."}]}}
                    print(json.dumps(response), flush=True)
                    continue
            p.stdin.write(line)
            p.stdin.flush()
            if "id" not in request:
                continue
            with selectors.DefaultSelector() as sel:
                sel.register(p.stdout, selectors.EVENT_READ)
                if not sel.select(610):
                    raise RuntimeError("relay_timeout")
            answer = p.stdout.readline(65537)
            if not answer or len(answer) > 65536:
                raise RuntimeError("relay_response_limit")
            response = json.loads(answer)
            if response.get("id") != request["id"]:
                raise RuntimeError("relay_response_id")
            if is_call:
                result = response.get("result", {})
                status = result.get("structuredContent", {}).get("status", "tool_error")
                if status not in ("cloud", "complete", "agent_failed", "timed_out", "tool_error"):
                    status = "tool_error"
                path = Path(file)
                after = task_digest(path)
                atom(audit, {"calls": calls, "status": status, "after_sha256": after})
            sys.stdout.buffer.write(answer)
            sys.stdout.buffer.flush()
    finally:
        p.kill()
        p.wait(timeout=5)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--mcp-relay", action="store_true", help=argparse.SUPPRESS)
    parser.add_argument("--allowed-file", help=argparse.SUPPRESS)
    parser.add_argument("--audit", type=Path, help=argparse.SUPPRESS)
    parser.add_argument("--run-subscriptions", action="store_true", help="Explicitly run the registered public campaign; no paid fallback")
    parser.add_argument("--connectivity", action="store_true", help="Two connection-only native calls, excluded from task evidence")
    parser.add_argument("--output", type=Path, default=DOCS / "subscription-2026-09-12.json")
    parser.add_argument("--resume", type=Path, help="Continue an untouched suffix after a resource pause or reviewed setup refusal; use a new output path")
    parser.add_argument("--finish-partial", action="store_true", help="Close unfinished Codex warm rows and run only the untouched Claude smoke after a reviewed resource refusal")
    args = parser.parse_args()
    for sig in (signal.SIGINT, signal.SIGTERM):
        signal.signal(sig, interrupted)
    try:
        if args.mcp_relay:
            relay(args.allowed_file, args.audit)
        elif args.run_subscriptions:
            run(args.output.resolve(), args.connectivity, args.resume, args.finish_partial)
        else:
            print(json.dumps(preflight(), indent=2))
    except Exception as error:
        code = str(error) if isinstance(error, RuntimeError) else type(error).__name__
        print("workflow refused: " + code, file=sys.stderr)
        return 1
    finally:
        cleanup()
    return 0


if __name__ == "__main__":
    sys.exit(main())
