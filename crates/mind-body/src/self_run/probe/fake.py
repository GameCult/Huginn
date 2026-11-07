"""A fake Messages API for probing the claude CLI stream-json session: it answers
every request with a scripted text or a Bash tool call and reports scripted
usage. A marker in the newest user text picks the behaviour:

  SLEEPCMD   answer with a 25 s Bash tool call (the turn stays open)
  SLOWEND    hold the closing text request for 12 s
  APIERROR   answer 400 (the CLI reports an api_error result)
  CEIL300    report 300000 input tokens (the actuator ceiling)
  MID360     report 360000 input tokens
  BIGCONTEXT report 990000 input tokens (the CLI blocking limit)

Holds no credential: the CLI is pointed at it with a fake key.
"""
import json
import http.server
import os
import threading
import time

OUT = os.environ["PROBE_OUT"]
T0 = time.time()
N = [0]
MAIN = [0]
LOCK = threading.Lock()


def sse(w, ev, data):
    w.write(f"event: {ev}\ndata: {json.dumps(data)}\n\n".encode())
    w.flush()


def reply(h, code, body):
    h.send_response(code)
    h.send_header("content-type", "application/json")
    h.send_header("content-length", str(len(body)))
    h.end_headers()
    h.wfile.write(body)


class H(http.server.BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.1"

    def log_message(self, *a):
        pass

    def do_GET(self):
        reply(self, 200, b"{}")

    def stream(self, n, req, block, delta, stop, tokens):
        usage = {"input_tokens": tokens, "output_tokens": 5, "cache_creation_input_tokens": 0, "cache_read_input_tokens": 0}
        self.send_response(200)
        self.send_header("content-type", "text/event-stream")
        self.send_header("cache-control", "no-cache")
        self.send_header("connection", "close")
        self.end_headers()
        w = self.wfile
        sse(w, "message_start", {"type": "message_start", "message": {"id": f"msg_{n:04d}", "type": "message", "role": "assistant", "model": req.get("model", "x"), "content": [], "stop_reason": None, "stop_sequence": None, "usage": usage}})
        sse(w, "content_block_start", {"type": "content_block_start", "index": 0, "content_block": block})
        sse(w, "content_block_delta", {"type": "content_block_delta", "index": 0, "delta": delta})
        sse(w, "content_block_stop", {"type": "content_block_stop", "index": 0})
        sse(w, "message_delta", {"type": "message_delta", "delta": {"stop_reason": stop, "stop_sequence": None}, "usage": {"output_tokens": 5}})
        sse(w, "message_stop", {"type": "message_stop"})
        self.close_connection = True

    def do_POST(self):
        body = self.rfile.read(int(self.headers.get("content-length", 0)))
        with LOCK:
            N[0] += 1
            n = N[0]
        try:
            req = json.loads(body)
        except Exception:
            req = {}
        msgs = req.get("messages", [])
        last_assistant = max([i for i, m in enumerate(msgs) if m.get("role") == "assistant"], default=-1)
        tail = json.dumps(msgs[last_assistant + 1:])
        tools = [t.get("name") for t in req.get("tools", [])]
        rec = {"n": n, "t": round(time.time() - T0, 2), "path": self.path, "nmsgs": len(msgs), "main": "Bash" in tools, "tail": tail[-500:]}
        open(f"{OUT}/requests.jsonl", "a").write(json.dumps(rec) + "\n")
        if "count_tokens" in self.path:
            reply(self, 200, json.dumps({"input_tokens": 100}).encode())
            return
        if "APIERROR" in tail:
            reply(self, 400, json.dumps({"type": "error", "error": {"type": "invalid_request_error", "message": "probe: bad request"}}).encode())
            return
        first = json.dumps(msgs[0]) if msgs else ""
        if "SUBTASK" in first and '"tool_result"' not in tail:
            time.sleep(6)
            self.stream(n, req, {"type": "text", "text": ""}, {"type": "text_delta", "text": "SUBTASK done"}, "end_turn", 10)
            return
        plan = [p for p in os.environ.get("FAKE_PLAN", "").split(",") if p]
        step = "text"
        if "Bash" in tools:
            with LOCK:
                MAIN[0] += 1
                k = MAIN[0]
            if k <= len(plan):
                step = plan[k - 1]
        kind, _, arg = step.partition(":")
        if kind in ("agent", "monitor"):
            name, inp = (("Agent", {"description": "probe agent", "prompt": "SUBTASK say done", "subagent_type": "general-purpose", "run_in_background": True}) if kind == "agent" else ("Monitor", {"description": "probe monitor", "command": "sleep 6; echo monitor-done", "timeout_ms": 30000, "persistent": False}))
            self.stream(n, req, {"type": "tool_use", "id": f"toolu_{n:04d}", "name": name, "input": {}}, {"type": "input_json_delta", "partial_json": json.dumps(inp)}, "tool_use", 10)
            return
        if kind == "hold":
            time.sleep(float(arg))
            kind = "text"
        if kind in ("bg", "fg"):
            block = {"type": "tool_use", "id": f"toolu_{n:04d}", "name": "Bash", "input": {}}
            delta = {"type": "input_json_delta", "partial_json": json.dumps({"command": f"python3 -c 'import time; time.sleep({arg})'; echo probe-done", "description": f"probe {kind}", "run_in_background": kind == "bg", "dangerouslyDisableSandbox": True})}
            stop = "tool_use"
        elif plan:
            block = {"type": "text", "text": ""}
            delta = {"type": "text_delta", "text": f"ANSWER to request {n}"}
            stop = "end_turn"
        elif "SLEEPCMD" in tail and '"tool_result"' not in tail:
            block = {"type": "tool_use", "id": f"toolu_{n:04d}", "name": "Bash", "input": {}}
            delta = {"type": "input_json_delta", "partial_json": json.dumps({"command": "python3 -c 'import time; time.sleep(25)'", "description": "probe wait"})}
            stop = "tool_use"
        else:
            block = {"type": "text", "text": ""}
            delta = {"type": "text_delta", "text": f"ANSWER to request {n}"}
            stop = "end_turn"
        tokens = 10
        for marker, value in (("CEIL300", 300000), ("MID360", 360000), ("BIGCONTEXT", 990000)):
            if marker in tail:
                tokens = value
        usage = {"input_tokens": tokens, "output_tokens": 5, "cache_creation_input_tokens": 0, "cache_read_input_tokens": 0}
        self.send_response(200)
        self.send_header("content-type", "text/event-stream")
        self.send_header("cache-control", "no-cache")
        self.send_header("connection", "close")
        self.end_headers()
        w = self.wfile
        sse(w, "message_start", {"type": "message_start", "message": {"id": f"msg_{n:04d}", "type": "message", "role": "assistant", "model": req.get("model", "x"), "content": [], "stop_reason": None, "stop_sequence": None, "usage": usage}})
        sse(w, "content_block_start", {"type": "content_block_start", "index": 0, "content_block": block})
        if "SLOWEND" in tail and stop == "end_turn":
            time.sleep(12)
        sse(w, "content_block_delta", {"type": "content_block_delta", "index": 0, "delta": delta})
        sse(w, "content_block_stop", {"type": "content_block_stop", "index": 0})
        sse(w, "message_delta", {"type": "message_delta", "delta": {"stop_reason": stop, "stop_sequence": None}, "usage": {"output_tokens": 5}})
        sse(w, "message_stop", {"type": "message_stop"})
        self.close_connection = True


def start(port):
    server = http.server.ThreadingHTTPServer(("127.0.0.1", port), H)
    threading.Thread(target=server.serve_forever, daemon=True).start()
    return port
