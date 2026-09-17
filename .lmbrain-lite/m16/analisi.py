"""M-16 — riassunto delle misure.

    python analisi.py kld <radice>/m16                 T-01: una riga per passo di llama-perplexity
    python analisi.py banco <T-02x.jsonl> [<meta>]     T-02: mediane, memoria, testo a temperatura 0
    python analisi.py freddo <tok/s> [<tok/s> ...]     secondi per un prompt a freddo da 16.822 token

kld: KLD media e al 99%, «Same top p», PPL(Q), PPL(base) e la loro differenza.
banco: di ogni variante si tiene l'ultimo avvio completo (con --riprendi una variante interrotta si
       rifà). Mediana e sd di prefill e decode per variante e carico; memoria dopo il caricamento dal
       file .meta.json; poi il testo a temperatura 0, giro per giro:
       - ubatch: ogni variante contro la stessa build all'ubatch del profilo (argomento di T-04);
       - patch: moro1 contro moro0 allo stesso ubatch.
"""
import json
import re
import statistics as st
import sys
from pathlib import Path

PROMPT_CLAUDE_CODE = 16822


def kld(cartella):
    cartella = Path(cartella)
    campi = {
        "ppl_q": r"Mean PPL\(Q\)\s*:\s*([\d.]+)\s*±\s*([\d.]+)",
        "ppl_base": r"Mean PPL\(base\)\s*:\s*([\d.]+)\s*±\s*([\d.]+)",
        "dppl": r"Mean PPL\(Q\)-PPL\(base\)\s*:\s*(-?[\d.]+)\s*±\s*([\d.]+)",
        "kld": r"Mean\s+KLD:\s*(-?[\d.]+)\s*±\s*([\d.]+)",
        "kld99": r"99\.0%\s+KLD:\s*(-?[\d.]+)",
        "kldmax": r"Maximum KLD:\s*([\d.]+)",
        "top": r"Same top p:\s*([\d.]+)\s*±\s*([\d.]+)",
        "rms_dp": r"RMS Δp\s*:\s*([\d.]+)\s*±\s*([\d.]+)",
        "final": r"Final estimate: PPL = ([\d.]+) \+/- ([\d.]+)",
    }
    print(f"{'file':34} {'PPL(Q)':>16} {'ΔPPL':>18} {'KLD media':>22} {'KLD 99%':>10} {'KLD max':>9} {'same top p':>16} {'RMS Δp':>14}")
    for f in sorted(cartella.glob("T-01-*.txt")):
        t = f.read_text(encoding="utf-8", errors="replace")
        v = {k: re.search(r, t) for k, r in campi.items()}
        g = lambda k, i=1: v[k].group(i) if v[k] else "—"
        if v["final"] and not v["kld"]:
            print(f"{f.stem:34} {g('final')} ± {g('final', 2)} (base)")
            continue
        print(f"{f.stem:34} {g('ppl_q'):>8} ± {g('ppl_q', 2):>5} {g('dppl'):>9} ± {g('dppl', 2):>7}"
              f" {g('kld'):>10} ± {g('kld', 2):>9} {g('kld99'):>10} {g('kldmax'):>9} {g('top'):>7} ± {g('top', 2):>5} %"
              f" {g('rms_dp'):>6} ± {g('rms_dp', 2):>5} %")
    for f in sorted(cartella.glob("T-01-*-base.kld")):
        print(f"{f.name}: {f.stat().st_size / 2**30:.2f} GiB")


def prefisso(a, b):
    n = 0
    for x, y in zip(a, b):
        if x != y:
            break
        n += 1
    return n


def ultimo_avvio(righe):
    """Per ogni variante, le righe del suo ultimo avvio (run_id più recente)."""
    per = {}
    for r in righe:
        per.setdefault(r["variant"], set()).add(r["run_id"])
    ultimo = {v: max(ids) for v, ids in per.items()}
    return [r for r in righe if r["run_id"] == ultimo[r["variant"]]]


