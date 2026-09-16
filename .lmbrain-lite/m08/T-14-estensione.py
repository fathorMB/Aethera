"""M-08 T-14 — la differenza fra un turno che estende e un turno che riscrive.

T-13 ha misurato turni in cui il prompt nuovo NON conteneva la risposta precedente del modello, e
ha trovato un costo fisso di un ubatch per turno. T-10 ha poi mostrato che Nonio fa l'opposto:
rimanda indietro tutto, risposta del modello compresa, e il motore riusa il 100% del prefisso.

Quindi il costo fisso non e' una proprieta' del motore: e' il prezzo di un client che ricostruisce
il prompt senza la risposta che il modello ha appena dato. Questa misura mette i due modi uno
accanto all'altro sullo stesso motore acceso, con lo stesso prompt congelato.

    python T-14-estensione.py http://127.0.0.1:8080 <radice>/m08/T-14.jsonl
"""
import json
import os
import sys
import time
import urllib.request

HERE = os.path.dirname(os.path.abspath(__file__))
TURNS = 4
N_PREDICT = 128


def post(url, path, body, timeout=1800):
    req = urllib.request.Request(url + path, data=json.dumps(body).encode("utf-8"),
                                 headers={"Content-Type": "application/json"})
    with urllib.request.urlopen(req, timeout=timeout) as r:
        return json.load(r)


def templated(url, messages):
    return post(url, "/apply-template",
                {"messages": messages, "chat_template_kwargs": {"enable_thinking": False}})["prompt"]


def run(url, mode, body, out):
    """mode='estende': il prompt nuovo contiene la risposta precedente (come fa un client serio).
       mode='riscrive': il prompt nuovo riparte dalla sola domanda (come faceva il banco di T-13)."""
    nonce = "// banco M-08 T-14 · %s · %d\n" % (mode, time.time_ns())
    base = nonce + body + "\n\nLeggi il codice qui sopra. "
    messages = []
    rows = []
    for turn in range(TURNS):
        question = "Domanda %d: nomina UNA funzione a rischio di regressione e spiega perche' in due righe." % turn
        if mode == "estende":
            messages = messages + [{"role": "user", "content": (base + question) if turn == 0 else question}]
        else:
            messages = [{"role": "user", "content": base + question}]
        prompt = templated(url, messages)
        t0 = time.time()
        v = post(url, "/completion", {"prompt": prompt, "n_predict": N_PREDICT, "cache_prompt": True,
                                      "temperature": 0.0, "seed": 1234})
        wall = int((time.time() - t0) * 1000)
        t = v["timings"]
        row = {"mode": mode, "turn": turn, "prompt_n": t["prompt_n"], "cache_n": t.get("cache_n"),
               "prompt_ms": t["prompt_ms"], "predicted_ms": t["predicted_ms"], "wall_ms": wall}
        rows.append(row)
        out.write(json.dumps(row) + "\n")
        out.flush()
        print("%-9s turno %d · elaborati %5d · riusati %5d · prefill %6.1f s · turno %5.1f s"
              % (mode, turn, t["prompt_n"], t.get("cache_n") or 0, t["prompt_ms"] / 1000, wall / 1000))
        if mode == "estende":
            messages = messages + [{"role": "assistant", "content": v["content"]}]
    return rows


def main():
    url = sys.argv[1] if len(sys.argv) > 1 else "http://127.0.0.1:8080"
    path = sys.argv[2] if len(sys.argv) > 2 else "T-14.jsonl"
    body = open(os.path.join(HERE, "prompt-7k.txt"), encoding="utf-8").read()
    with open(path, "a", encoding="utf-8") as out:
        for mode in ("estende", "riscrive"):
            run(url, mode, body, out)


if __name__ == "__main__":
    main()
