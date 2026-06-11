#!/usr/bin/env python3
"""Verification harness for a local-model review run.

A minimal MCP stdio client that spawns `sah serve --model qwen --cwd
scripts/review-verify/sample`, performs the MCP handshake (sequencing each
step on the actual prior response, not on timers), calls the `review` tool
with backend=local over the seeded sample crate, and asserts a
machine-checkable success:

  1. the review markdown is non-empty AND findings (blockers+warnings+nits) > 0
  2. counts.failed == 0 and counts.attempted > 0 (the serialized names of the
     engine's tasks_failed / tasks_attempted tallies)
  3. zero "Queue is full" lines in the sample dir's .sah/mcp.<pid>.log
  4. at least one "AgentMessage (" reply line in that log

Exits 0 only when every assertion holds; nonzero with a clear message
otherwise. See README.md alongside this script for setup (build `sah` first).

`--self-test` exercises the assertion logic against synthetic fixtures without
spawning a server or a model.
"""

import argparse
import json
import subprocess
import sys
import threading
import time
from pathlib import Path

SCRIPT_DIR = Path(__file__).resolve().parent
SAMPLE_DIR = SCRIPT_DIR / "sample"
REVIEW_FILE = SAMPLE_DIR / "src" / "orders.rs"
VALIDATORS = ["duplication", "magic-numbers"]
REVIEW_TIMEOUT_SECONDS = 1800
# initialize / tools/list are answered without touching the model, but a cold
# start may still be slow; generous, yet far below the review timeout so a
# wedged handshake fails with a step-named message instead of masquerading as
# a review timeout.
HANDSHAKE_STEP_TIMEOUT_SECONDS = 120
# How long to give the server to exit after SIGTERM before escalating to
# SIGKILL during shutdown.
SHUTDOWN_WAIT_SECONDS = 10

JSONRPC_VERSION = "2.0"
JSONRPC_METHOD_NOT_FOUND = -32601  # JSON-RPC spec: "Method not found"
INITIALIZE_ID = 1
TOOLS_LIST_ID = 2
REVIEW_CALL_ID = 3

EXIT_SUCCESS = 0
EXIT_ASSERTIONS_FAILED = 1
EXIT_TIMEOUT = 2

QUEUE_FULL_MARKER = "Queue is full"
AGENT_MESSAGE_MARKER = "AgentMessage ("


def rpc(**fields):
    """Build a JSON-RPC 2.0 envelope (request, notification, or response)."""
    return {"jsonrpc": JSONRPC_VERSION, **fields}


# ---------------------------------------------------------------------------
# Self-test fixtures: synthetic tools/call responses and server logs.
# ---------------------------------------------------------------------------

def _response(markdown, counts):
    """A synthetic successful tools/call JSON-RPC response for the review tool."""
    payload = {"markdown": markdown, "counts": counts}
    return rpc(
        id=REVIEW_CALL_ID,
        result={"content": [{"type": "text", "text": json.dumps(payload)}]},
    )


def _counts(blockers=1, warnings=1, nits=0, attempted=4, failed=0):
    """A synthetic `counts` payload mirroring the serialized ReviewCountsView."""
    return {
        "blockers": blockers,
        "warnings": warnings,
        "nits": nits,
        "confirmed": blockers + warnings + nits,
        "refuted": 0,
        "attempted": attempted,
        "failed": failed,
    }


GOOD_MARKDOWN = (
    "## Review Findings (2026-06-10 12:00)\n\n"
    "- [ ] WARNING orders.rs: duplicated tax computation\n"
)
GOOD_LOG = (
    "INFO session=abc, AgentMessage (42 chars): {\"findings\": []}\n"
    "INFO fleet task finished\n"
)


