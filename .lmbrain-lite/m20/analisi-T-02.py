"""M-20 T-02 — aggrega le righe grezze in una tabella leggibile.

Aggrega, non riscrive: mediana e scarto tipo per condizione e carico, con lo scostamento rispetto
ad A1 e la sentinella A2 messa accanto. Se A1 e A2 non coincidono entro il loro rumore, la misura
ha preso una deriva e lo dice invece di far finta di niente.
"""

import json
import statistics as st
import sys
from collections import defaultdict
from pathlib import Path

RIGHE = Path(sys.argv[1] if len(sys.argv) > 1 else "C:/AetheraData/m20/T-02.jsonl")
COMPAGNO = RIGHE.with_name("T-02-compagno.jsonl")
ORDINE = ["A1-solo", "B-fermo", "C-attivo", "A2-solo"]


def mediana_sd(valori):
    v = list(valori)
    return st.median(v), (st.stdev(v) if len(v) > 1 else 0.0)


def main():
    rows = [json.loads(l) for l in RIGHE.read_text(encoding="utf-8").splitlines()]
    rows = [r for r in rows if r.get("measure") == "M20-T02" and not r["warmup"]]
    if not rows:
        raise SystemExit("nessuna riga utile")

    g = defaultdict(list)
    vram = {}
    for r in rows:
        g[(r["variant"], r["workload"])].append(r["timings"])
        vram.setdefault(r["variant"], r["vram_gib_inizio"])

    carichi = sorted({k[1] for k in g})
    print(f"\n{len(rows)} righe utili · {RIGHE}\n")
    print(f'{"condizione":11} {"VRAM":>6}  ' + "  ".join(f'{c:>28}' for c in carichi))
    print(f'{"":11} {"GiB":>6}  ' + "  ".join(f'{"prefill tok/s    decode tok/s":>28}' for _ in carichi))
    print("-" * (20 + 30 * len(carichi)))

    base = {}
    for cond in ORDINE:
        if not any(k[0] == cond for k in g):
            continue
        celle = []
        for c in carichi:
            t = g.get((cond, c))
            if not t:
                celle.append(f'{"—":>28}')
                continue
            pm, ps = mediana_sd(x["prompt_per_second"] for x in t)
            dm, ds = mediana_sd(x["predicted_per_second"] for x in t)
            if cond == "A1-solo":
                base[c] = (pm, dm)
                celle.append(f"{pm:8.1f} ±{ps:4.1f} {dm:7.2f} ±{ds:4.2f}")
            else:
                bp, bd = base.get(c, (None, None))
                dp = f"{(pm / bp - 1) * 100:+5.1f}%" if bp else "     "
                dd = f"{(dm / bd - 1) * 100:+5.1f}%" if bd else "     "
                celle.append(f"{pm:7.1f} {dp} {dm:6.2f} {dd}")
        v = vram.get(cond)
        print(f'{cond:11} {v if v is not None else "—":>6}  ' + "  ".join(celle))

    # La sentinella: A2 deve tornare dov'era A1.
    print()
    for c in carichi:
        a1, a2 = g.get(("A1-solo", c)), g.get(("A2-solo", c))
        if not (a1 and a2):
            continue
        p1, s1 = mediana_sd(x["prompt_per_second"] for x in a1)
        p2, _ = mediana_sd(x["prompt_per_second"] for x in a2)
        d1, sd1 = mediana_sd(x["predicted_per_second"] for x in a1)
        d2, _ = mediana_sd(x["predicted_per_second"] for x in a2)
        dp, dd = (p2 / p1 - 1) * 100, (d2 / d1 - 1) * 100
        # Due scarti tipo come soglia: sotto, e' rumore; sopra, la sessione e' derivata.
        deriva = abs(p2 - p1) > 2 * max(s1, 1e-9) or abs(d2 - d1) > 2 * max(sd1, 1e-9)
        esito = "DERIVA: A1 e A2 non coincidono, i confronti sopra valgono meno" if deriva else "ok, la sessione non e' derivata"
        print(f"sentinella {c:14} prefill {dp:+5.1f}%  decode {dd:+5.1f}%  -> {esito}")

    if COMPAGNO.exists():
        cr = [json.loads(l) for l in COMPAGNO.read_text(encoding="utf-8").splitlines()]
        per_cond = defaultdict(list)
        for r in cr:
            per_cond[r["condizione"]].append(r)
        print()
        for cond, rr in sorted(per_cond.items()):
            errori = sum("error" in r for r in rr)
            tok = [r["usage"]["completion_tokens"] for r in rr if r.get("usage")]
            ms = [r["wall_ms"] for r in rr if "error" not in r]
            if ms:
                tps = sum(tok) / (sum(ms) / 1000) if tok else 0
                print(f"compagno in {cond:9} {len(rr):4} richieste, {errori} errori, "
                      f"{st.median(ms)/1000:.1f} s mediani, ~{tps:.1f} tok/s complessivi")


if __name__ == "__main__":
    main()
