"""M-08 T-11 — quanto rallenta il 35B su Vulkan quando la NPU lavora insieme a lui.

Il 35B è acceso da `m08_hold` (profilo G1); FastFlowLM serve Qwen3.5-4B con EmbeddingGemma sulla
porta 52625. Per ogni condizione si mandano al 35B gli stessi due carichi di T-05, a cache fredda
(un nonce in testa, come fa m08_bench), mentre un filo a parte tiene occupata la NPU:

  A  npu-ferma      nessun carico sulla NPU (i modelli restano caricati)
  B  npu-embedding  richieste /v1/embeddings in continuo
  C  npu-genera     richieste /v1/chat/completions in continuo al 4B

Uso: python T-11-npu.py <condizione> [--reps 5] [--engine http://127.0.0.1:8080]
Le righe finiscono in C:/AetheraData/m08/T-11.jsonl (35B) e T-11-npu.jsonl (carico NPU).
"""

import argparse
import json
import threading
import time
import urllib.request
from datetime import datetime
from pathlib import Path

HERE = Path(__file__).parent
OUT = Path("C:/AetheraData/m08")
FLM = "http://127.0.0.1:52625"

# Gli stessi carichi e lo stesso campionamento di T-05-mtp.toml.
WORKLOADS = [
    ("codice-7k", "prompt-7k.txt",
     "Riscrivi in Rust la funzione `build_args` del file cmdline.rs in modo che gli argomenti vengano "
     "composti da una tabella dichiarativa invece che da chiamate ripetute, mantenendo esattamente lo "
     "stesso ordine e lo stesso comportamento. Rispondi con il solo codice."),
    ("riassunto-21k", "prompt-21k.txt",
     "Riassumi in italiano, in dodici righe al massimo, che cosa fa questo programma nel suo insieme "
     "e dove sono i punti in cui una modifica sbagliata farebbe danni."),
]
SAMPLING = dict(n_predict=384, temperature=0.7, top_p=0.8, top_k=20, min_p=0.0,
                presence_penalty=1.5, seed=1234, cache_prompt=True, timings_per_token=False)

NPU_TEXT = (HERE / "prompt-7k.txt").read_text(encoding="utf-8")[:3000]


def post(url, body, timeout=900):
    req = urllib.request.Request(url, json.dumps(body).encode(), {"Content-Type": "application/json"})
    with urllib.request.urlopen(req, timeout=timeout) as r:
        return json.loads(r.read())


def npu_loop(kind, stop, rows):
    i = 0
    while not stop.is_set():
        t0 = time.perf_counter()
        try:
            if kind == "embedding":
                v = post(f"{FLM}/v1/embeddings", {"model": "embed-gemma:300m", "input": NPU_TEXT[i % 500:][:1500]})
                row = {"kind": kind, "n": len(v.get("data", [])), "dim": len(v["data"][0]["embedding"]) if v.get("data") else 0}
            else:
                v = post(f"{FLM}/v1/chat/completions", {
                    "model": "qwen3.5:4b", "max_tokens": 256, "temperature": 0.7, "stream": False,
                    "messages": [{"role": "user", "content": f"({i}) Spiega in dieci righe che cosa fa questo codice:\n{NPU_TEXT[:1200]}"}]})
                row = {"kind": kind, "usage": v.get("usage"), "extra": {k: v[k] for k in v if k not in ("choices", "usage")}}
        except Exception as e:  # la NPU che fallisce va scritta, non nascosta
            row = {"kind": kind, "error": str(e)}
            time.sleep(2)
        row["wall_ms"] = round((time.perf_counter() - t0) * 1000)
        row["at"] = datetime.now().isoformat()
        rows.append(row)
        i += 1


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("condition", choices=["npu-ferma", "npu-embedding", "npu-genera"])
    ap.add_argument("--reps", type=int, default=5)
    ap.add_argument("--engine", default="http://127.0.0.1:8080")
    a = ap.parse_args()

    stop, npu_rows = threading.Event(), []
    th = None
    if a.condition != "npu-ferma":
        th = threading.Thread(target=npu_loop, args=(a.condition.split("-")[1], stop, npu_rows), daemon=True)
        th.start()
        time.sleep(5)  # la NPU a regime prima della prima misura

    out = open(OUT / "T-11.jsonl", "a", encoding="utf-8")
    for name, file, instruction in WORKLOADS:
        body = (HERE / file).read_text(encoding="utf-8")
        for rep in range(a.reps + 1):  # il primo è riscaldamento
            nonce = f"[T-11 {a.condition} {name} {rep} {time.time_ns()}]\n"
            content = f"{nonce}{body}\n\n{instruction}\n"
            prompt = post(f"{a.engine}/apply-template", {
                "messages": [{"role": "user", "content": content}],
                "chat_template_kwargs": {"enable_thinking": False}})["prompt"]
            t0 = time.perf_counter()
            v = post(f"{a.engine}/completion", dict(SAMPLING, prompt=prompt))
            t = v["timings"]
            row = {"measure": "T-11", "variant": a.condition, "workload": name, "rep": rep,
                   "warmup": rep == 0, "at": datetime.now().isoformat(), "timings": t,
                   "wall_ms": round((time.perf_counter() - t0) * 1000), "npu_requests_so_far": len(npu_rows)}
            out.write(json.dumps(row) + "\n")
            out.flush()
            print(f"{a.condition:14} {name:14} rep {rep}  prefill {t['prompt_per_second']:7.1f}  "
                  f"decode {t['predicted_per_second']:6.2f}  cache {t.get('cache_n')}  npu {len(npu_rows)}")
    stop.set()
    if th:
        th.join(timeout=300)
    with open(OUT / "T-11-npu.jsonl", "a", encoding="utf-8") as f:
        for r in npu_rows:
            f.write(json.dumps(dict(r, condition=a.condition)) + "\n")
    print(f"NPU: {len(npu_rows)} richieste, errori {sum('error' in r for r in npu_rows)}")


if __name__ == "__main__":
    main()