def self_test():
    """Exercise the assertion logic on synthetic pass/fail fixtures.

    Each case is (name, failures, expected): `expected` is None when the
    fixture must produce no failures, otherwise a substring that must appear
    in one of the failure messages — so every case fails for exactly its
    named reason, not for an incidental one.

    Returns the number of failed cases; prints one line per case.
    """
    cases = [
        (
            "passing review response yields no failures",
            check_review_response(_response(GOOD_MARKDOWN, _counts())),
            None,
        ),
        (
            "JSON-RPC error response is rejected",
            check_review_response(
                rpc(id=REVIEW_CALL_ID, error={"code": -32000, "message": "boom"})
            ),
            "JSON-RPC error",
        ),
        (
            "isError tool result is rejected",
            check_review_response(
                rpc(
                    id=REVIEW_CALL_ID,
                    result={
                        "isError": True,
                        "content": [{"type": "text", "text": "incomplete review"}],
                    },
                )
            ),
            "error result",
        ),
        (
            "empty markdown is rejected",
            check_review_response(_response("", _counts())),
            "markdown is empty",
        ),
        (
            "zero findings is rejected",
            check_review_response(
                _response(GOOD_MARKDOWN, _counts(blockers=0, warnings=0, nits=0))
            ),
            "zero findings",
        ),
        (
            "nonzero failed task tally is rejected",
            check_review_response(_response(GOOD_MARKDOWN, _counts(failed=1))),
            "tasks failed",
        ),
        (
            "zero attempted tasks is rejected",
            check_review_response(_response(GOOD_MARKDOWN, _counts(attempted=0))),
            "tasks were attempted",
        ),
        (
            "clean log with an agent reply yields no failures",
            check_server_log(GOOD_LOG),
            None,
        ),
        (
            "a Queue is full line is rejected",
            check_server_log(GOOD_LOG + "WARN Queue is full, dropping task\n"),
            QUEUE_FULL_MARKER,
        ),
        (
            "a log with no AgentMessage reply is rejected",
            check_server_log("INFO fleet task finished\n"),
            AGENT_MESSAGE_MARKER,
        ),
    ]

    failed = 0
    for name, failures, expected in cases:
        if expected is None:
            ok = not failures
        else:
            ok = any(expected in failure for failure in failures)
        status = "PASS" if ok else "FAIL"
        detail = "" if ok else f" — expected {expected!r}, got failures={failures}"
        print(f"[{status}] {name}{detail}")
        if not ok:
            failed += 1
    return failed


# ---------------------------------------------------------------------------
# Assertion logic (assertions 1+2 over the tool response, 3+4 over the log).
# ---------------------------------------------------------------------------

def check_review_response(message):
    """Validate the tools/call JSON-RPC response for the review op.

    Covers assertions 1 and 2: non-empty markdown with findings > 0, zero
    failed fan-out tasks, and at least one attempted task. Returns a list of
    human-readable failure strings (empty when everything holds).
    """
    if "error" in message:
        return [f"tools/call returned a JSON-RPC error: {message['error']}"]
    result = message.get("result") or {}
    content = result.get("content") or []
    text = next(
        (c.get("text") for c in content if isinstance(c, dict) and c.get("text")), None
    )
    if result.get("isError"):
        return [f"review tool returned an error result: {text or result}"]
    if text is None:
        return [f"review tool result has no text content: {result}"]
    try:
        payload = json.loads(text)
    except ValueError:
        return [f"review tool result text is not JSON: {text!r}"]

    failures = []
    markdown = payload.get("markdown") or ""
    counts = payload.get("counts") or {}
    findings = sum(counts.get(k, 0) for k in ("blockers", "warnings", "nits"))
    if not markdown.strip():
        failures.append("review markdown is empty")
    if findings <= 0:
        failures.append(
            f"review reported zero findings (counts={counts}) — the sample crate "
            "has planted duplication and magic-number findings"
        )
    if counts.get("attempted", 0) <= 0:
        failures.append(f"no fan-out review tasks were attempted (counts={counts})")
    if counts.get("failed", 0) != 0:
        failures.append(
            f"{counts.get('failed')} fan-out review tasks failed (counts={counts}) "
            "— the findings are incomplete"
        )
    return failures


