"""M-16 T-01 — log-verosimiglianza del token vero, per zona, da più file di logit.

    python nll_zona.py <nome=file.kld> [<nome=file.kld> ...] [--zona A-B ...]

Per ogni zona di posizioni del prompt (default: la zona fragile del G1, 5680-5773, e i due
blocchi interi) stampa la NLL media del token vero e la perplessità equivalente per ogni file.
Dice quale backend segue meglio il testo dove i backend non sono d'accordo.
"""
import argparse

import numpy as np

import kld_per_token as k


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("file", nargs="+")
    ap.add_argument("--zona", action="append")
    a = ap.parse_args()
    lett = {}
    for x in a.file:
        nome, path = x.split("=", 1)
        lett[nome] = k.leggi(path)
    r0 = next(iter(lett.values()))
    nv, pb, nctx = r0["n_vocab"], r0["per_blocco"], r0["n_ctx"]
    pos = np.array([divmod(i, pb)[0] * nctx + nctx // 2 + divmod(i, pb)[1] for i in range(len(r0["rec"]))])
    zone = a.zona or ["5680-5773", f"{nctx // 2}-{nctx - 2}", f"{nctx + nctx // 2}-{2 * nctx - 2}"]
    for z in zone:
        da, a_ = (int(v) for v in z.split("-"))
        idx = np.nonzero((pos >= da) & (pos <= a_))[0]
        parti = []
        for nome, r in lett.items():
            v = [-float(k.logp(r["rec"][i], nv)[int(r["tokens"][pos[i] + 1])]) for i in idx]
            parti.append(f"{nome} {np.mean(v):.3f} (PPL {np.exp(np.mean(v)):.3f})")
        print(f"zona {z} · {len(idx)} token · NLL media: " + " · ".join(parti))


if __name__ == "__main__":
    main()
