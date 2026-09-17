"""M-15 — riassume un risultati.jsonl in tabelle Markdown per il rapporto.

Uso: python analizza.py <risultati.jsonl> [--agente nonio]

Per modello: compiti riusciti, ore di macchina (somma dei secondi dei compiti; il caricamento del
modello è a parte), compiti riusciti per ora, turni e token mediani, token di ragionamento per
turno, chiamate agli strumenti fallite, compattazioni, cause dei fallimenti. Poi l'esito per
compito, per livello e per lingua (i gemelli en/it).
"""

from __future__ import annotations

import argparse
import json
import statistics
import sys
from collections import Counter, defaultdict
from pathlib import Path

ORDINE = ["G1", "FC", "G3", "FN"]


def med(v):
    v = [x for x in v if x is not None]
    return statistics.median(v) if v else None


def fmt(x, nd=0):
    if x is None:
        return "—"
    if isinstance(x, float):
        return f"{x:,.{nd}f}".replace(",", " ")
    return f"{x:,}".replace(",", " ") if isinstance(x, int) else str(x)


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("jsonl")
    ap.add_argument("--agente", default="nonio")
    args = ap.parse_args()
    righe, eventi = [], []
    for line in Path(args.jsonl).read_text(encoding="utf-8").splitlines():
        r = json.loads(line)
        if r.get("compito") and r.get("agente") == args.agente:
            righe.append(r)
        elif r.get("evento"):
            eventi.append(r)
    # Se un compito compare più volte (ripresa), vale l'ultima riga.
    ultime = {}
    for r in righe:
        ultime[(r["modello"], r["compito"], r["ripetizione"])] = r
    righe = list(ultime.values())
    # Righe scritte prima che il runner contasse i comandi di shell falliti: si rileggono dalla traccia.
    sys.path.insert(0, str(Path(__file__).resolve().parent))
    import esegui

    base = Path(args.jsonl).parent
    for r in righe:
        if "shell_falliti" not in r:
            tr = base / r["modello"] / f"{r['compito']}-r{r['ripetizione']}" / "nonio-stdout.jsonl"
            if tr.is_file():
                t = esegui.leggi_traccia(tr)
                for k in ("shell", "shell_falliti", "shell_senza_output", "finish_respinti"):
                    r[k] = t[k]
    per_modello = defaultdict(list)
    for r in righe:
        per_modello[r["modello"]].append(r)
    modelli = [m for m in ORDINE if m in per_modello] + sorted(set(per_modello) - set(ORDINE))

    print("| modello | compiti | riusciti | ore | riusciti/ora | turni (med) | prompt max (med) | output tok | ragion. tok/turno | decode tok/s (med) | chiamate fallite | compattazioni | Windows |")
    print("|---|---|---|---|---|---|---|---|---|---|---|---|---|")
    for m in modelli:
        rs = per_modello[m]
        ok = sum(r["esito"] == "riuscito" for r in rs)
        ore = sum(r["secondi"] for r in rs) / 3600
        turni = [r.get("turni") for r in rs]
        rag = [r.get("token_ragionamento") for r in rs]
        tot_turni = sum(t for t in turni if t)
        rag_turno = (sum(x for x in rag if x) / tot_turni) if tot_turni and all(x is not None for x in rag) else None
        fall = sum(r.get("chiamate_fallite") or 0 for r in rs)
        chiam = sum(r.get("chiamate") or 0 for r in rs)
        print(
            f"| {m} | {len(rs)} | {ok} | {ore:.2f} | {ok / ore if ore else 0:.1f} | {fmt(med(turni))} | "
            f"{fmt(med([r.get('token_prompt_max') for r in rs]))} | {fmt(sum(r.get('token_output') or 0 for r in rs))} | "
            f"{fmt(rag_turno, 0)} | {fmt(med([r.get('motore_decode_tps_mediana') for r in rs]), 1)} | {fall}/{chiam} | "
            f"{sum(r.get('compattazioni') or 0 for r in rs)} | {', '.join(sorted({str(r.get('windows')) for r in rs}))} |"
        )
    print()
    print("| modello | al tetto di turni | finish respinti | shell | shell uscite ≠ 0 | shell mute | compiti con file protetti toccati |")
    print("|---|---|---|---|---|---|---|")
    for m in modelli:
        rs = per_modello[m]
        tetto = sum(1 for r in rs if "turn ceiling" in (r.get("stop_reason") or "") or "time ceiling" in (r.get("stop_reason") or ""))
        print(f"| {m} | {tetto}/{len(rs)} | {sum(r.get('finish_respinti') or 0 for r in rs)} | {sum(r.get('shell') or 0 for r in rs)} | "
              f"{sum(r.get('shell_falliti') or 0 for r in rs)} | {sum(r.get('shell_senza_output') or 0 for r in rs)} | "
              f"{sum(1 for r in rs if r['verifica']['protetti_violati'])} |")
    print()
    print("Cause dei fallimenti:")
    for m in modelli:
        c = Counter(r["causa"] for r in per_modello[m] if r["esito"] != "riuscito")
        print(f"- {m}: " + (", ".join(f"{k} {v}" for k, v in c.most_common()) or "nessuno"))
    print()
    print("Esito per compito (✓ riuscito, ✗ fallito, · non eseguito; tra parentesi i secondi):")
    compiti = sorted({r["compito"] for r in righe}, key=lambda x: [r for r in righe if r["compito"] == x][0]["inizio"])
    print("| compito | livello | " + " | ".join(modelli) + " |")
    print("|---|---|" + "---|" * len(modelli))
    for c in compiti:
        liv = next(r["livello"] for r in righe if r["compito"] == c)
        celle = []
        for m in modelli:
            rr = [r for r in per_modello[m] if r["compito"] == c]
            celle.append(" ".join(("✓" if r["esito"] == "riuscito" else "✗") + f" ({r['secondi']:.0f})" for r in rr) or "·")
        print(f"| {c} | {liv} | " + " | ".join(celle) + " |")
    print()
    print("Per livello (riusciti/eseguiti):")
    for m in modelli:
        d = defaultdict(lambda: [0, 0])
        for r in per_modello[m]:
            d[r["livello"]][1] += 1
            d[r["livello"]][0] += r["esito"] == "riuscito"
        print(f"- {m}: " + ", ".join(f"{k} {v[0]}/{v[1]}" for k, v in d.items()))
    print()
    print("Gemelli en/it (esito en · esito it, secondi):")
    for m in modelli:
        parti = []
        for base in ("rs-durata", "py-config", "ts-eventi"):
            en = next((r for r in per_modello[m] if r["compito"] == f"{base}-en"), None)
            it = next((r for r in per_modello[m] if r["compito"] == f"{base}-it"), None)
            if en and it:
                parti.append(f"{base}: {en['esito'][0]}{en['secondi']:.0f}s · {it['esito'][0]}{it['secondi']:.0f}s")
        print(f"- {m}: " + "; ".join(parti))
    print()
    print("Controlli: solo motore locale = " + str(Counter(str(r.get("solo_motore_locale")) for r in righe)))
    for ev in eventi:
        print(f"evento: {json.dumps(ev, ensure_ascii=False)}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
