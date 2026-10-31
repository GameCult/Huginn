"""Drives the real claude CLI (stream-json, --replay-user-messages) against fake.py
through one scenario and records its stdout and what was written to its stdin.

  PROBE_SCENARIO  folded | queued | three-queued | serialized | ceiling-flush-folded |
                  ceiling-flush-queued | api-error-first | blocking-limit | compaction-off-360k
                  (three-queued writes a second and a third line while the first request is in
                  flight: the CLI joins them into one echo. serialized is the actuator's policy
                  with the same lines: `want` holds a line until every line sent was echoed, and
                  coalesces what waits into one line)
  PROBE_BASE      a scratch directory (config and work dirs are made in it)
  PROBE_OUT       where stream.jsonl is written: one JSON object per line,
                  {"t": seconds, "line": <claude stdout line>}, {"t": seconds, "sent": <our stdin text>} or
                  (serialized) {"t": seconds, "want": <a line the policy was asked to send>}
  SCRUB           1 (default) sets CLAUDE_CODE_SUBPROCESS_ENV_SCRUB=1 as the actuator does; 0 turns it off
  CLAUDE          the claude binary (default /opt/claude-code/2.1.312/claude)
  PROBE_PORT      the fake port (default 18611)

DISABLE_COMPACT=1 is always set, as the actuator sets it. No credential is used.
"""
import json
import os
import subprocess
import sys
import threading
import time
import uuid

SCENARIO = os.environ["PROBE_SCENARIO"]
BASE = os.environ["PROBE_BASE"]
OUT = os.environ["PROBE_OUT"]
CFG = f"{BASE}/cfg"
WORK = f"{BASE}/work"
for d in (OUT, CFG, WORK):
    os.makedirs(d, exist_ok=True)
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import fake  # noqa: E402

port = fake.start(int(os.environ.get("PROBE_PORT", "18611")))
json.dump({}, open(f"{CFG}/settings.json", "w"))
json.dump({"hasCompletedOnboarding": True}, open(f"{CFG}/.claude.json", "w"))
T0 = time.time()
CLAUDE = os.environ.get("CLAUDE", "/opt/claude-code/2.1.312/claude")
args = [CLAUDE, "-p", "--input-format", "stream-json", "--output-format", "stream-json", "--verbose", "--session-id", str(uuid.uuid4()),
        "--max-turns", "20", "--permission-mode", "bypassPermissions", "--permission-prompts", "none", "--setting-sources", "user",
        "--replay-user-messages", "--allowedTools", "Bash"]
env = {"PATH": f"{os.path.dirname(CLAUDE)}:/usr/local/bin:/usr/bin:/bin", "HOME": CFG, "CLAUDE_CONFIG_DIR": CFG,
       "CLAUDE_CODE_SUBPROCESS_ENV_SCRUB": os.environ.get("SCRUB", "1"), "DISABLE_COMPACT": "1", "LANG": "C.UTF-8",
       "ANTHROPIC_API_KEY": "sk-ant-fake-probe", "ANTHROPIC_BASE_URL": f"http://127.0.0.1:{port}",
       "CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC": "1", "DISABLE_AUTOUPDATER": "1"}
p = subprocess.Popen(args, stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=open(f"{OUT}/stderr.txt", "w"), cwd=WORK, env=env, text=True)
events = []
out = open(f"{OUT}/stream.jsonl", "w")
lock = threading.Lock()


def record(entry):
    with lock:
        out.write(json.dumps(dict(t=round(time.time() - T0, 2), **entry)) + "\n")
        out.flush()


sent = 0
pending = []
plock = threading.Lock()
serial = False


def echoes():
    return sum(1 for e in events if e.get("type") == "user" and e.get("isReplay") is True and isinstance(e.get("message", {}).get("content"), str))


def pump():
    """The one-outstanding policy: send what waits, as one line, only when every line sent was echoed."""
    with plock:
        if serial and pending and echoes() >= sent:
            text = "\n\n".join(pending)
            pending.clear()
            send(text)


def want(text):
    record({"want": text})
    pending.append(text)
    pump()


def reader():
    for line in p.stdout:
        record({"line": line.rstrip("\n")})
        try:
            events.append(json.loads(line))
        except Exception:
            pass
        pump()


threading.Thread(target=reader, daemon=True).start()


def send(text):
    global sent
    sent += 1
    p.stdin.write(json.dumps({"type": "user", "message": {"role": "user", "content": text}}) + "\n")
    p.stdin.flush()
    record({"sent": text})


def results():
    return sum(1 for e in events if e.get("type") == "result")


def assistants():
    return sum(1 for e in events if e.get("type") == "assistant")


def wait(pred, limit=60):
    end = time.time() + limit
    while time.time() < end and not pred() and p.poll() is None:
        time.sleep(0.05)


FLUSH = "The context ceiling is reached. Admit to the mind what it lacks, close your run, and end."
if SCENARIO == "folded":
    send("SLEEPCMD turn C.")
    time.sleep(8)
    send("FOLDED line D.")
    wait(lambda: results() >= 1)
    time.sleep(3)
elif SCENARIO == "queued":
    send("SLOWEND turn A.")
    time.sleep(4)
    send("QUEUED line B.")
    wait(lambda: results() >= 2)
elif SCENARIO == "three-queued":
    send("SLOWEND turn A.")
    time.sleep(3)
    send("QUEUED line B.")
    time.sleep(2)
    send("QUEUED line C.")
    wait(lambda: results() >= 3, 45)
    time.sleep(3)
elif SCENARIO == "serialized":
    serial = True
    send("SLOWEND turn A.")
    time.sleep(3)
    want("QUEUED line B.")
    time.sleep(2)
    want("CEIL300 QUEUED line C.")

    def ceiling_request():
        def usage(e):
            return (e.get("message") or {}).get("usage") or {}

        return any(e.get("type") == "assistant" and sum(usage(e).get(k) or 0 for k in ("input_tokens", "cache_read_input_tokens", "cache_creation_input_tokens")) >= 300000 for e in events)

    wait(ceiling_request, 60)
    want(FLUSH)
    wait(lambda: echoes() >= sent and results() >= 3, 60)
    time.sleep(3)
elif SCENARIO == "ceiling-flush-folded":
    send("SLEEPCMD CEIL300 turn.")
    wait(lambda: assistants() >= 1)
    send(FLUSH)
    wait(lambda: results() >= 1)
    time.sleep(3)
elif SCENARIO == "ceiling-flush-queued":
    send("CEIL300 turn.")
    wait(lambda: assistants() >= 1)
    send(FLUSH)
    wait(lambda: results() >= 2, 20)
elif SCENARIO == "api-error-first":
    send("APIERROR turn.")
    wait(lambda: results() >= 1)
elif SCENARIO == "blocking-limit":
    send("BIGCONTEXT turn E.")
    wait(lambda: results() >= 1)
    send("after big F.")
    wait(lambda: results() >= 2)
elif SCENARIO == "compaction-off-360k":
    send("MID360 turn E.")
    wait(lambda: results() >= 1)
    send("after 360 F.")
    wait(lambda: results() >= 2)
else:
    sys.exit(f"unknown scenario {SCENARIO}")
p.stdin.close()
try:
    p.wait(30)
except Exception:
    p.kill()
record({"exit": p.returncode})
