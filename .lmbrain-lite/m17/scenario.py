"""M-17: il costo fisso per richiesta, misurato fuori dalla batteria.

Una conversazione che cresce in coda, come quella di un agente: un primo messaggio con del codice
(il prefisso, `--prefisso` caratteri), poi turni in cui il client rimanda tutto e aggiunge la
risposta breve del modello più un messaggio nuovo di dimensione variabile (da ~20 a ~1.200 token).
Per ogni richiesta si registra quello che dichiara il motore (`timings`: prompt_n, prompt_ms,
cache_n, predicted_n, predicted_ms) e il tempo a muro visto dal client; alla fine la regressione di
prompt_ms su prompt_n dà l'intercetta (il costo fisso) e la pendenza (il costo marginale).

Il motore si accende tramite Aethera (`m15_hold`), una variante per avvio: le leve passano da
`--extra`, `--ctx`, `--cache-ram`, `--ctx-checkpoints`. Con `--gia-acceso` si usa un motore che c'è già.

Uso:
  python scenario.py --profilo qwen3-coder-next.q4_k_m.vulkan --nome g3-standard --misura T-01
  python scenario.py ... --endpoint completion        # /completion con il testo grezzo
  python scenario.py ... --extra "--ctx-checkpoints 0" --nome g3-ckpt0

Uscite in <radice>/m17/: <misura>.jsonl (una riga per richiesta), <misura>-<nome>.pronto.json,
hold-<nome>.log; il log del motore resta in <radice>/runs/<avvio>/server.log.
"""

import argparse
import datetime as dt
import json
import os
import subprocess
import sys
import time
import urllib.request
from pathlib import Path

QUI = Path(__file__).resolve().parent
sys.path.insert(0, str(QUI.parent / "batteria"))
import batteria as b  # noqa: E402
import esegui as e  # noqa: E402

# Caratteri nuovi per turno (~3,3 caratteri per token): tanti turni piccoli, come con un agente, e
# qualcuno grande per dare leva alla pendenza. L'ordine alterna per non confondere dimensione e
# posizione nella conversazione.
PASSI = [70, 70, 140, 70, 300, 100, 70, 600, 70, 160, 1200, 70, 100, 2400, 70, 140, 4000, 70, 100, 70]
SISTEMA = "You are a coding assistant. Answer in one short sentence."
DOMANDA = "\n\nIn one short sentence: what does the code above do?"
SEGUITO = "\n\nHere is more of the same program. In one short sentence: what does this part add?\n\n"


def http_json(url: str, corpo: dict, timeout: float = 3600) -> tuple[dict, float]:
    dati = json.dumps(corpo).encode("utf-8")
    req = urllib.request.Request(url, data=dati, headers={"Content-Type": "application/json"})
    t0 = time.perf_counter()
    with urllib.request.urlopen(req, timeout=timeout) as r:
        testo = r.read()
    return json.loads(testo), (time.perf_counter() - t0) * 1000


def regressione(punti: list[tuple[float, float]]) -> dict | None:
    n = len(punti)
    if n < 3:
        return None
    mx = sum(x for x, _ in punti) / n
    my = sum(y for _, y in punti) / n
    sxx = sum((x - mx) ** 2 for x, _ in punti)
    if sxx == 0:
        return None
    pend = sum((x - mx) * (y - my) for x, y in punti) / sxx
    inter = my - pend * mx
    res = [y - (inter + pend * x) for x, y in punti]
    sd = (sum(r * r for r in res) / max(n - 2, 1)) ** 0.5
    return {"n": n, "intercetta_ms": inter, "ms_per_token": pend, "sd_residui_ms": sd}


