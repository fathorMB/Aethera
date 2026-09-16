"""M-11 T-02: risposte a prompt fissi in inglese e in italiano, con e senza strumenti.
Uso: python coerenza.py <alias> <uscita.jsonl>   (motore acceso su 127.0.0.1:8080)"""
import json, sys, time, urllib.request

URL = "http://127.0.0.1:8080/v1/chat/completions"
alias, out = sys.argv[1], sys.argv[2]
TOOLS = [{"type": "function", "function": {"name": "read_file", "description": "Read a file from the workspace",
          "parameters": {"type": "object", "properties": {"path": {"type": "string"}}, "required": ["path"]}}}]
CASES = [
    ("en-code", "en", "Write a Rust function `fn dedup_sorted(v: &mut Vec<i32>)` that removes consecutive duplicates in place without allocating. Only the code.", False),
    ("it-code", "it", "Scrivi una funzione Rust `fn dedup_sorted(v: &mut Vec<i32>)` che toglie i duplicati consecutivi sul posto senza allocare. Spiega in italiano in due righe, poi il codice.", False),
    ("en-tool", "en", "There is a bug in src/cmdline.rs. Look at the file before answering.", True),
    ("it-tool", "it", "C'è un errore in src/cmdline.rs. Guarda il file prima di rispondere.", True),
]
with open(out, "w", encoding="utf-8") as f:
    for name, lang, prompt, tools in CASES:
        body = {"model": alias, "messages": [{"role": "user", "content": prompt}], "max_tokens": 1500,
                "temperature": 0.6, "top_p": 0.95, "seed": 1234}
        if tools:
            body["tools"] = TOOLS
        t = time.time()
        req = urllib.request.Request(URL, json.dumps(body).encode(), {"Content-Type": "application/json"})
        try:
            r = json.load(urllib.request.urlopen(req, timeout=900))
            m = r["choices"][0]["message"]
            row = {"case": name, "lang": lang, "s": round(time.time() - t, 1),
                   "finish": r["choices"][0].get("finish_reason"), "content": m.get("content"),
                   "reasoning": m.get("reasoning_content"),
                   "tool_calls": m.get("tool_calls"), "usage": r.get("usage"), "timings": r.get("timings")}
        except Exception as e:
            row = {"case": name, "error": str(e)}
        f.write(json.dumps(row, ensure_ascii=False) + "\n"); f.flush()
        print(name, row.get("finish"), row.get("s"), (row.get("content") or "")[:120].replace("\n", " "), row.get("tool_calls") and "TOOL", row.get("error", ""))
