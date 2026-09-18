"""M-17 T-03: dove vanno i millisecondi dentro prompt_ms, letti dal log del motore a `-lv 5`.

Il cronometro di prompt_ms parte alla riga «new prompt» e finisce al primo token campionato. In mezzo
il log a verbosità 5 marca le fasi: «cached n_tokens = N, memory_seq_rm» (confronto del prefisso
fatto, inizio di un batch), «main/do_checkpoint = yes/no», «created context checkpoint … size = …
MiB», «n_batch (effective)» (inizio di un decode), «init sampler, took», «prompt processing … t = …».
Questo script somma, per ogni richiesta, quanto è costata ogni fase, e stampa le mediane.

Uso:  python fasi.py <server.log> [<server.log> ...]
      python fasi.py --pronto <radice>/m17/T-03-g3-lv5.pronto.json
"""

import argparse
import json
import re
import statistics as st
import sys
from pathlib import Path

# 0.38.537.614 I slot ... : minuti.secondi.millisecondi.microsecondi dall'avvio del motore
ORA = re.compile(r"^(\d+)\.(\d+)\.(\d+)\.(\d+) ")
NUOVO = re.compile(r"new prompt, n_ctx_slot = \d+, n_keep = \d+, task\.n_tokens = (\d+)")
CACHED = re.compile(r"cached n_tokens = (\d+), memory_seq_rm")
CHECKPOINT = re.compile(r"created context checkpoint (\d+) of \d+ \(.*n_tokens = (\d+), size = ([\d.]+) MiB\)")
DECIDE = re.compile(r"main/do_checkpoint = (yes|no)")
BATCH = re.compile(r"n_batch \(effective\) = (\d+)")
SAMPLER = re.compile(r"init sampler, took ([\d.]+) ms")
PROC = re.compile(r"prompt processing, n_tokens =\s+(\d+), progress = [\d.]+, t =\s+([\d.]+) s")
PRIMO = re.compile(r"slot decode token, id=")
RESTORE = re.compile(r"restored context checkpoint|forcing full prompt re-processing")


def ms(riga: str) -> float | None:
    m = ORA.match(riga)
    if not m:
        return None
    mi, s, milli, micro = (int(x) for x in m.groups())
    return (mi * 60 + s) * 1000 + milli + micro / 1000


def leggi(percorso: Path) -> list[dict]:
    """Una voce per richiesta, con i millisecondi spesi per fase."""
    richieste, cur, ultimo = [], None, None
    for riga in percorso.read_text(encoding="utf-8", errors="replace").splitlines():
        t = ms(riga)
        if t is None:
            continue
        if m := NUOVO.search(riga):
            if cur:
                richieste.append(cur)
            cur = {"t0": t, "token_totali": int(m.group(1)), "checkpoint": 0, "checkpoint_ms": 0.0,
                   "checkpoint_mib": 0.0, "decode": 0, "decode_ms": 0.0, "prefisso_ms": 0.0,
                   "coda_ms": 0.0, "sampler_ms": 0.0, "ripristini": 0, "prompt_n": None, "cache_n": None}
            ultimo = ("nuovo", t)
            continue
        if cur is None:
            continue
        if RESTORE.search(riga):
            cur["ripristini"] += 1
        elif m := CACHED.search(riga):
            if ultimo and ultimo[0] == "nuovo":
                cur["prefisso_ms"] += t - ultimo[1]   # confronto del prefisso e seq_rm
            elif ultimo and ultimo[0] == "batch":
                cur["decode_ms"] += t - ultimo[1]     # il decode appena finito
            if cur.get("cache_n") is None:
                cur["cache_n"] = int(m.group(1))      # il primo: quanto era in cache
            ultimo = ("cached", t)
        elif DECIDE.search(riga):
            pass
        elif m := CHECKPOINT.search(riga):
            if ultimo:
                cur["checkpoint_ms"] += t - ultimo[1]
            cur["checkpoint"] += 1
            cur["checkpoint_mib"] += float(m.group(3))
            ultimo = ("checkpoint", t)
        elif BATCH.search(riga):
            cur["decode"] += 1
            ultimo = ("batch", t)
        elif m := SAMPLER.search(riga):
            cur["sampler_ms"] += float(m.group(1))
            if ultimo and ultimo[0] == "batch":
                cur["decode_ms"] += t - ultimo[1]
            ultimo = ("sampler", t)
        elif PRIMO.search(riga):
            # il cronometro di prompt_ms finisce al primo token campionato
            if ultimo:
                cur["coda_ms"] = t - ultimo[1]        # ultimo decode del prompt e campionamento
            cur["totale_ms"] = t - cur["t0"]
            cur["prompt_n"] = cur["token_totali"] - cur["cache_n"] if cur.get("cache_n") is not None else None
            richieste.append(cur)
            cur, ultimo = None, None
    if cur:
        richieste.append(cur)
    return richieste


def med(valori) -> float:
    v = [x for x in valori if x is not None]
    return round(st.median(v), 1) if v else float("nan")


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("log", nargs="*")
    ap.add_argument("--pronto", action="append", default=[])
    args = ap.parse_args()
    percorsi = [Path(x) for x in args.log]
    for p in args.pronto:
        percorsi.append(Path(json.loads(Path(p).read_text(encoding="utf-8"))["run_dir"]) / "server.log")
    if not percorsi:
        return 2
    for p in percorsi:
        r = [x for x in leggi(p) if x.get("prompt_n") is not None]
        caldi = [x for x in r if x["prompt_n"] <= 600]   # le richieste di un agente: poco di nuovo
        print(f"\n=== {p.parent.name} · {len(r)} richieste ({len(caldi)} con <= 600 token nuovi)")
        if not caldi:
            continue
        print(f"  dal «new prompt» al primo token: {med(x['totale_ms'] for x in caldi):8.1f} ms   "
              f"(token nuovi mediani {med(x['prompt_n'] for x in caldi):.0f})")
        print(f"    confronto prefisso   {med(x['prefisso_ms'] for x in caldi):8.1f} ms")
        print(f"    checkpoint           {med(x['checkpoint_ms'] for x in caldi):8.1f} ms   "
              f"(n {med(x['checkpoint'] for x in caldi):.0f}, {med(x['checkpoint_mib'] for x in caldi):.0f} MiB)")
        print(f"    decode del prompt    {med(x['decode_ms'] for x in caldi):8.1f} ms   "
              f"(batch {med(x['decode'] for x in caldi):.0f})")
        print(f"    init sampler         {med(x['sampler_ms'] for x in caldi):8.1f} ms")
        print(f"    coda (campionamento) {med(x['coda_ms'] for x in caldi):8.1f} ms")
        rip = sum(x["ripristini"] for x in r)
        print(f"  ripristini/ricalcoli   {rip}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