def banco(path, meta_path=None):
    righe = ultimo_avvio([json.loads(l) for l in open(path, encoding="utf-8") if l.strip()])
    buone = [r for r in righe if not r["warmup"]]
    varianti, carichi = [], []
    for r in buone:
        if r["variant"] not in varianti:
            varianti.append(r["variant"])
        if r["workload"] not in carichi:
            carichi.append(r["workload"])
    meta = {}
    mp = Path(meta_path) if meta_path else Path(path).with_suffix(".meta.json")
    if mp.is_file():
        for m in json.loads(mp.read_text(encoding="utf-8"))["variants"]:
            meta[m["variant"]] = m
    print(f"== {path}")
    print(f"   {'variante':14} {'avvio':18} {'pronto s':>8} {'VRAM GiB':>9} {'RAM libera':>10}")
    for v in varianti:
        m = meta.get(v, {})
        mem = m.get("memory_after_load") or {}
        run = next(r["run_id"] for r in buone if r["variant"] == v)
        print(f"   {v:14} {run:18} {m.get('load_ms', 0) / 1000:8.1f} {mem.get('vram_dedicated_gib') or 0:9.2f} {mem.get('ram_available_gib') or 0:10.1f}")
    for w in carichi:
        print(f"  -- {w}")
        for v in varianti:
            rs = [r for r in buone if r["variant"] == v and r["workload"] == w]
            pp = [r["timings"]["prompt_per_second"] for r in rs]
            tg = [r["timings"]["predicted_per_second"] for r in rs]
            sd = lambda xs: st.stdev(xs) if len(xs) > 1 else 0.0
            dr = [r["timings"].get("draft_n_accepted", 0) / r["timings"]["draft_n"] for r in rs if r["timings"].get("draft_n")]
            acc = f"  MTP accettati {st.median(dr):.2f}" if dr else ""
            print(f"     {v:14} n={len(rs)} prompt {rs[0]['timings']['prompt_n']:6}  prefill {st.median(pp):7.1f} (sd {sd(pp):5.1f})"
                  f"  decode {st.median(tg):6.2f} (sd {sd(tg):4.2f}){acc}")
        testi = {}
        for r in buone:
            if r["workload"] == w:
                testi.setdefault(r["variant"], {})[r["rep"]] = r.get("text") if r.get("text") is not None else r["text_sha256_12"]
        profilo = {"T-02a": "ub4096", "T-02b": "ub512"}.get(buone[0]["measure"])
        for v in varianti:
            serie, ub = v.split("-")
            confronti = []
            if profilo and ub != profilo and f"{serie}-{profilo}" in testi:
                confronti.append((f"{serie}-{profilo}", "ubatch"))
            if serie == "moro1" and f"moro0-{ub}" in testi:
                confronti.append((f"moro0-{ub}", "patch"))
            for rif, tipo in confronti:
                uguali, div = 0, []
                for rep, ta in testi[rif].items():
                    tb = testi[v].get(rep)
                    if tb is None:
                        continue
                    if ta == tb:
                        uguali += 1
                        div.append("=")
                    else:
                        div.append(str(prefisso(ta, tb)))
                print(f"     testo [{tipo}] {v} contro {rif}: {uguali}/{len(div)} identici · divergenza al carattere {', '.join(div)}")


def freddo(valori):
    for x in valori:
        x = float(x)
        print(f"{x:8.1f} tok/s → {PROMPT_CLAUDE_CODE / x:6.1f} s per {PROMPT_CLAUDE_CODE} token")


if __name__ == "__main__":
    if len(sys.argv) < 3 or sys.argv[1] not in ("kld", "banco", "freddo"):
        print(__doc__)
        sys.exit(2)
    if sys.argv[1] == "kld":
        kld(sys.argv[2])
    elif sys.argv[1] == "banco":
        banco(*sys.argv[2:4])
    else:
        freddo(sys.argv[2:])
