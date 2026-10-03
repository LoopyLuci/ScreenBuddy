"""Stub server shaped like 9Router's documented API, used by the integration test.

The Rust test `nine_router_talks_to_a_router_shaped_server` binds this and drives
ScreenBuddy's real engine against it, so the request path is proven rather than
assumed from unit tests on URL strings.
"""

import json
import sys
import threading
from http.server import BaseHTTPRequestHandler, HTTPServer

# Every request the server received, so the test can assert on headers and body.
CAPTURED = []

# Overridden by the test to control what the fake router answers with.
STATE = {"models": {}, "chat": {}, "chat_status": 200, "models_status": 200}


class Handler(BaseHTTPRequestHandler):
    def log_message(self, *args):
        """Silence the default stderr access log."""

    def _json(self, code, payload):
        body = json.dumps(payload).encode()
        self.send_response(code)
        self.send_header("content-type", "application/json")
        self.send_header("content-length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def do_GET(self):
        CAPTURED.append(
            {"method": "GET", "path": self.path, "auth": self.headers.get("Authorization")}
        )
        if self.path.rstrip("/").endswith("/models"):
            self._json(STATE["models_status"], STATE["models"])
        else:
            self._json(404, {"error": "not found"})

    def do_POST(self):
        length = int(self.headers.get("content-length", 0))
        raw = self.rfile.read(length)
        try:
            body = json.loads(raw or b"{}")
        except json.JSONDecodeError:
            body = {"__unparseable": True}
        CAPTURED.append(
            {
                "method": "POST",
                "path": self.path,
                "auth": self.headers.get("Authorization"),
                "content_type": self.headers.get("Content-Type"),
                "body": body,
            }
        )
        if self.path.rstrip("/").endswith("/chat/completions"):
            self._json(STATE["chat_status"], STATE["chat"])
        else:
            self._json(404, {"error": "unknown path"})


def free_port():
    import socket

    s = socket.socket()
    s.bind(("127.0.0.1", 0))
    port = s.getsockname()[1]
    s.close()
    return port


def start(port=None, models=None, chat=None, chat_status=200, models_status=200):
    """Start the stub router on a free port. Returns (base_url, state)."""
    state = {
        "models": models
        if models is not None
        else {
            "data": [
                {"id": "cc/claude-opus-4-6", "owned_by": "anthropic"},
                {"id": "openai/gpt-4o", "owned_by": "openai"},
                {"id": "free/llama-3.3-70b"},
            ]
        },
        "chat": chat
        if chat is not None
        else {
            "choices": [
                {
                    "message": {"role": "assistant", "content": "Routed through 9Router."},
                    "finish_reason": "stop",
                }
            ],
            "usage": {"total_tokens": 42},
        },
        "chat_status": chat_status,
        "models_status": models_status,
    }

    srv = HTTPServer(("127.0.0.1", port or free_port()), Handler)
    srv.requests = CAPTURED
    srv.state = state
    threading.Thread(target=srv.serve_forever, daemon=True).start()
    base = "http://127.0.0.1:{}/v1".format(srv.server_address[1])
    return base, srv, state


if __name__ == "__main__":
    # Accept a port so a test can pin one; without it, pick a free port.
    port = int(sys.argv[1]) if len(sys.argv) > 1 else None
    base, srv, state = start(port)
    # The handlers read module-level STATE, so publish what start() built.
    # Without this every reply is the empty default and a router appears to
    # answer with nothing useful.
    globals()["STATE"].update(state)
    print("stub 9router listening on", base, flush=True)
    try:
        import time

        while True:
            time.sleep(1)
    except KeyboardInterrupt:
        srv.shutdown()