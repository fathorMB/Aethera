"""M-08 — dalle righe grezze del banco alle tabelle del rapporto.

Legge `<radice>/m08/T-NN.jsonl` (una riga per richiesta, numeri del motore così come li manda) e
`<radice>/m08/T-NN.meta.json` (condizioni e run id) e stampa una tabella Markdown per misura.

Mediana e scarto tipo sui soli giri buoni: i riscaldamenti restano nel file ma non nel conto.

    python analisi.py C:/AetheraData/m08 T-04 T-05 …
"""
import io
import json
import os
import statistics
import sys
import tomllib


if hasattr(sys.stdout, "reconfigure"):
    sys.stdout.reconfigure(encoding="utf-8")


def load(dirpath, measure):
    rows = []
    path = os.path.join(dirpath, measure + ".jsonl")
    if not os.path.exists(path):
        return rows, {}
    with open(path, encoding="utf-8") as fh:
        for line in fh:
            line = line.strip()
            if line:
                rows.append(json.loads(line))
    meta_path = os.path.join(dirpath, measure + ".meta.json")
    meta = json.load(open(meta_path, encoding="utf-8")) if os.path.exists(meta_path) else {}
    return rows, meta


def from_manifest(runs_dir, run_id):
    """Le condizioni vere stanno nel manifest dell'avvio: il banco ne tiene una copia, ma se l'ha
    letto troppo presto (la memoria misurata arriva qualche secondo dopo il «pronto») la fonte
    buona resta `runs/<id>/manifest.toml`."""
    path = os.path.join(runs_dir, run_id, "manifest.toml")
    if not run_id or not os.path.exists(path):
        return {}
    with open(path, "rb") as fh:
        return tomllib.load(fh)


def stat(values):
    values = [v for v in values if v is not None]
    if not values:
        return None, None
    med = statistics.median(values)
    sd = statistics.stdev(values) if len(values) > 1 else 0.0
    return med, sd


def fmt(med, sd, digits=1):
    if med is None:
        return "—"
    return "%.*f ± %.*f" % (digits, med, digits, sd or 0.0)


def table(rows, meta, measure, dirpath):
    by = {}
    for r in rows:
        if r.get("warmup"):
            continue
        key = (r["variant"], r["workload"], r.get("turn", 0))
        by.setdefault(key, []).append(r)

    runs = {v["variant"]: v for v in meta.get("variants", [])}
    print("\n### %s — %s" % (measure, meta.get("title", "")))
    print("\n| variante | carico | turno | prefill tok/s | decode tok/s | token di prompt | dalla cache | giri | run id |")
    print("|---|---|---:|---:|---:|---:|---:|---:|---|")
    for (variant, workload, turn), group in sorted(by.items()):
        pp = stat([g["timings"].get("prompt_per_second") for g in group])
        tg = stat([g["timings"].get("predicted_per_second") for g in group])
        pn = stat([g["timings"].get("prompt_n") for g in group])
        ca = stat([g.get("cache_tokens") for g in group])
        run = runs.get(variant, {}).get("run_id", "")
        print("| %s | %s | %d | %s | %s | %s | %s | %d | %s |" % (
            variant, workload, turn, fmt(*pp), fmt(*tg, digits=2),
            "%d" % pn[0] if pn[0] is not None else "—",
            "%d" % ca[0] if ca[0] is not None else "—",
            len(group), run))

    # Accettazione della speculazione, quando il motore la riporta.
    acc = {}
    for r in rows:
        if r.get("warmup"):
            continue
        t = r["timings"]
        n, a = t.get("draft_n"), t.get("draft_n_accepted")
        if n:
            acc.setdefault((r["variant"], r["workload"]), []).append(a / n)
    if acc:
        print("\n| variante | carico | accettazione dei token proposti |")
        print("|---|---|---:|")
        for (variant, workload), v in sorted(acc.items()):
            m, sd = stat(v)
            print("| %s | %s | %.3f ± %.3f |" % (variant, workload, m, sd))

    # Coerenza del testo a temperatura 0: stesse impronte o no.
    sha = {}
    for r in rows:
        if r.get("warmup"):
            continue
        sha.setdefault((r["workload"], r["variant"]), set()).add(r["text_sha256_12"])
    zero = {k: v for k, v in sha.items() if len(v) >= 1}
    if zero:
        print("\n| carico | variante | impronte del testo generato |")
        print("|---|---|---|")
        for (workload, variant), v in sorted(zero.items()):
            print("| %s | %s | %s |" % (workload, variant, ", ".join(sorted(v))))

    print("\n**Condizioni**\n")
    print("| variante | build | ctx servito | caricamento ms | VRAM dedicata GiB | VRAM condivisa GiB | working set GiB | RAM libera GiB | doppia copia |")
    print("|---|---|---:|---:|---:|---:|---:|---:|---|")
    runs_dir = os.path.join(os.path.dirname(os.path.abspath(dirpath)), "runs")
    for v in meta.get("variants", []):
        m = v.get("memory_after_load") or {}
        if not m:
            m = (from_manifest(runs_dir, v.get("run_id", "")).get("memory") or {}).get("after_load") or {}
        print("| %s | %s | %s | %s | %s | %s | %s | %s | %s |" % (
            v.get("variant"), v.get("build", "—"), v.get("ctx_served", "—"), v.get("load_ms", "—"),
            m.get("vram_dedicated_gib", "—"), m.get("vram_shared_gib", "—"),
            m.get("working_set_gib", "—"), m.get("ram_available_gib", "—"), m.get("double_copy", "—")))
    for v in meta.get("variants", []):
        if v.get("command"):
            print("\n- `%s` → `%s`" % (v["variant"], v["command"]))
        if v.get("error"):
            print("\n- `%s` **non avviata**: %s" % (v["variant"], v["error"]))


def main():
    d = sys.argv[1]
    measures = sys.argv[2:] or sorted(
        f[:-6] for f in os.listdir(d) if f.endswith(".jsonl") and not f.endswith(".meta.jsonl"))
    for m in measures:
        rows, meta = load(d, m)
        if not rows:
            print("\n### %s — nessuna riga" % m)
            continue
        table(rows, meta, m, d)


if __name__ == "__main__":
    main()
