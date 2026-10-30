#!/usr/bin/env python3
"""Mock gateway tra 429 de test luong ep usage 100%."""
import http.server
import json
import sys

class Handler(http.server.BaseHTTPRequestHandler):
    def do_POST(self):
        if self.path.endswith("/chat/completions"):
            self.send_response(429)
            self.send_header("Content-Type", "application/json")
            body = json.dumps({"error": {"message": "Rate limit exceeded", "code": 429}}).encode()
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)
        else:
            self.send_response(404)
            self.end_headers()

    def log_message(self, fmt, *args):
        pass

if __name__ == "__main__":
    port = int(sys.argv[1]) if len(sys.argv) > 1 else 3999
    server = http.server.HTTPServer(("127.0.0.1", port), Handler)
    print(f"mock gateway 429 tren cong {port}", flush=True)
    server.serve_forever()