def conversazione(base: str, args, sorgente: str, riga: dict, fh) -> list[dict]:
    """Un giro: prefisso, poi i turni di PASSI. Ritorna le righe scritte."""
    alias = riga["alias"]
    prefisso = sorgente[: args.prefisso]
    if args.divergenza:
        meta = args.prefisso // 2
        prefisso = prefisso[:meta] + f"/* turno {riga['giro']} */" + prefisso[meta:]
    cursore = args.prefisso
    righe = []
    messaggi = [{"role": "system", "content": SISTEMA}, {"role": "user", "content": prefisso + DOMANDA}]
    grezzo = SISTEMA + "\n\n" + prefisso + DOMANDA + "\n\nAnswer: "
    for turno in range(args.turni + 1):
        if args.endpoint == "chat":
            corpo = {"model": alias, "messages": messaggi, "max_tokens": args.n_predict, "temperature": 0.0,
                     "seed": 1234, "stream": False, "cache_prompt": True}
            if args.senza_thinking:
                corpo["chat_template_kwargs"] = {"enable_thinking": False}
            risposta, muro = http_json(base + "/v1/chat/completions", corpo)
            testo = (risposta.get("choices") or [{}])[0].get("message", {}).get("content") or ""
        else:
            corpo = {"prompt": grezzo, "n_predict": args.n_predict, "temperature": 0.0, "seed": 1234,
                     "stream": False, "cache_prompt": True}
            risposta, muro = http_json(base + "/completion", corpo)
            testo = risposta.get("content") or ""
        t = risposta.get("timings") or {}
        r = {**riga, "turno": turno, "caratteri_nuovi": (args.prefisso if turno == 0 else PASSI[(turno - 1) % len(PASSI)]),
             "prompt_n": t.get("prompt_n"), "prompt_ms": t.get("prompt_ms"), "cache_n": t.get("cache_n"),
             "predicted_n": t.get("predicted_n"), "predicted_ms": t.get("predicted_ms"),
             "muro_ms": round(muro, 1),
             "fuori_dai_timings_ms": round(muro - (t.get("prompt_ms") or 0) - (t.get("predicted_ms") or 0), 1),
             "ora": dt.datetime.now().isoformat(timespec="milliseconds")}
        fh.write(json.dumps(r, ensure_ascii=False) + "\n")
        fh.flush()
        righe.append(r)
        print(f"  giro {riga['giro']} turno {turno:2d}: nuovi {r['prompt_n']} · cache {r['cache_n']} · "
              f"prompt {r['prompt_ms']:.0f} ms · muro {muro:.0f} ms", flush=True)
        if args.divergenza:
            # a ogni turno la stessa riga a meta' prefisso cambia: il prefisso in cache non vale piu'
            meta = args.prefisso // 2
            base = sorgente[: args.prefisso]
            nuovo_pref = base[:meta] + f"/* turno {turno + 1} */" + base[meta:]
            if args.endpoint == "chat":
                messaggi[1]["content"] = nuovo_pref + DOMANDA
            else:
                grezzo = SISTEMA + "\n\n" + nuovo_pref + DOMANDA + "\n\nAnswer: "
        passo = PASSI[turno % len(PASSI)]
        nuovo = sorgente[cursore: cursore + passo]
        cursore += passo
        if args.endpoint == "chat":
            messaggi.append({"role": "assistant", "content": testo})
            messaggi.append({"role": "user", "content": SEGUITO + nuovo})
        else:
            grezzo += testo + SEGUITO + nuovo + "\n\nAnswer: "
    return righe


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--profilo", required=True)
    ap.add_argument("--nome", required=True, help="etichetta della variante")
    ap.add_argument("--misura", default="T-01")
    ap.add_argument("--endpoint", choices=("chat", "completion"), default="chat")
    ap.add_argument("--prefisso", type=int, default=13000, help="caratteri del primo messaggio (~4k token)")
    ap.add_argument("--sorgente", default=str(QUI.parent / "m10" / "prompt-60k.txt"))
    ap.add_argument("--turni", type=int, default=20)
    ap.add_argument("--giri", type=int, default=3)
    ap.add_argument("--n-predict", type=int, default=24)
    ap.add_argument("--senza-thinking", action="store_true")
    ap.add_argument("--divergenza", action="store_true",
                    help="riscrive una parola in mezzo al prefisso a ogni turno: simula un harness che "
                         "ritocca la conversazione, il caso peggiore per i modelli ibridi")
    ap.add_argument("--ctx", type=int)
    ap.add_argument("--extra")
    ap.add_argument("--cache-ram", type=int)
    ap.add_argument("--ctx-checkpoints", type=int)
    ap.add_argument("--gia-acceso", help="pronto.json di un m15_hold già acceso")
    args = ap.parse_args()

    radice = b.radice()
    out = radice / "m17"
    out.mkdir(parents=True, exist_ok=True)
    sorgente = Path(args.sorgente).read_text(encoding="utf-8")
    pronto_f = Path(args.gia_acceso) if args.gia_acceso else out / f"{args.misura}-{args.nome}.pronto.json"
    proc = None
    if not args.gia_acceso:
        while True:
            o = e.ostacoli(radice, e.RAM_MINIMA_GIB)
            if not o:
                break
            print("aspetto: " + "; ".join(o), flush=True)
            time.sleep(60)
        pronto_f.unlink(missing_ok=True)
        (radice / "stop-m15").unlink(missing_ok=True)
        exe = os.environ.get("AETHERA_M15_HOLD") or str(
            QUI.parents[1] / "src-tauri" / "target" / "release" / "examples" / "m15_hold.exe")
        cmd = [exe, str(radice), args.profilo, "--pronto", str(pronto_f)]
        for flag, v in (("--ctx", args.ctx), ("--extra", args.extra), ("--cache-ram", args.cache_ram),
                        ("--ctx-checkpoints", args.ctx_checkpoints)):
            if v is not None:
                cmd += [flag, str(v)]
        log_hold = open(out / f"hold-{args.nome}.log", "a", encoding="utf-8")
        proc = subprocess.Popen(cmd, stdout=log_hold, stderr=subprocess.STDOUT)
        t0 = time.monotonic()
        while not pronto_f.is_file():
            if proc.poll() is not None:
                print(f"m15_hold è uscito con {proc.returncode} (vedi hold-{args.nome}.log)")
                return 1
            if time.monotonic() - t0 > 1800:
                proc.kill()
                return 1
            time.sleep(1)
    pronto = json.loads(pronto_f.read_text(encoding="utf-8"))
    base = pronto["base_url"].rstrip("/")
    print(f"=== {args.misura} · {args.nome} · {pronto['run_id']} · RAM libera {e.ram_libera_gib():.1f} GiB\n"
          f"    {pronto['command_line']}", flush=True)

    tutte = []
    try:
        with open(out / f"{args.misura}.jsonl", "a", encoding="utf-8") as fh:
            for giro in range(args.giri):
                riga = {"misura": args.misura, "variante": args.nome, "profilo": args.profilo, "endpoint": args.endpoint,
                        "run_id": pronto["run_id"], "build": pronto.get("build"), "alias": pronto["alias"], "giro": giro,
                        "prefisso_caratteri": args.prefisso}
                # Ogni giro parte da un prefisso diverso, così il primo turno è sempre a freddo.
                spostato = sorgente[giro * 997:]
                tutte += conversazione(base, args, spostato, riga, fh)
    finally:
        if proc is not None:
            (radice / "stop-m15").write_text("", encoding="utf-8")
            try:
                proc.wait(timeout=180)
            except subprocess.TimeoutExpired:
                proc.kill()

    caldi = [(r["prompt_n"], r["prompt_ms"]) for r in tutte if r["turno"] > 0 and r["prompt_n"] and r["prompt_ms"]]
    piccoli = [p for p in caldi if p[0] <= 100]
    reg = regressione(caldi)
    esito = {"misura": args.misura, "variante": args.nome, "run_id": pronto["run_id"], "regressione": reg,
             "richieste_sotto_100_token": len(piccoli),
             "prompt_ms_mediano_sotto_100": sorted(p[1] for p in piccoli)[len(piccoli) // 2] if piccoli else None,
             "prompt_n_mediano_sotto_100": sorted(p[0] for p in piccoli)[len(piccoli) // 2] if piccoli else None,
             "fuori_dai_timings_ms_mediano": sorted(r["fuori_dai_timings_ms"] for r in tutte)[len(tutte) // 2] if tutte else None}
    with open(out / f"{args.misura}-esiti.jsonl", "a", encoding="utf-8") as fh:
        fh.write(json.dumps(esito, ensure_ascii=False) + "\n")
    print(json.dumps(esito, ensure_ascii=False, indent=1))
    return 0


if __name__ == "__main__":
    sys.exit(main())