def check_server_log(log_text):
    """Validate the server's per-pid .sah/mcp.<pid>.log contents.

    Covers assertions 3 and 4: zero "Queue is full" lines (the silent-drop
    symptom that turns a review into an empty clean pass) and at least one
    "AgentMessage (" reply line proving the local model actually answered.
    Returns a list of human-readable failure strings.
    """
    failures = []
    queue_full = sum(1 for line in log_text.splitlines() if QUEUE_FULL_MARKER in line)
    agent_replies = sum(
        1 for line in log_text.splitlines() if AGENT_MESSAGE_MARKER in line
    )
    if queue_full > 0:
        failures.append(
            f'{queue_full} "{QUEUE_FULL_MARKER}" lines in the server log — '
            "the agent queue dropped fan-out tasks"
        )
    if agent_replies == 0:
        failures.append(
            f'no "{AGENT_MESSAGE_MARKER}" reply lines in the server log — '
            "the local model never produced a reply"
        )
    return failures


# ---------------------------------------------------------------------------
# MCP stdio client.
# ---------------------------------------------------------------------------

def log(message):
    """Print a timestamped progress line to stdout and flush."""
    sys.stdout.write(f"[{time.strftime('%H:%M:%S')}] {message}\n")
    sys.stdout.flush()


def ensure_sample_git_repo():
    """`git init` the sample dir if needed.

    `sah serve` resolves its `.sah/` data dir (logs, code-context index) at the
    git root of its cwd; without a nested repo everything would land in this
    repository's root `.sah/` instead of the sample dir's. Git never tracks
    paths under a `.git` component, so the nested repo is invisible to the
    parent repository.
    """
    if not (SAMPLE_DIR / ".git").exists():
        subprocess.run(["git", "init", "--quiet", str(SAMPLE_DIR)], check=True)
        log(f"initialized nested git repo at {SAMPLE_DIR}")


class McpClient:
    """A minimal JSON-RPC-over-stdio client for one `sah serve` process.

    The reader thread records each response by request id and signals a
    per-id Event, so callers sequence the handshake on actual responses via
    `wait_for` instead of sleeping.
    """

    def __init__(self, proc):
        self.proc = proc
        self.responses = {}
        self.events = {
            request_id: threading.Event()
            for request_id in (INITIALIZE_ID, TOOLS_LIST_ID, REVIEW_CALL_ID)
        }
        self.stdin_lock = threading.Lock()
        self.thread = threading.Thread(target=self._reader, daemon=True)
        self.thread.start()

    def send(self, obj):
        """Write one newline-delimited JSON-RPC message to the server's stdin.

        Locked: both the main thread (requests) and the reader thread
        (declines of server->client requests) send, and interleaved writes
        would corrupt the newline-delimited stream.
        """
        with self.stdin_lock:
            self.proc.stdin.write(json.dumps(obj) + "\n")
            self.proc.stdin.flush()
        log(f"-> {obj.get('method', 'response')} id={obj.get('id')}")

    def wait_for(self, request_id, timeout):
        """Block until the response to `request_id` arrives; None on timeout."""
        if not self.events[request_id].wait(timeout=timeout):
            return None
        return self.responses.get(request_id)

    def _reader(self):
        for line in self.proc.stdout:
            line = line.strip()
            if not line:
                continue
            try:
                msg = json.loads(line)
            except ValueError:
                continue
            # Server -> client request: decline politely so nothing blocks.
            if "method" in msg and "id" in msg:
                self.send(
                    rpc(
                        id=msg["id"],
                        error={
                            "code": JSONRPC_METHOD_NOT_FOUND,
                            "message": "client has no handlers",
                        },
                    )
                )
                continue
            request_id = msg.get("id")
            if request_id in self.events:
                self.responses[request_id] = msg
                self.events[request_id].set()
                if request_id == REVIEW_CALL_ID:
                    return


