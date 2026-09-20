"""M-20 T-02 — quanto costa al motore principale avere accanto un secondo llama-server.

La domanda non e' «quanto va il motore piccolo»: e' **quanto rallenta il motore principale**
quando un secondo llama-server sta sulla stessa iGPU. Su questa macchina la memoria e' una sola
(UMA) e M-08 T-02 ha misurato che il decode del 35B e' un problema di banda; M-08 T-11 ha misurato
che la NPU, che e' un altro dispositivo, costava il 40%. Qui il secondo carico sta sulla stessa
GPU, quindi il costo puo' essere piu' alto (si serializza sui comandi) o piu' basso (non c'e'
contesa di potenza fra due unita' diverse). Non si deduce: si misura.

Con la precedenza decisa dall'operatore il 20-09 — i ruoli girano anche di giorno ma il coding ha
la precedenza — **la condizione che decide e' la B**, cioe' il motore compagno caricato e fermo:
e' li' che la macchina passera' quasi tutto il tempo. La C dice quanto costa la finestra di 30 s
con cui Aethera dichiara «in uso», e i pezzi di lavoro gia' iniziati.

Quattro condizioni di fila, nella stessa sessione e senza riavviare il motore principale:

  A1  solo          nessun secondo motore
  B   fermo         secondo motore caricato, che non riceve richieste
  C   attivo        secondo motore che genera in continuo
  A2  solo          ripetizione di A, come sentinella: se A1 e A2 non coincidono, la misura
                    ha preso una deriva e non un effetto

Il motore principale e' acceso a parte da `m08_hold` col profilo unico, cosi' l'avvio lascia il
suo manifest. Il motore compagno lo avvia e lo spegne questo script.

Uso: python T-02-secondo-motore.py [--reps 5] [--solo-condizione B]
Righe grezze in <radice>/m20/T-02.jsonl (principale) e T-02-compagno.jsonl (compagno).
"""

import argparse
import json
import os
import subprocess
import threading
import time
import urllib.error
import urllib.request
from datetime import datetime
from pathlib import Path

HERE = Path(__file__).parent
M08 = HERE.parent / "m08"
OUT = Path(os.environ.get("AETHERA_RADICE", "C:/AetheraData")) / "m20"
PRINCIPALE = "http://127.0.0.1:8080"
COMPAGNO = "http://127.0.0.1:8081"

BUILD = Path(os.environ.get("AETHERA_BUILD", "C:/Nonio/llama-b10809-vulkan")) / "llama-server.exe"
PESI = Path(os.environ.get("AETHERA_PESI", "C:/Git/minis-config/models"))
# Qwen3-8B Q4_K_M: e' gia' sul disco ed e' gia' caratterizzato su questa macchina da M-08 T-02
# (decode 16,01 tok/s, prefill 303 dopo l'aggiornamento dei driver). Usare un modello gia'
# misurato toglie un'incognita: se il principale rallenta, non e' perche' il compagno e' ignoto.
# E' anche un limite superiore onesto per un motore di ruoli, che sarebbe un 4B.
COMPAGNO_PESI = PESI / "Qwen3-8B-Q4_K_M.gguf"
COMPAGNO_CTX = 8192

# Gli stessi due carichi e lo stesso campionamento di M-08 T-05 e T-11: cosi' i numeri di oggi si
# confrontano con quelli del 16-09 senza tradurre niente.
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

TESTO_COMPAGNO = (M08 / "prompt-7k.txt").read_text(encoding="utf-8")[:3000]

CONDIZIONI = [
    ("A1-solo", False, False),
    ("B-fermo", True, False),
    ("C-attivo", True, True),
    ("A2-solo", False, False),
]


def post(url, body, timeout=1800):
    req = urllib.request.Request(url, json.dumps(body).encode(), {"Content-Type": "application/json"})
    with urllib.request.urlopen(req, timeout=timeout) as r:
        return json.loads(r.read())


def vram_gib():
    """Dedicata in uso dal contatore di Windows. --list-devices non serve: M-20 T-01 ha misurato
    che riporta lo stesso 'free' a motore spento e a motore carico (e' il budget dell'heap)."""
    ps = (r"$c=(Get-Counter -ListSet 'GPU Adapter Memory').PathsWithInstances|"
          r"Where-Object{$_ -like '*Dedicated Usage*'};"
          r"$t=0;(Get-Counter -Counter $c -EA SilentlyContinue).CounterSamples|"
          r"ForEach-Object{$t+=$_.CookedValue};[math]::Round($t/1GB,2)")
    try:
        out = subprocess.run(["powershell", "-NoProfile", "-Command", ps],
                             capture_output=True, text=True, timeout=60)
        return float(out.stdout.strip().replace(",", "."))
    except Exception:
        return None


