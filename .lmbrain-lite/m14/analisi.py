"""M-14 — riassunto delle misure del fork leggero.

    python analisi.py bench <file.jsonl> [<file.jsonl> ...]   llama-bench (T-04, T-06-bench)
    python analisi.py banco <T-06x.jsonl>                     banco m08_bench (T-06a, T-06b)

bench: una riga per modello e prova, con media, sd e i cinque campioni in ordine di tempo.
banco: mediana e sd di prefill e decode per variante e carico (giri buoni, senza riscaldamento);
       poi la coerenza del testo a temperatura 0: per ogni giro, se il testo della variante è
       identico a quello della base (moro0) e, se no, dopo quanti caratteri diverge. La coppia
       moro0 / moro0-bis dà la divergenza che il backend produce da solo.
"""
import json
import statistics as st
import sys
from collections import defaultdict


def bench(paths):
    for path in paths:
        print(f"== {path}")
        for line in open(path, encoding="utf-8"):
            d = json.loads(line)
            samples = " ".join(f"{x:.1f}" for x in d["samples_ts"])
            print(f"  {d['model_type'][:24]:24} p{d['n_prompt']:<5} g{d['n_gen']:<4} "
                  f"{d['avg_ts']:8.2f} ± {d['stddev_ts']:6.2f}   [{samples}]  commit {d['build_commit']}")


def common_prefix(a, b):
    n = 0
    for x, y in zip(a, b):
        if x != y:
            break
        n += 1
    return n


def banco(path):
    rows = [json.loads(l) for l in open(path, encoding="utf-8")]
    good = [r for r in rows if not r["warmup"]]
    order = []
    for r in good:
        if r["variant"] not in order:
            order.append(r["variant"])
    workloads = []
    for r in good:
        if r["workload"] not in workloads:
            workloads.append(r["workload"])
    runs = {r["variant"]: r["run_id"] for r in good}
    print(f"== {path}  (avvii: {', '.join(f'{v} {runs[v]}' for v in order)})")
    for w in workloads:
        print(f"  -- {w}")
        for v in order:
            rs = [r for r in good if r["variant"] == v and r["workload"] == w]
            if not rs:
                print(f"     {v:10} nessun giro buono (variante interrotta)")
                continue
            pp =[r["timings"]["prompt_per_second"] for r in rs]
            tg = [r["timings"]["predicted_per_second"] for r in rs]
            n = rs[0]["timings"]["prompt_n"] if rs else 0
            sd = lambda xs: st.stdev(xs) if len(xs) > 1 else 0.0
            print(f"     {v:10} n={len(rs)} prompt {n:6}  prefill {st.median(pp):7.1f} (sd {sd(pp):5.1f})"
                  f"  decode {st.median(tg):6.2f} (sd {sd(tg):4.2f})"
                  f"  prefill [{' '.join(f'{x:.0f}' for x in pp)}]")
        base = order[0]
        by = defaultdict(dict)
        for r in good:
            if r["workload"] == w:
                by[r["variant"]][r["rep"]] = r
        for v in order[1:]:
            same, prefixes = 0, []
            for rep, rb in by[base].items():
                rv = by[v].get(rep)
                if rv is None:
                    continue
                ta, tb = rb.get("text"), rv.get("text")
                if ta is None or tb is None:
                    ta, tb = rb["text_sha256_12"], rv["text_sha256_12"]
                if ta == tb:
                    same += 1
                    prefixes.append(None)
                else:
                    prefixes.append(common_prefix(ta, tb))
            div = ", ".join("=" if p is None else f"{p}" for p in prefixes)
            print(f"     testo {v} contro {base}: {same}/{len(prefixes)} identici; carattere di divergenza per giro: {div}")


if __name__ == "__main__":
    if len(sys.argv) < 3 or sys.argv[1] not in ("bench", "banco"):
        print(__doc__)
        sys.exit(2)
    if sys.argv[1] == "bench":
        bench(sys.argv[2:])
    else:
        for p in sys.argv[2:]:
            banco(p)