def drive_review(client, timeout):
    """Perform the MCP handshake and the review tools/call against `client`.

    Each step waits for the previous response (per-step timeouts) so a slow
    or wedged handshake fails with a step-named message instead of a
    misleading whole-review timeout. Returns (result, error): the review
    response message on success, or a human-readable error string.
    """
    client.send(
        rpc(
            id=INITIALIZE_ID,
            method="initialize",
            params={
                "protocolVersion": "2025-06-18",
                "capabilities": {},
                "clientInfo": {"name": "review-verify", "version": "0"},
            },
        )
    )
    if client.wait_for(INITIALIZE_ID, HANDSHAKE_STEP_TIMEOUT_SECONDS) is None:
        return None, (
            f"no initialize response within {HANDSHAKE_STEP_TIMEOUT_SECONDS}s"
        )
    log("<- initialize ok")
    client.send(rpc(method="notifications/initialized"))

    client.send(rpc(id=TOOLS_LIST_ID, method="tools/list"))
    tools_msg = client.wait_for(TOOLS_LIST_ID, HANDSHAKE_STEP_TIMEOUT_SECONDS)
    if tools_msg is None:
        return None, (
            f"no tools/list response within {HANDSHAKE_STEP_TIMEOUT_SECONDS}s"
        )
    tools = [t["name"] for t in tools_msg.get("result", {}).get("tools", [])]
    log(f"<- tools/list ok: {tools}")
    if "review" not in tools:
        return None, "the server does not advertise a `review` tool"

    client.send(
        rpc(
            id=REVIEW_CALL_ID,
            method="tools/call",
            params={
                "name": "review",
                "arguments": {
                    "op": "review file",
                    "path": str(REVIEW_FILE),
                    "backend": "local",
                    "validators": VALIDATORS,
                },
            },
        )
    )
    result = client.wait_for(REVIEW_CALL_ID, timeout)
    if result is None:
        return None, f"no review result within {timeout}s"
    log("<- review result received")
    return result, None


def report(result, failures, log_path):
    """Print the review result, the log path, and the verdict; return exit code."""
    print("\n=== REVIEW RESULT ===")
    print(json.dumps(result.get("result", result), indent=2))
    print(f"\nserver log: {log_path}")

    if failures:
        print("\nFAIL: verification assertions failed:")
        for failure in failures:
            print(f"  - {failure}")
        return EXIT_ASSERTIONS_FAILED
    print("\nPASS: all verification assertions hold")
    return EXIT_SUCCESS


def shutdown_server(proc):
    """Terminate `proc` and reap it, escalating to SIGKILL if it ignores SIGTERM.

    Must complete before the server's `.sah` log is read: a wedged `sah serve`
    can ignore SIGTERM, hold the cross-process GPU lock, and keep appending to
    the log after the harness has read it.
    """
    proc.terminate()
    try:
        proc.wait(timeout=SHUTDOWN_WAIT_SECONDS)
    except subprocess.TimeoutExpired:
        proc.kill()
        proc.wait()


def run_review(timeout):
    """Spawn the server, drive one review, and run every assertion.

    Returns a process exit code: EXIT_SUCCESS on full success,
    EXIT_ASSERTIONS_FAILED on failed assertions, EXIT_TIMEOUT when any
    handshake step or the review call produces no usable response in time.
    """
    ensure_sample_git_repo()

    proc = subprocess.Popen(
        ["sah", "serve", "--model", "qwen", "--cwd", str(SAMPLE_DIR)],
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        cwd=str(SAMPLE_DIR),
        text=True,
        bufsize=1,
    )
    log(f"spawned sah serve pid={proc.pid}")
    log_path = SAMPLE_DIR / ".sah" / f"mcp.{proc.pid}.log"

    try:
        result, error = drive_review(McpClient(proc), timeout)
        if error:
            print(f"\nFAIL: {error}")
            return EXIT_TIMEOUT
    finally:
        # Reap before the log read below: the server must be dead (log
        # flushed/closed, GPU lock released) on every exit path.
        shutdown_server(proc)

    failures = check_review_response(result)
    if log_path.exists():
        failures += check_server_log(log_path.read_text(errors="replace"))
    else:
        failures.append(f"server log not found at {log_path}")

    return report(result, failures, log_path)


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument(
        "--self-test",
        action="store_true",
        help="run the assertion logic against synthetic fixtures (no server, no model)",
    )
    parser.add_argument(
        "--timeout",
        type=float,
        default=REVIEW_TIMEOUT_SECONDS,
        help=f"seconds to wait for the review result (default {REVIEW_TIMEOUT_SECONDS})",
    )
    args = parser.parse_args()

    if args.self_test:
        failed = self_test()
        if failed:
            print(f"\nself-test FAILED: {failed} case(s)")
            sys.exit(EXIT_ASSERTIONS_FAILED)
        print("\nself-test passed")
        sys.exit(EXIT_SUCCESS)

    sys.exit(run_review(args.timeout))


if __name__ == "__main__":
    main()
