"""Il metro del prodotto, letto da una sessione vera.

Dal 20-09-2026 il riferimento non e' un banco rifatto ogni volta, ma le sessioni che l'app
registra da sola: ogni avvio lascia un `runs/<id>/` con manifest.toml, server.log e
telemetry.jsonl. Questo script legge una di quelle cartelle e stampa gli stessi numeri della
baseline scritta nel profilo, cosi' un peggioramento si vede senza rimisurare niente.

Uso:  python baseline.py <runs/r-...>            una sessione
      python baseline.py --ultime 5              le ultime N della radice di Aethera

Le richieste a riuso zero sono quelle che rifanno il prefill da capo. Una all'inizio di ogni
conversazione nuova e' normale; di piu' significa che il client fa divergere il prompt, e il
riuso del prefisso e' tutto-o-niente (misura del 20-09).
"""
from __future__ import annotations

import json
import os
import statistics as st
import sys
import tomllib
from pathlib import Path

# Sotto questa soglia la richiesta non aggiunge quasi niente al prompt: e' il costo fisso.
POCHI_TOKEN_NUOVI = 200
# Sopra questa taglia una richiesta senza riuso e' un prefill rifatto davvero, non un saluto.
PROMPT_SIGNIFICATIVO = 1000
# `cache_n` = 1 e' il solo BOS: vale zero riuso.
RIUSO_NULLO = 1


def leggi(run: Path) -> dict | None:
    tel = run / "telemetry.jsonl"
    if not tel.exists():
        return None
    rec = [json.loads(l) for l in tel.read_text(encoding="utf-8").splitlines() if l.startswith("{")]
    if not rec:
        return None
    man = {}
    if (run / "manifest.toml").exists():
        man = tomllib.loads((run / "manifest.toml").read_text(encoding="utf-8"))
    # `prompt_n` sono i token ELABORATI, non tutto il prompt: quelli presi dalla cache stanno in
    # `cache_n` e non sono compresi. Il prompt intero e' la somma dei due.
    elaborati = sum(r.get("prompt_n", 0) for r in rec)
    cache = sum(r.get("cache_n", 0) for r in rec)
    prompt = elaborati + cache
    fissi = [r["prompt_ms"] for r in rec
             if r.get("prompt_n", 0) - r.get("cache_n", 0) < POCHI_TOKEN_NUOVI and "prompt_ms" in r]
    zero = [r for r in rec
            if r.get("cache_n", 0) <= RIUSO_NULLO and r.get("prompt_n", 0) > PROMPT_SIGNIFICATIVO]
    dec = [r["decode_tps"] for r in rec if r.get("decode_tps")]
    draft = sum(r.get("draft_n", 0) for r in rec)
    acc = sum(r.get("draft_accepted", 0) for r in rec)
    return {
        "id": man.get("run", {}).get("id", run.name),
        "profilo": man.get("run", {}).get("profile", "?"),
        "ctx": man.get("server", {}).get("ctx_served"),
        "richieste": len(rec),
        "riuso": 100 * cache / prompt if prompt else 0.0,
        "prefill_per_richiesta": sum(r.get("prompt_ms", 0) for r in rec) / 1000 / len(rec),
        "costo_fisso": st.median(fissi) / 1000 if fissi else None,
        "costo_fisso_n": len(fissi),
        "senza_riuso": len(zero),
        "decode": st.median(dec) if dec else None,
        "accettazione": 100 * acc / draft if draft else None,
    }


def stampa(d: dict) -> None:
    cf = f"{d['costo_fisso']:.2f}s (n={d['costo_fisso_n']})" if d["costo_fisso"] is not None else "—"
    dec = f"{d['decode']:.1f}" if d["decode"] is not None else "—"
    acc = f"{d['accettazione']:.1f}%" if d["accettazione"] is not None else "—"
    print(f"{d['id']:22} {d['profilo']:32} ctx {d['ctx'] or '?':>7}")
    print(f"  richieste {d['richieste']:<5} riuso {d['riuso']:5.1f}%   prefill/richiesta "
          f"{d['prefill_per_richiesta']:.2f}s   costo fisso {cf}")
    print(f"  senza riuso {d['senza_riuso']:<4} decode {dec} tok/s   accettazione della bozza {acc}")


def main() -> int:
    args = sys.argv[1:]
    if args and args[0] == "--ultime":
        radice = Path(os.environ.get("AETHERA_RADICE", "C:/AetheraData")) / "runs"
        quante = int(args[1]) if len(args) > 1 else 5
        run = sorted([p for p in radice.iterdir() if p.is_dir()], reverse=True)[:quante]
    elif args:
        run = [Path(a) for a in args]
    else:
        print(__doc__)
        return 2
    print("BASELINE del 20-09-2026 (batteria di M-15, 16 compiti, ctx 32768): 16/16 · 42,8 compiti/ora")
    print("  riuso 90,3% · prefill 1,61 s/richiesta · una sola richiesta senza riuso per compito\n")
    for r in run:
        d = leggi(r)
        if d is None:
            print(f"{r.name}: nessuna telemetria")
            continue
        stampa(d)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