def avvia_compagno():
    cmd = [str(BUILD), "--model", str(COMPAGNO_PESI), "--host", "127.0.0.1", "--port", "8081",
           "--ctx-size", str(COMPAGNO_CTX), "--parallel", "1", "--n-gpu-layers", "999",
           "--flash-attn", "on", "--alias", "compagno", "--metrics", "--jinja"]
    print("  compagno:", " ".join(cmd))
    p = subprocess.Popen(cmd, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    t0 = time.perf_counter()
    while time.perf_counter() - t0 < 300:
        try:
            with urllib.request.urlopen(f"{COMPAGNO}/health", timeout=3) as r:
                if r.status == 200:
                    print(f"  compagno pronto in {time.perf_counter()-t0:.1f} s")
                    return p
        except (urllib.error.URLError, OSError):
            pass
        time.sleep(2)
    p.terminate()
    raise SystemExit("il motore compagno non e' arrivato a pronto in 300 s")


def ferma_compagno(p):
    if p is None:
        return
    p.terminate()
    try:
        p.wait(timeout=60)
    except subprocess.TimeoutExpired:
        p.kill()
    time.sleep(5)  # la VRAM torna libera con un attimo di ritardo


def carico_compagno(stop, righe):
    i = 0
    while not stop.is_set():
        t0 = time.perf_counter()
        try:
            v = post(f"{COMPAGNO}/v1/chat/completions", {
                "model": "compagno", "max_tokens": 256, "temperature": 0.7, "stream": False,
                "messages": [{"role": "user", "content":
                              f"({i}) Spiega in dieci righe che cosa fa questo codice:\n{TESTO_COMPAGNO[:1200]}"}]},
                timeout=300)
            riga = {"usage": v.get("usage")}
        except Exception as e:  # un compagno che fallisce va scritto, non nascosto
            riga = {"error": str(e)}
            time.sleep(2)
        riga["wall_ms"] = round((time.perf_counter() - t0) * 1000)
        riga["at"] = datetime.now().isoformat()
        righe.append(riga)
        i += 1


def una_condizione(nome, reps, out, righe_compagno_tot):
    stop, righe_compagno = threading.Event(), []
    th = None
    attivo = nome.startswith("C")
    if attivo:
        th = threading.Thread(target=carico_compagno, args=(stop, righe_compagno), daemon=True)
        th.start()
        time.sleep(10)  # il compagno a regime prima della prima misura

    vram = vram_gib()
    print(f"\n--- {nome}   VRAM dedicata in uso: {vram} GiB")
    for name, file, instruction in WORKLOADS:
        body = (M08 / file).read_text(encoding="utf-8")
        for rep in range(reps + 1):  # il primo e' riscaldamento e non conta
            nonce = f"[M20-T02 {nome} {name} {rep} {time.time_ns()}]\n"
            content = f"{nonce}{body}\n\n{instruction}\n"
            prompt = post(f"{PRINCIPALE}/apply-template", {
                "messages": [{"role": "user", "content": content}],
                "chat_template_kwargs": {"enable_thinking": False}})["prompt"]
            t0 = time.perf_counter()
            v = post(f"{PRINCIPALE}/completion", dict(SAMPLING, prompt=prompt))
            t = v["timings"]
            riga = {"measure": "M20-T02", "variant": nome, "workload": name, "rep": rep,
                    "warmup": rep == 0, "at": datetime.now().isoformat(), "timings": t,
                    "wall_ms": round((time.perf_counter() - t0) * 1000),
                    "vram_gib_inizio": vram,
                    "compagno_richieste_finora": len(righe_compagno)}
            out.write(json.dumps(riga) + "\n")
            out.flush()
            print(f"{nome:10} {name:14} rep {rep}  prefill {t['prompt_per_second']:7.1f}  "
                  f"decode {t['predicted_per_second']:6.2f}  cache {t.get('cache_n')}  "
                  f"compagno {len(righe_compagno)}")
    stop.set()
    if th:
        th.join(timeout=300)
    for r in righe_compagno:
        righe_compagno_tot.append(dict(r, condizione=nome))


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--reps", type=int, default=5)
    ap.add_argument("--solo-condizione", default=None, help="A1-solo | B-fermo | C-attivo | A2-solo")
    a = ap.parse_args()

    OUT.mkdir(parents=True, exist_ok=True)
    if not COMPAGNO_PESI.exists():
        raise SystemExit(f"pesi del compagno non trovati: {COMPAGNO_PESI}")
    # il principale deve essere gia' acceso da m08_hold
    props = json.loads(urllib.request.urlopen(f"{PRINCIPALE}/props", timeout=30).read())
    ctx = props.get("default_generation_settings", {}).get("n_ctx") or props.get("n_ctx")
    print(f"motore principale: ctx servito {ctx}")

    da_fare = [c for c in CONDIZIONI if a.solo_condizione in (None, c[0])]
    righe_compagno_tot = []
    compagno = None
    out = open(OUT / "T-02.jsonl", "a", encoding="utf-8")
    t_inizio = time.perf_counter()
    try:
        for nome, serve_compagno, _ in da_fare:
            if serve_compagno and compagno is None:
                print(f"\n=== {nome}: accendo il motore compagno")
                compagno = avvia_compagno()
            elif not serve_compagno and compagno is not None:
                print(f"\n=== {nome}: spengo il motore compagno")
                ferma_compagno(compagno)
                compagno = None
            una_condizione(nome, a.reps, out, righe_compagno_tot)
    finally:
        ferma_compagno(compagno)
        out.close()
        with open(OUT / "T-02-compagno.jsonl", "a", encoding="utf-8") as f:
            for r in righe_compagno_tot:
                f.write(json.dumps(r) + "\n")
    errori = sum("error" in r for r in righe_compagno_tot)
    print(f"\nFinito in {(time.perf_counter()-t_inizio)/60:.1f} minuti. "
          f"Compagno: {len(righe_compagno_tot)} richieste, {errori} errori.")


if __name__ == "__main__":
    main()
