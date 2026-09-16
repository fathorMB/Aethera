"""M-08 T-10 — chi rompe la cache del prefisso.

Un ponte fra un client e il motore: inoltra tutto senza toccarlo e, per ogni richiesta con un
prompt, scrive una riga con quanto del prompt precedente e' sopravvissuto. Un client che tiene il
prefisso stabile ha un prefisso comune quasi lungo quanto la richiesta precedente; un client che
riscrive l'inizio (un'intestazione con l'ora, un identificativo nuovo a ogni turno, i messaggi
riordinati) lo azzera, e il motore deve rielaborare tutto.

    python prefix_proxy.py --port 8081 --upstream http://127.0.0.1:8080 --label opencode \
        --out C:/AetheraData/m08/T-10-prefissi.jsonl

Il client va puntato su http://127.0.0.1:8081. Non si misura il client a parole: si misura
il byte in cui la sua richiesta smette di assomigliare alla precedente.
"""
import argparse
import hashlib
import http.client
import json
import os
import sys
import threading
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from urllib.parse import urlparse

if hasattr(sys.stdout, "reconfigure"):
    sys.stdout.reconfigure(encoding="utf-8")

STATE = {"prev": {}, "lock": threading.Lock(), "n": 0}


def prompt_text(body):
    """Il testo su cui il motore costruisce la cache, nelle due forme che usano i client."""
    try:
        v = json.loads(body)
    except Exception:
        return None
    if isinstance(v.get("prompt"), str):
        return v["prompt"]
    msgs = v.get("messages")
    if isinstance(msgs, list):
        out = []
        for m in msgs:
            c = m.get("content")
            if isinstance(c, list):
                # Le parti senza testo (tool_use, tool_result dell'API Anthropic) contano anche loro.
                c = "".join(p["text"] if isinstance(p.get("text"), str) else json.dumps(p, sort_keys=True)
                            for p in c if isinstance(p, dict))
            if m.get("tool_calls"):
                c = (c or "") + json.dumps(m["tool_calls"], sort_keys=True)
            out.append("%s\n%s" % (m.get("role", ""), c or ""))
        # L'API Anthropic tiene il prompt di sistema fuori dai messaggi: è lì che Claude Code mette
        # l'intestazione di attribuzione, quindi va in testa come la rende il template.
        system = v.get("system")
        if isinstance(system, list):
            system = "".join(p.get("text", "") for p in system if isinstance(p, dict))
        if isinstance(system, str) and system:
            out.insert(0, "system\n" + system)
        # Gli strumenti dichiarati stanno in testa al prompt reso dal template: contano.
        if v.get("tools"):
            out.insert(0, json.dumps(v["tools"], sort_keys=True))
        return "\n".join(out)
    return None


def common_prefix(a, b):
    n = min(len(a), len(b))
    lo, hi = 0, n
    while lo < hi:
        mid = (lo + hi + 1) // 2
        if a[:mid] == b[:mid]:
            lo = mid
        else:
            hi = mid - 1
    return lo


class Handler(BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.1"

    def log_message(self, *a):  # niente rumore sullo stderr
        pass

    def _relay(self, method):
        length = int(self.headers.get("Content-Length") or 0)
        body = self.rfile.read(length) if length else b""
        if body:
            self._record(body)
        up = urlparse(self.server.upstream)
        conn = http.client.HTTPConnection(up.hostname, up.port or 80, timeout=3600)
        headers = {k: v for k, v in self.headers.items() if k.lower() not in ("host", "connection")}
        conn.request(method, self.path, body=body, headers=headers)
        r = conn.getresponse()
        self.send_response(r.status)
        for k, v in r.getheaders():
            if k.lower() in ("connection", "transfer-encoding", "content-length"):
                continue
            self.send_header(k, v)
        self.send_header("Transfer-Encoding", "chunked")
        self.end_headers()
        while True:
            chunk = r.read(8192)
            if not chunk:
                break
            self.wfile.write(b"%X\r\n%s\r\n" % (len(chunk), chunk))
            self.wfile.flush()
        self.wfile.write(b"0\r\n\r\n")
        self.wfile.flush()
        conn.close()

    def _record(self, body):
        text = prompt_text(body)
        if text is None:
            return
        with STATE["lock"]:
            prev = STATE["prev"].get(self.path, "")
            shared = common_prefix(prev, text)
            STATE["n"] += 1
            row = {
                "at": time.strftime("%Y-%m-%dT%H:%M:%S"),
                "label": self.server.label,
                "path": self.path,
                "n": STATE["n"],
                "chars": len(text),
                "prev_chars": len(prev),
                "shared_prefix_chars": shared,
                "shared_share": round(shared / len(prev), 4) if prev else None,
                "head_sha12": hashlib.sha256(text[:2000].encode("utf-8")).hexdigest()[:12],
                "head": text[:200],
            }
            STATE["prev"][self.path] = text
            with open(self.server.out, "a", encoding="utf-8") as fh:
                fh.write(json.dumps(row, ensure_ascii=False) + "\n")
            print("%s #%d %d caratteri · prefisso comune %d (%s)" % (
                self.server.label, row["n"], row["chars"], shared,
                "%.1f%%" % (100 * row["shared_share"]) if row["shared_share"] is not None else "primo"))

    def do_POST(self):
        self._relay("POST")

    def do_GET(self):
        self._relay("GET")


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--port", type=int, default=8081)
    ap.add_argument("--upstream", default="http://127.0.0.1:8080")
    ap.add_argument("--label", default="client")
    ap.add_argument("--out", default="T-10-prefissi.jsonl")
    a = ap.parse_args()
    os.makedirs(os.path.dirname(os.path.abspath(a.out)), exist_ok=True)
    srv = ThreadingHTTPServer(("127.0.0.1", a.port), Handler)
    srv.upstream, srv.label, srv.out = a.upstream, a.label, a.out
    print("ponte su 127.0.0.1:%d → %s · righe in %s" % (a.port, a.upstream, a.out))
    srv.serve_forever()


if __name__ == "__main__":
    main()
